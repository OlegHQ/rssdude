mod actions;
mod data;
mod helpers;
mod input;
mod modals;
mod render;
mod state;

use std::collections::{HashMap, HashSet};
use std::io::stdout;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use native_db::Database;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use tokio::task::spawn_blocking;

use crate::db::*;
use crate::feed;
use crate::output::time_ago;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(300);
const FLASH_TTL: Duration = Duration::from_secs(8);
const DOUBLE_CLICK_TTL: Duration = Duration::from_millis(450);

use crate::db::FeedStatsEntry as FeedStats;

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
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum SidebarKind {
    All,
    Starred,
    Uncategorized,
    Folder(String),
    Feed(String),
}

pub(super) struct SidebarEntry {
    pub(super) kind: SidebarKind,
    pub(super) label: String,
    pub(super) unread: usize,
    pub(super) depth: usize,
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
}

pub(super) struct ConfirmModal {
    pub(super) title: String,
    pub(super) body: String,
    pub(super) action: ConfirmAction,
}

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
        title: String,
        total_new: usize,
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
    pub(super) current: String,
    pub(super) total_new: usize,
    pub(super) scope: String,
}

pub(super) struct FlashMessage {
    pub(super) text: String,
    pub(super) is_error: bool,
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
}

impl App {
    fn draw(&mut self, frame: &mut Frame) {
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(frame.area());

        let header = areas[0];
        let body = areas[1];
        let footer = areas[2];

        let (status, is_error) = self.status_text();
        let filters = format!(
            "focus:{}  unread:{}  search:{}  tag:{}  since:{}",
            match self.focus {
                Focus::Sidebar => "sidebar",
                Focus::Items => "items",
                Focus::Preview => "preview",
            },
            if self.unread_only { "on" } else { "off" },
            if self.search_query.is_empty() { "-".to_string() } else { self.search_query.clone() },
            self.tag_filter.as_deref().unwrap_or("-"),
            self.since_filter.as_deref().unwrap_or("-"),
        );
        let header_line = Line::from(vec![
            Span::styled("rssdude", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                status,
                if is_error {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("  "),
            Span::styled(filters, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(header_line), header);

        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Percentage(32),
                Constraint::Percentage(40),
            ])
            .split(body);

        self.layout = Some(UiLayout {
            sidebar: panes[0],
            sidebar_inner: render::inner_rect(panes[0]),
            items: panes[1],
            items_inner: render::inner_rect(panes[1]),
            preview: panes[2],
            preview_inner: render::inner_rect(panes[2]),
        });

        let sidebar_entries = self.sidebar_entries();
        let items = self.visible_items();
        self.sidebar_index = helpers::clamp_index(self.sidebar_index, sidebar_entries.len());
        self.item_index = helpers::clamp_index(self.item_index, items.len());

        self.sidebar_offset = render::draw_sidebar(
            frame,
            panes[0],
            &sidebar_entries,
            self.sidebar_index,
            self.sidebar_offset,
            self.focus,
        );
        self.items_offset = render::draw_items(
            frame,
            panes[1],
            &items,
            self.item_index,
            self.items_offset,
            self.focus,
        );
        self.preview_links = render::draw_preview(
            frame,
            panes[2],
            self.current_item(),
            self.preview_scroll,
            self.focus,
        );

        let footer_text = "Tab panes  j/k move  Enter open  space read  * star/unstar  N note  / search  u unread  a add feed  n new folder  e rename folder  M move  x delete  X recursive delete  s sync selected feed  r sync all  o open link  double-click item open link  q quit";
        frame.render_widget(
            Paragraph::new(Text::from(footer_text))
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::TOP)),
            footer,
        );

        if self.show_help {
            render::draw_help_overlay(frame);
        }

        if let Some(ref lines) = self.digest_content {
            render::draw_scrollable_overlay(frame, "Digest (Esc close, j/k scroll)", lines, self.digest_scroll);
        }

        if let Some(ref lines) = self.trending_content {
            render::draw_scrollable_overlay(frame, "Trending (Esc close, j/k scroll, Enter select)", lines, self.trending_scroll);
        }

        if let Some(modal) = &self.modal {
            render::draw_modal(frame, modal);
        }
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
    let (tx, rx) = channel();
    let mut app = App::new(db, tx, rx);
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
    fn strip_html_removes_tags_and_decodes_basic_entities() {
        let input = "<p>Hello <b>world</b> &amp; friends</p>";
        assert_eq!(helpers::strip_html(input, 200), "Hello world & friends");
    }

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
