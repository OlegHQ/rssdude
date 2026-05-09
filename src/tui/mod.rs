mod actions;
mod data;
mod helpers;
mod input;
mod modals;
mod render;
mod state;
pub(super) mod theme;

use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use native_db::Database;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use tokio::task::spawn_blocking;

use crate::shared::db::*;

use crate::shared::output::time_ago;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(300);
const FLASH_TTL: Duration = Duration::from_secs(8);
const DOUBLE_CLICK_TTL: Duration = Duration::from_millis(450);

use crate::shared::db::FeedStatsEntry as FeedStats;

#[derive(Clone)]
pub(super) struct BrowserData {
    pub(super) feeds: Vec<Feed>,
    pub(super) folders: Vec<Folder>,
    pub(super) items: Vec<Item>,
    pub(super) feed_lookup: HashMap<String, Feed>,
    pub(super) marks: HashMap<String, Mark>,
    pub(super) feed_stats: HashMap<String, FeedStats>,
    pub(super) total_unread: usize,
    pub(super) total_starred: usize,
    pub(super) boards: Vec<Board>,
    pub(super) board_items: Vec<BoardItem>,
    pub(super) watches: Vec<SavedSearch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum SidebarKind {
    All,
    Starred,
    Uncategorized,
    Folder(String),
    Feed(String),
    ReadLater,
    RecentlyRead,
    Board(String),
    Watch(String),
}

pub(super) struct SidebarEntry {
    pub(super) kind: SidebarKind,
    pub(super) label: String,
    pub(super) unread: usize,
    pub(super) depth: usize,
    pub(super) has_error: bool,
    pub(super) is_last_child: bool,
}

#[derive(Clone)]
pub(super) struct VisibleItem {
    pub(super) item: Item,
    pub(super) feed: Option<Feed>,
    pub(super) mark: Option<Mark>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Focus {
    Sidebar,
    Items,
    Preview,
}

pub(super) enum Modal {
    Input(InputModal),
    Picker(PickerModal),
    Confirm(ConfirmModal),
}

pub(super) enum InputPurpose {
    Search,
    AddFeed { folder_id: Option<String> },
    NewFolder { parent_id: Option<String> },
    RenameFolder { folder_id: String },
    EditNote { item_id: String },
    CreateBoard,
    ImportOpml,
    CreateWatch,
}

pub(super) struct InputField {
    pub(super) label: String,
    pub(super) value: String,
}

pub(super) struct InputModal {
    pub(super) title: String,
    pub(super) hint: String,
    pub(super) fields: Vec<InputField>,
    pub(super) active: usize,
    pub(super) purpose: InputPurpose,
}

pub(super) enum PickerPurpose {
    MoveFeed { feed_id: String },
    MoveFolder { folder_id: String },
    FilterByTag,
    FilterByTime,
    ExportItem { item_id: String },
    AddToBoard { item_id: String },
}

pub(super) struct PickerEntry {
    pub(super) label: String,
    pub(super) folder_id: Option<String>,
}

pub(super) struct PickerModal {
    pub(super) title: String,
    pub(super) entries: Vec<PickerEntry>,
    pub(super) selected: usize,
    pub(super) purpose: PickerPurpose,
}

pub(super) enum ConfirmAction {
    DeleteFeed { feed_id: String },
    DeleteFolder { folder_id: String, recursive: bool },
    MarkAllRead { scope: String, scope_id: Option<String> },
    DeleteBoard { board_id: String },
    DeleteWatch { watch_id: String },
}

pub(super) struct ConfirmModal {
    pub(super) title: String,
    pub(super) body: String,
    pub(super) action: ConfirmAction,
}

#[allow(clippy::large_enum_variant)]
pub(super) enum AppMessage {
    DataLoaded(Result<BrowserData, String>),
    ActionFinished {
        result: Result<String, String>,
        refresh: bool,
    },
    SyncStarted {
        total: usize,
        scope: String,
    },
    SyncProgress {
        completed: usize,
        total: usize,
    },
    SyncFinished {
        total_new: usize,
        errors: Vec<String>,
    },
    DigestReady(Vec<String>),
    TrendingReady(Vec<String>),
}

pub(super) struct SyncState {
    pub(super) total: usize,
    pub(super) completed: usize,
    pub(super) scope: String,
}

pub(super) struct FlashMessage {
    pub(super) text: String,
    pub(super) at: Instant,
}

pub(super) struct LastItemClick {
    pub(super) item_index: usize,
    pub(super) at: Instant,
}

#[derive(Clone, Copy)]
pub(super) struct UiLayout {
    pub(super) sidebar: Rect,
    pub(super) sidebar_inner: Rect,
    pub(super) items: Rect,
    pub(super) items_inner: Rect,
    pub(super) preview: Rect,
    pub(super) preview_inner: Rect,
}

/// A URL found in the preview pane, with its line and column range.
pub(super) struct PreviewLink {
    pub(super) line: usize,     // line index in the rendered text (before scroll)
    pub(super) col_start: usize,
    pub(super) col_end: usize,
    pub(super) url: String,
}

pub(super) struct App {
    pub(super) db: Arc<Database<'static>>,
    pub(super) tx: Sender<AppMessage>,
    pub(super) rx: Receiver<AppMessage>,
    pub(super) data: Option<BrowserData>,
    pub(super) focus: Focus,
    pub(super) modal: Option<Modal>,
    pub(super) show_help: bool,
    pub(super) should_quit: bool,
    pub(super) unread_only: bool,
    pub(super) search_query: String,
    pub(super) tag_filter: Option<String>,
    pub(super) since_filter: Option<String>,
    pub(super) digest_content: Option<Vec<String>>,
    pub(super) digest_scroll: u16,
    pub(super) trending_content: Option<Vec<String>>,
    pub(super) trending_scroll: u16,
    pub(super) sidebar_index: usize,
    pub(super) sidebar_offset: usize,
    pub(super) item_index: usize,
    pub(super) items_offset: usize,
    pub(super) preview_scroll: u16,
    pub(super) syncing: Option<SyncState>,
    pub(super) refreshing: bool,
    pub(super) pending_refresh: bool,
    pub(super) flash: Option<FlashMessage>,
    pub(super) last_auto_sync: Instant,
    pub(super) last_item_click: Option<LastItemClick>,
    pub(super) layout: Option<UiLayout>,
    pub(super) preview_links: Vec<PreviewLink>,
    pub(super) theme: theme::Theme,
    pub(super) collapsed_folders: HashSet<String>,
    pub(super) visual_mode: bool,
    pub(super) selected_items: HashSet<String>,
    pub(super) sort_order: String,
    pub(super) dedup: bool,
    pub(super) hover_sidebar: Option<usize>,
    pub(super) hover_item: Option<usize>,
    pub(super) hover_link: Option<String>,
    pub(super) show_sidebar: bool,
    pub(super) show_preview: bool,
    pub(super) sidebar_pct: u16,
    pub(super) preview_pct: u16,
    pub(super) dragging: Option<DragTarget>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum DragTarget {
    SidebarBorder,
    PreviewBorder,
}

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let outer = frame.area();
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // top margin
                Constraint::Min(1),   // body
                Constraint::Length(1), // status bar
            ])
            .split(outer);

        let body = areas[1];
        let status_area = areas[2];

        let mut constraints: Vec<Constraint> = Vec::new();
        if self.show_sidebar { constraints.push(Constraint::Percentage(self.sidebar_pct)); }
        constraints.push(Constraint::Min(20)); // items always visible, takes remaining space
        if self.show_preview { constraints.push(Constraint::Percentage(self.preview_pct)); }

        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(body);

        let (sidebar_area, items_area, preview_area) = match (self.show_sidebar, self.show_preview) {
            (true, true)   => (Some(panes[0]), panes[1], Some(panes[2])),
            (true, false)  => (Some(panes[0]), panes[1], None),
            (false, true)  => (None, panes[0], Some(panes[1])),
            (false, false) => (None, panes[0], None),
        };

        let zero = Rect::default();
        self.layout = Some(UiLayout {
            sidebar: sidebar_area.unwrap_or(zero),
            sidebar_inner: sidebar_area.map(render::inner_rect).unwrap_or(zero),
            items: items_area,
            items_inner: render::inner_rect(items_area),
            preview: preview_area.unwrap_or(zero),
            preview_inner: preview_area.map(render::inner_rect).unwrap_or(zero),
        });

        let sidebar_entries = self.sidebar_entries();
        let items = self.visible_items();
        self.sidebar_index = helpers::clamp_index(self.sidebar_index, sidebar_entries.len());
        self.item_index = helpers::clamp_index(self.item_index, items.len());

        if let Some(area) = sidebar_area {
            self.sidebar_offset = render::draw_sidebar(
                frame, area, &sidebar_entries,
                self.sidebar_index, self.sidebar_offset,
                self.focus, &self.theme, &self.collapsed_folders,
                self.hover_sidebar,
            );
        }
        self.items_offset = render::draw_items(
            frame, items_area, &items,
            self.item_index, self.items_offset,
            self.focus, &self.theme, self.visual_mode, &self.selected_items,
            self.hover_item,
        );
        if let Some(area) = preview_area {
            self.preview_links = render::draw_preview(
                frame, area, self.current_item(),
                self.preview_scroll, self.focus, &self.theme,
            );
        } else {
            self.preview_links.clear();
        }

        self.draw_status_bar(frame, status_area);

        if self.show_help { render::draw_help_overlay(frame, &self.theme); }
        if let Some(ref lines) = self.digest_content {
            render::draw_scrollable_overlay(frame, "Digest (Esc close, j/k scroll)", lines, self.digest_scroll, &self.theme);
        }
        if let Some(ref lines) = self.trending_content {
            render::draw_scrollable_overlay(frame, "Trending (Esc close, j/k scroll, Enter select)", lines, self.trending_scroll, &self.theme);
        }
        if let Some(modal) = &self.modal {
            render::draw_modal(frame, modal, &self.theme);
        }
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        let theme = &self.theme;
        let mode_str = if self.visual_mode {
            format!(" visual ({}) ", self.selected_items.len())
        } else {
            " normal ".to_string()
        };
        let scope_str = format!(" {} ", self.scope_label());
        let unread_str = format!(" {} unread ", self.data.as_ref().map(|d| d.total_unread).unwrap_or(0));
        let right_str = if let Some(ref link) = self.hover_link {
            format!(" {link} ")
        } else if let Some(ref sync) = self.syncing {
            format!(" syncing {}/{} ", sync.completed, sync.total)
        } else if let Some(ref flash) = self.flash {
            if flash.at.elapsed() <= FLASH_TTL {
                format!(" {} ", flash.text)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let segments = vec![
            Span::styled(mode_str.clone(), Style::default().bg(theme.status_bg_1).fg(theme.status_fg).add_modifier(Modifier::BOLD)),
            Span::styled(scope_str, Style::default().bg(theme.status_bg_2).fg(theme.status_fg)),
            Span::styled(unread_str, Style::default().bg(theme.status_bg_3).fg(theme.status_fg)),
            Span::styled(right_str, Style::default().bg(theme.status_bg_2).fg(theme.status_fg)),
        ];
        frame.render_widget(Paragraph::new(Line::from(segments)), area);
    }

    fn scope_label(&self) -> String {
        let entries = self.sidebar_entries();
        entries.get(self.sidebar_index)
            .map(|e| e.label.clone())
            .unwrap_or_else(|| "All Items".into())
    }
}

pub async fn run(db: Arc<Database<'static>>) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        anyhow::bail!("rssdude TUI requires an interactive terminal. Run a subcommand (e.g. `rssdude list`) or pipe input differently.");
    }
    // Resolve the theme BEFORE entering raw mode so an "auto" config can run
    // OSC 11 against the terminal in normal cooked mode.
    let config = crate::shared::config::Config::load().unwrap_or_default();
    let theme = theme::Theme::from_name(&config.ui.theme);
    execute!(stdout(), EnableMouseCapture)?;
    let terminal = ratatui::init();
    let result = run_app(terminal, db, theme).await;
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

async fn run_app(mut terminal: DefaultTerminal, db: Arc<Database<'static>>, theme: theme::Theme) -> Result<()> {
    let (tx, rx) = channel();
    let mut app = App::new(db, tx, rx, theme);
    app.request_refresh();
    app.start_sync(None);

    while !app.should_quit {
        app.handle_messages();
        app.maybe_auto_sync();
        terminal.draw(|frame| app.draw(frame))?;

        if event::poll(POLL_INTERVAL)? {
            let event = event::read()?;
            match event {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::mpsc::channel;

    #[test]
    fn double_click_requires_same_item_within_threshold() {
        let first = Instant::now();
        let same_item_soon = first + Duration::from_millis(200);
        let same_item_late = first + Duration::from_millis(600);

        let click = LastItemClick {
            item_index: 3,
            at: first,
        };

        assert!(helpers::is_double_click(Some(&click), 3, same_item_soon));
        assert!(!helpers::is_double_click(Some(&click), 4, same_item_soon));
        assert!(!helpers::is_double_click(Some(&click), 3, same_item_late));
    }

    // -----------------------------------------------------------------------
    // Fixtures
    // -----------------------------------------------------------------------

    fn open_tmp_db() -> Arc<Database<'static>> {
        // Each test needs its own DB file because redb is single-writer.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = format!(
            "/tmp/rssdude_tui_test_{}_{}_{}.redb",
            std::process::id(), n, gen_id()
        );
        let _ = std::fs::remove_file(&path);
        let builder = native_db::Builder::new();
        let db = builder.create(&MODELS, &path).expect("open tmp db");
        Arc::new(db)
    }

    fn now_iso() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    fn make_browser_data() -> BrowserData {
        let folder = Folder { id: "tech".into(), name: "Tech".into(), parent_id: None, created_at: now_iso() };
        let feed = Feed {
            id: "feed1".into(),
            url: "https://example.com/rss".into(),
            title: Some("Example Feed".into()),
            description: None,
            tags: vec!["tech".into()],
            added_at: now_iso(),
            last_synced: Some(now_iso()),
            etag: None,
            last_modified: None,
            folder_id: Some("tech".into()),
            last_error: None,
            error_count: 0,
            last_success_at: Some(now_iso()),
            custom_title: None,
        };
        let item_unread = Item {
            id: "item1".into(),
            feed_id: "feed1".into(),
            guid: "guid1".into(),
            title: Some("Unread Article".into()),
            link: Some("https://example.com/1".into()),
            content: Some("<p>This is content for the unread article.</p>".into()),
            summary: Some("Unread summary".into()),
            published_at: Some(now_iso()),
            fetched_at: now_iso(),
            full_content: None,
        };
        let item_starred = Item {
            id: "item2".into(),
            feed_id: "feed1".into(),
            guid: "guid2".into(),
            title: Some("Starred Article".into()),
            link: Some("https://example.com/2".into()),
            content: Some("<p>Body text.</p>".into()),
            summary: Some("Starred summary".into()),
            published_at: Some(
                (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339(),
            ),
            fetched_at: now_iso(),
            full_content: None,
        };
        let mark_starred = Mark {
            item_id: "item2".into(),
            read: true,
            starred: true,
            note: Some("kept this one".into()),
            marked_at: now_iso(),
            read_at: Some(now_iso()),
            opened_at: None,
            read_later: false,
        };

        let feed_lookup: HashMap<String, Feed> = std::iter::once((feed.id.clone(), feed.clone())).collect();
        let marks: HashMap<String, Mark> = std::iter::once((mark_starred.item_id.clone(), mark_starred.clone())).collect();
        let items = vec![item_unread.clone(), item_starred.clone()];
        let feed_stats = compute_feed_stats(&items, std::slice::from_ref(&mark_starred));
        let total_unread = feed_stats.values().map(|s| s.unread).sum();
        let total_starred = feed_stats.values().map(|s| s.starred).sum();

        BrowserData {
            feeds: vec![feed],
            folders: vec![folder],
            items,
            feed_lookup,
            marks,
            feed_stats,
            total_unread,
            total_starred,
            boards: vec![],
            board_items: vec![],
            watches: vec![],
        }
    }

    fn build_app() -> App {
        let db = open_tmp_db();
        let (tx, rx) = channel();
        let mut app = App::new(db, tx, rx, theme::Theme::dark());
        app.data = Some(make_browser_data());
        app
    }

    fn render(app: &mut App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).expect("build terminal");
        terminal.draw(|frame| app.draw(frame)).expect("draw frame");
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent {
            code: c,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    // -----------------------------------------------------------------------
    // Render tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_default_layout_shows_three_panes_and_status() {
        let mut app = build_app();
        let frame = render(&mut app, 120, 24);
        assert!(frame.contains("Feeds"), "Sidebar pane title missing:\n{frame}");
        assert!(frame.contains("Items"), "Items pane title missing:\n{frame}");
        assert!(frame.contains("Preview"), "Preview pane title missing:\n{frame}");
        assert!(frame.contains("Tech"), "Folder label missing:\n{frame}");
        assert!(frame.contains("Example Feed"), "Feed label missing:\n{frame}");
        assert!(frame.contains("Unread Article"), "Item title missing:\n{frame}");
        assert!(frame.contains(" unread"), "Status bar unread count missing:\n{frame}");
    }

    #[test]
    fn keypress_q_quits() {
        let mut app = build_app();
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn keypress_question_mark_toggles_help() {
        let mut app = build_app();
        assert!(!app.show_help);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.show_help, "? should open help overlay");
        // Any subsequent key dismisses help (per current behavior).
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_help, "second key should close help");
    }

    #[test]
    fn focus_cycles_with_tab() {
        let mut app = build_app();
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Items);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Preview);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn sidebar_toggle_with_1_and_preview_with_3() {
        let mut app = build_app();
        assert!(app.show_sidebar);
        assert!(app.show_preview);
        app.handle_key(key(KeyCode::Char('1')));
        assert!(!app.show_sidebar, "1 should hide sidebar");
        app.handle_key(key(KeyCode::Char('3')));
        assert!(!app.show_preview, "3 should hide preview");
        // Layout still renders without panicking.
        let frame = render(&mut app, 100, 20);
        assert!(frame.contains("Items"), "Items pane should still render:\n{frame}");
    }

    #[test]
    fn render_dump_default_view() {
        let mut app = build_app();
        let frame = render(&mut app, 100, 24);
        eprintln!("---DEFAULT VIEW---\n{frame}---END---");
    }

    #[test]
    fn render_dump_help_overlay() {
        let mut app = build_app();
        app.show_help = true;
        let frame = render(&mut app, 100, 24);
        eprintln!("---HELP---\n{frame}---END---");
    }

    #[test]
    fn render_dump_after_focus_items_and_navigate() {
        let mut app = build_app();
        // Move into items pane and step down
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('j')));
        let frame = render(&mut app, 100, 24);
        eprintln!("---ITEMS-FOCUS-J---\n{frame}---END---");
    }

    #[test]
    fn render_dump_visual_mode() {
        let mut app = build_app();
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('v')));
        let frame = render(&mut app, 100, 24);
        eprintln!("---VISUAL-MODE---\n{frame}---END---");
    }

    #[test]
    fn render_dump_search_modal() {
        let mut app = build_app();
        app.handle_key(key(KeyCode::Char('/')));
        let frame = render(&mut app, 100, 24);
        eprintln!("---SEARCH-MODAL---\n{frame}---END---");
    }

    #[test]
    fn render_dump_sync_in_progress() {
        let mut app = build_app();
        app.syncing = Some(SyncState { total: 5, completed: 2, scope: "all".into() });
        let frame = render(&mut app, 120, 24);
        eprintln!("---SYNC-PROGRESS---\n{frame}---END---");
    }

    #[test]
    fn render_dump_flash_message() {
        let mut app = build_app();
        app.flash = Some(FlashMessage { text: "Marked 3 items as read".into(), at: Instant::now() });
        let frame = render(&mut app, 120, 24);
        eprintln!("---FLASH---\n{frame}---END---");
    }

    #[test]
    fn render_dump_no_items() {
        let mut app = build_app();
        // Switch to filtering with a query that matches nothing.
        app.search_query = "zzznothingmatcheszzz".into();
        let frame = render(&mut app, 100, 24);
        eprintln!("---NO-ITEMS---\n{frame}---END---");
    }

    #[test]
    fn item_navigation_with_empty_list_does_not_panic() {
        let mut app = build_app();
        // Make items invisible via a bogus search.
        app.search_query = "no-match-zzz".into();
        app.handle_key(key(KeyCode::Tab));         // focus items
        app.handle_key(key(KeyCode::Char('j')));   // down
        app.handle_key(key(KeyCode::Char('k')));   // up
        app.handle_key(key(KeyCode::Char('G')));   // last
        app.handle_key(key(KeyCode::Char('g')));   // first
        app.handle_key(key(KeyCode::Enter));       // open
        // Just shouldn't panic.
        let _ = render(&mut app, 100, 24);
    }

    #[test]
    fn item_navigation_clamps_at_bounds() {
        let mut app = build_app();
        app.handle_key(key(KeyCode::Tab)); // focus items
        // With 2 items, going down 5 times shouldn't blow up the index.
        for _ in 0..5 { app.handle_key(key(KeyCode::Char('j'))); }
        assert!(app.item_index < 2, "index escaped bounds: {}", app.item_index);
        for _ in 0..10 { app.handle_key(key(KeyCode::Char('k'))); }
        assert_eq!(app.item_index, 0);
    }

    #[test]
    fn esc_closes_search_modal() {
        let mut app = build_app();
        app.handle_key(key(KeyCode::Char('/')));
        assert!(app.modal.is_some());
        app.handle_key(key(KeyCode::Esc));
        assert!(app.modal.is_none(), "Esc should close modal");
    }

    #[test]
    fn render_dump_light_theme() {
        let mut app = build_app();
        app.theme = theme::Theme::light();
        let frame = render(&mut app, 100, 24);
        eprintln!("---LIGHT-THEME---\n{frame}---END---");
    }

    #[test]
    fn unread_only_filter_keypress_u() {
        let mut app = build_app();
        assert!(!app.unread_only);
        app.handle_key(key(KeyCode::Char('u')));
        assert!(app.unread_only, "u should toggle unread-only filter");
        // Visible items should now exclude the read+starred fixture.
        let visible = app.visible_items();
        assert!(visible.iter().all(|v| v.mark.as_ref().map(|m| !m.read).unwrap_or(true)));
    }

    #[test]
    fn search_modal_opens_with_slash() {
        let mut app = build_app();
        assert!(app.modal.is_none());
        app.handle_key(key(KeyCode::Char('/')));
        assert!(matches!(app.modal, Some(Modal::Input(_))), "/ should open search input modal");
    }

    #[test]
    fn render_under_extreme_narrow_width_does_not_panic() {
        // Resolves a class of off-by-one bugs in min-width layout.
        let mut app = build_app();
        // 30 is the minimum reasonable width; below that ratatui may clip.
        let frame = render(&mut app, 30, 12);
        assert!(!frame.is_empty());
    }

    #[test]
    fn render_with_no_data_does_not_panic() {
        // Boot path: data hasn't loaded yet.
        let db = open_tmp_db();
        let (tx, rx) = channel();
        let mut app = App::new(db, tx, rx, theme::Theme::dark());
        let frame = render(&mut app, 100, 20);
        assert!(frame.contains("Feeds"));
        assert!(frame.contains("Items"));
        assert!(frame.contains("Preview"));
    }

    #[test]
    fn light_theme_renders_distinct_from_dark() {
        // Sanity-check: switching themes actually changes the rendered output's
        // color attributes, so theme detection is wired through to the renderer.
        let mut a_dark = build_app();
        a_dark.theme = theme::Theme::dark();
        let mut a_light = build_app();
        a_light.theme = theme::Theme::light();

        let backend_dark = TestBackend::new(80, 12);
        let mut td = Terminal::new(backend_dark).unwrap();
        td.draw(|f| a_dark.draw(f)).unwrap();
        let buf_d = td.backend().buffer().clone();

        let backend_light = TestBackend::new(80, 12);
        let mut tl = Terminal::new(backend_light).unwrap();
        tl.draw(|f| a_light.draw(f)).unwrap();
        let buf_l = tl.backend().buffer().clone();

        let mut differ = false;
        for y in 0..buf_d.area.height {
            for x in 0..buf_d.area.width {
                if buf_d[(x, y)].fg != buf_l[(x, y)].fg
                    || buf_d[(x, y)].bg != buf_l[(x, y)].bg
                {
                    differ = true;
                    break;
                }
            }
            if differ { break; }
        }
        assert!(differ, "dark and light themes produced identical buffers");
    }
}
