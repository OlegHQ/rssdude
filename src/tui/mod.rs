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
    pub(super) mute_filters: Vec<MuteFilter>,
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

#[derive(Clone, Copy, PartialEq, Eq)]
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
    execute!(stdout(), EnableMouseCapture)?;
    let terminal = ratatui::init();
    let result = run_app(terminal, db).await;
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

async fn run_app(mut terminal: DefaultTerminal, db: Arc<Database<'static>>) -> Result<()> {
    let config = crate::shared::config::Config::load().unwrap_or_default();
    let (tx, rx) = channel();
    let mut app = App::new(db, tx, rx, &config.ui.theme);
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
}
