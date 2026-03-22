# rssdude v3 — Implementation Plan

## Phase 1: TUI Visual Overhaul + UX Fixes

### 1.1 Theme Infrastructure

**New file:** `src/tui/theme.rs`

Define the color system that everything else builds on.

```rust
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub accent_secondary: Color,
    pub border: Color,
    pub border_focus: Color,
    pub error: Color,
    pub status_bg_1: Color,
    pub status_bg_2: Color,
    pub status_bg_3: Color,
    pub status_fg: Color,
}

impl Theme {
    pub fn dark() -> Self { /* fuchsia/purple accent, dark bg */ }
    pub fn light() -> Self { /* blue/indigo accent, light bg */ }
    pub fn solarized() -> Self { /* solarized dark values */ }
}

/// Detect terminal color capability from env vars.
/// Returns: Truecolor, Color256, or Color16.
pub fn detect_color_depth() -> ColorDepth { ... }

/// Load theme from config, falling back to dark.
pub fn load_theme(config: &Config) -> Theme { ... }
```

Store `Theme` in `App` struct. All render functions receive `&Theme` instead of hardcoding colors.

**Modified:** `src/shared/config.rs` — add `[ui]` section with `theme` field.

---

### 1.2 Render Overhaul

**Modified:** `src/tui/render.rs` (major rewrite)

#### Rounded Borders

Replace all `Block::bordered()` / `Block::default().borders(ALL)` with:
```rust
Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)  // ╭╮╰╯
    .border_style(Style::default().fg(theme.border))
```

Focused pane gets `theme.border_focus` instead.

#### Sidebar Render Changes

Current sidebar builds `ListItem` with plain text. Replace with styled spans:

```rust
// Folder with collapse indicator
let prefix = if collapsed { "▸ " } else { "▾ " };
let tree_char = if is_last { "╰── " } else { "├── " };

// Feed entry
let mut spans = vec![
    Span::styled(tree_char, Style::default().fg(theme.fg_faint)),
    Span::styled(&name, name_style),
];
if unread > 0 {
    spans.push(Span::styled(
        format!(" ({})", unread),
        Style::default().fg(theme.accent_secondary),
    ));
}
if has_error {
    spans.push(Span::styled(" !", Style::default().fg(theme.error).bold()));
}
```

Replace highlight-bar selection with vertical gutter:
```rust
// Selected: thick bar + accent text
let gutter = if selected { "┃ " } else { "  " };
let gutter_style = if selected { theme.accent } else { theme.fg };
```

Section headers (All, Starred, Read Later, Boards, Watches): render as styled text with a bottom border separator line, not as list items with the same highlight style.

#### Items List Render Changes

Replace `"u/r"` and `"*/-"` status text with Unicode icons:

```rust
let unread_icon = if is_unread { "● " } else { "  " };
let star_icon = if is_starred { "★ " } else { "  " };
```

Title styling: **bold when unread**, `fg_faint` when read. Selected: accent color.

Metadata line with dot separator:
```rust
Line::from(vec![
    Span::styled(&feed_name, Style::default().fg(theme.fg_dim)),
    Span::styled(" • ", Style::default().fg(theme.fg_faint)),
    Span::styled(&time_ago, Style::default().fg(theme.fg_dim)),
    Span::styled(" • ", Style::default().fg(theme.fg_faint)),
    Span::styled(star_icon, star_style),
])
```

Vertical bar gutter for selection (same pattern as sidebar).

#### Preview Pane Render Changes

- Title: bold + `theme.accent`
- Source/date metadata: `theme.fg_dim`, dot-separated
- URL: `theme.accent_secondary` + underline
- Horizontal rule between metadata and body: `Span::styled("─".repeat(width), theme.fg_faint)`
- Body text: `theme.fg`, normal weight

#### Status Bar (replaces footer)

Delete the current 2-line footer that dumps all keybindings.

New single-line status bar at the bottom:

```rust
// Layout: [Mode] [Scope] [Unread] [Sync Status]
let segments = vec![
    // Segment 1: Mode (e.g., "NORMAL", "VISUAL", "SEARCH")
    Span::styled(
        format!(" {} ", mode_str),
        Style::default().bg(theme.status_bg_1).fg(theme.status_fg),
    ),
    // Segment 2: Current scope (e.g., "All", "Tech/AI", feed name)
    Span::styled(
        format!(" {} ", scope_str),
        Style::default().bg(theme.status_bg_2).fg(theme.status_fg),
    ),
    // Segment 3: Unread count
    Span::styled(
        format!(" {} unread ", unread_count),
        Style::default().bg(theme.status_bg_3).fg(theme.status_fg),
    ),
    // Segment 4: Sync status (right-aligned)
    Span::styled(
        format!(" {} ", sync_str),
        Style::default().bg(theme.status_bg_2).fg(theme.status_fg),
    ),
];
```

#### Modal Render Changes

Modals get rounded borders, accent-colored border, generous padding (1 row, 2 cols inside).

#### Help Overlay

The `?` overlay should be a well-organized, scrollable reference with section headers. Group keys by context: Navigation, Reading, Curation, Feeds, Search, Misc.

---

### 1.3 Reading Flow Rework

**Modified:** `src/tui/input.rs`

#### j/k from any pane

Currently `j`/`k` behavior depends on focus:
- Sidebar focused: move sidebar selection
- Items focused: move item selection
- Preview focused: scroll preview

Change preview-focused behavior:
```rust
// When preview is focused, j/k navigate items (not scroll)
Focus::Preview => {
    match key.code {
        KeyCode::Char('j') => {
            self.items_index = (self.items_index + 1).min(max);
            self.preview_scroll = 0; // reset scroll for new item
        }
        KeyCode::Char('k') => {
            self.items_index = self.items_index.saturating_sub(1);
            self.preview_scroll = 0;
        }
        // Preview scrolling uses Ctrl+D/U, PgUp/PgDn
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            self.preview_scroll += half_page;
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            self.preview_scroll = self.preview_scroll.saturating_sub(half_page);
        }
    }
}
```

#### Space marks read + advances

```rust
KeyCode::Char(' ') if self.focus == Focus::Items || self.focus == Focus::Preview => {
    if let Some(item) = self.current_item() {
        // Mark as read
        self.spawn_mark_read(item.id.clone());
        // Advance to next unread
        self.advance_to_next_unread();
    }
}
```

`advance_to_next_unread()`: scan `visible_items` from current index forward, find first unread, set `items_index`. If none found, stay put.

#### o opens + marks read

```rust
KeyCode::Char('o') => {
    if let Some(item) = self.current_item() {
        if let Some(link) = &item.link {
            let _ = std::process::Command::new("open").arg(link).spawn();
        }
        // Also mark as read
        self.spawn_mark_read(item.id.clone());
    }
}
```

#### l/h traverse all panes

```rust
KeyCode::Char('l') | KeyCode::Right => {
    self.focus = match self.focus {
        Focus::Sidebar => Focus::Items,
        Focus::Items => Focus::Preview,
        Focus::Preview => Focus::Preview, // stay at rightmost
    };
}
KeyCode::Char('h') | KeyCode::Left => {
    self.focus = match self.focus {
        Focus::Sidebar => Focus::Sidebar, // stay at leftmost
        Focus::Items => Focus::Sidebar,
        Focus::Preview => Focus::Items,
    };
}
```

---

### 1.4 Sidebar Collapse/Expand

**Modified:** `src/tui/state.rs`, `src/tui/input.rs`

Add to App state:
```rust
collapsed_folders: HashSet<String>,  // folder IDs that are collapsed
```

Space key on sidebar when a folder is selected:
```rust
KeyCode::Char(' ') if self.focus == Focus::Sidebar => {
    if let Some(SidebarKind::Folder(id)) = self.selected_sidebar_kind() {
        if !self.collapsed_folders.remove(&id) {
            self.collapsed_folders.insert(id);
        }
        self.rebuild_sidebar(); // only when collapse state changes
    }
}
```

In `sidebar_entries()` builder: skip children of collapsed folders:
```rust
if self.collapsed_folders.contains(&folder.id) {
    continue; // don't recurse into children
}
```

---

### 1.5 Incremental State Updates

**Modified:** `src/tui/state.rs`, `src/tui/data.rs`

#### Optimistic UI updates

Instead of full `load_browser_data()` after every mark/star/note:

```rust
pub fn mark_read_optimistic(&mut self, item_id: &str) {
    // Update in-memory marks
    if let Some(mark) = self.marks.get_mut(item_id) {
        mark.read = true;
        mark.read_at = Some(Utc::now().to_rfc3339());
    } else {
        self.marks.insert(item_id.to_string(), Mark { read: true, .. });
    }
    // Recompute affected feed stats (just this feed, not all)
    self.recompute_feed_stat(feed_id);
    // Invalidate caches
    self.visible_items_dirty = true;
    self.sidebar_dirty = true;
}
```

#### Dirty flags for caching

```rust
pub struct App {
    // ... existing fields ...
    sidebar_dirty: bool,          // rebuild sidebar_entries on next frame
    visible_items_dirty: bool,    // refilter items on next frame
    cached_sidebar: Vec<SidebarEntry>,
    cached_visible_items: Vec<Item>,
}
```

In `draw()`: only call `sidebar_entries()` and `visible_items()` when dirty flags are set. After computing, cache the result and clear the flag.

Set dirty flags when: data changes (sync, mark, star), filter changes (scope, unread toggle, search, time), sort changes.

#### Surgical sync reload

After sync completes, instead of reloading everything:
```rust
// Only reload items for feeds that were synced
pub fn merge_sync_results(&mut self, new_items: Vec<Item>, synced_feed_ids: &[String]) {
    // Remove old items from synced feeds
    self.items.retain(|i| !synced_feed_ids.contains(&i.feed_id));
    // Add new items
    self.items.extend(new_items);
    // Re-sort
    self.items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    self.visible_items_dirty = true;
    self.sidebar_dirty = true;
}
```

---

## Phase 2: Theming

### 2.1 Dark Theme (Default)

Inspired by Charm's signature palette:

```rust
pub fn dark() -> Self {
    Theme {
        bg: Color::Reset,                          // terminal default
        fg: Color::from_u32(0xE0E0E0),            // light gray
        fg_dim: Color::from_u32(0x909090),         // mid gray
        fg_faint: Color::from_u32(0x555555),       // dark gray
        accent: Color::from_u32(0xEE6FF8),         // fuchsia (Charm signature)
        accent_dim: Color::from_u32(0xAD58B4),     // dim fuchsia
        accent_secondary: Color::from_u32(0x04B575), // green
        border: Color::from_u32(0x3C3C3C),         // very dark gray
        border_focus: Color::from_u32(0x7D56F4),   // purple
        error: Color::from_u32(0xFF5F56),          // red
        status_bg_1: Color::from_u32(0x7D56F4),    // purple
        status_bg_2: Color::from_u32(0x353533),    // dark
        status_bg_3: Color::from_u32(0x6124DF),    // deep purple
        status_fg: Color::from_u32(0xFFFDF5),      // cream
    }
}
```

### 2.2 Light Theme

```rust
pub fn light() -> Self {
    Theme {
        bg: Color::Reset,
        fg: Color::from_u32(0x1A1A2E),
        fg_dim: Color::from_u32(0x666680),
        fg_faint: Color::from_u32(0xA0A0B0),
        accent: Color::from_u32(0x4338CA),         // indigo
        accent_dim: Color::from_u32(0x6366F1),
        accent_secondary: Color::from_u32(0x059669), // emerald
        border: Color::from_u32(0xD1D5DB),
        border_focus: Color::from_u32(0x4338CA),
        error: Color::from_u32(0xDC2626),
        status_bg_1: Color::from_u32(0x4338CA),
        status_bg_2: Color::from_u32(0xE5E7EB),
        status_bg_3: Color::from_u32(0x3730A3),
        status_fg: Color::from_u32(0xFFFDF5),
    }
}
```

### 2.3 Solarized Theme

```rust
pub fn solarized() -> Self {
    Theme {
        bg: Color::Reset,
        fg: Color::from_u32(0x839496),             // base0
        fg_dim: Color::from_u32(0x657B83),         // base00
        fg_faint: Color::from_u32(0x586E75),       // base01
        accent: Color::from_u32(0x268BD2),         // blue
        accent_dim: Color::from_u32(0x2AA198),     // cyan
        accent_secondary: Color::from_u32(0x859900), // green
        border: Color::from_u32(0x073642),         // base02
        border_focus: Color::from_u32(0x268BD2),
        error: Color::from_u32(0xDC322F),          // red
        status_bg_1: Color::from_u32(0x268BD2),
        status_bg_2: Color::from_u32(0x073642),
        status_bg_3: Color::from_u32(0x2AA198),
        status_fg: Color::from_u32(0xFDF6E3),      // base3
    }
}
```

### 2.4 Color Fallback

```rust
pub fn to_compatible_color(color: Color, depth: ColorDepth) -> Color {
    match depth {
        ColorDepth::TrueColor => color,
        ColorDepth::Color256 => map_to_256(color),  // nearest xterm-256 color
        ColorDepth::Color16 => map_to_16(color),    // nearest ANSI color
    }
}
```

Use `palette` or hand-roll a nearest-color lookup for the 256 palette. For 16-color, map each semantic slot to the closest ANSI color.

### 2.5 Config Integration

**Modified:** `src/shared/config.rs`

```rust
#[derive(Deserialize, Default)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String { "dark".into() }
```

Load in TUI startup:
```rust
let config = Config::load()?;
let theme = match config.ui.theme.as_str() {
    "light" => Theme::light(),
    "solarized" => Theme::solarized(),
    _ => Theme::dark(),
};
```

---

## Phase 3: Reading Stats

### 3.1 Data Requirements

Stats are computed from existing data — no new models needed.

Required fields (verify these exist):
- `Mark.read` (bool) — is item read?
- `Mark.read_at` (Option<String>) — when was it read? (RFC3339)
- `Mark.starred` (bool) — is it starred?
- `Item.feed_id` — which feed?
- `Item.published_at` — when was it published?
- `Feed.folder_id` — which folder?
- `Feed.tags` — which tags?

**One addition needed:** Track "opened in browser" distinctly from "marked read":
- Add `opened_at: Option<String>` to `Mark` model
- Set when `o` is pressed in TUI or `--open` in CLI
- This distinguishes "skimmed in preview" from "engaged enough to open"

Model migration: `Mark` v1 → v2 with `opened_at` field. Use `From<MarkV1> for Mark` pattern that native_db supports.

### 3.2 Stats Core Function

**New file:** `src/commands/stats.rs`

```rust
#[derive(Serialize)]
pub struct StatsResult {
    pub window_days: u32,
    pub activity: ActivityStats,
    pub per_feed: Vec<FeedEngagement>,
    pub by_hour: Vec<HourBucket>,
    pub by_tag: Vec<TagStats>,
    pub by_folder: Vec<FolderStats>,
    pub dead_feeds: Vec<DeadFeed>,
    pub streak: u32,
}

#[derive(Serialize)]
pub struct ActivityStats {
    pub articles_read: u32,
    pub articles_opened: u32,
    pub articles_starred: u32,
    pub daily_avg: f32,
    pub open_rate: f32,    // opened / read
    pub star_rate: f32,    // starred / read
}

#[derive(Serialize)]
pub struct FeedEngagement {
    pub feed_id: String,
    pub feed_name: String,
    pub read: u32,
    pub opened: u32,
    pub skipped: u32,
    pub engagement_pct: f32,  // read / (read + skipped)
    pub is_low: bool,         // engagement < 10%
}

#[derive(Serialize)]
pub struct HourBucket {
    pub hour_range: String,  // "6-9", "9-12", etc.
    pub count: u32,
}

#[derive(Serialize)]
pub struct TagStats {
    pub tag: String,
    pub articles: u32,
    pub engagement_pct: f32,
}

#[derive(Serialize)]
pub struct FolderStats {
    pub folder_name: String,
    pub depth: usize,
    pub articles: u32,
    pub engagement_pct: f32,
}

#[derive(Serialize)]
pub struct DeadFeed {
    pub feed_id: String,
    pub feed_name: String,
    pub skipped: u32,
    pub engagement_pct: f32,
}
```

#### Algorithm

```rust
pub async fn stats_core(
    db: Arc<Database<'static>>,
    since_days: u32,
    dead_threshold: Option<f32>,
) -> Result<StatsResult>
```

All computation in a single `spawn_blocking` + `r_transaction`:

1. Load all feeds, items, marks, folders in one transaction
2. Compute cutoff = `now - since_days`
3. Filter items to those with `published_at >= cutoff`
4. For each item in window:
   - Look up mark → classify as read, opened, starred, or skipped
   - Bucket by feed_id, hour-of-day, folder, tag
5. Compute per-feed engagement: `read_count / (read_count + skip_count)`
6. Compute streak: walk backwards from today, count consecutive days with `read_at` timestamps
7. Flag dead feeds: engagement < threshold (default 5%)

#### CLI Output

```rust
pub async fn stats(
    db: Arc<Database<'static>>,
    json: bool,
    since: Option<String>,
    dead_threshold: Option<f32>,
) -> Result<()>
```

If `--json`: `print_json(&result)`.

Otherwise, print each section with headers, tables (using `print_table`), and the bar chart for time-of-day (using `█` and `░` block characters).

#### CLI args (main.rs)

```rust
/// Show reading statistics and feed engagement
Stats {
    /// Time window (default: 30d)
    #[arg(long, default_value = "30d")]
    since: String,
    /// Show dead feeds below this engagement % (default: 5)
    #[arg(long)]
    dead: Option<f32>,
},
```

### 3.3 Server/Client

```rust
// Server: GET /api/stats?since=30d&dead=5
async fn stats_handler(State(s): State<AppState>, Query(params): Query<StatsParams>) -> ApiResult<Value> {
    let result = commands::stats::stats_core(s.db, params.since_days(), params.dead).await?;
    Ok(Json(serde_json::json!(result)))
}

// Client:
pub async fn stats(&self, json: bool, since: &str, dead: Option<f32>) -> Result<()> { ... }
```

### 3.4 Mark Model Migration

**Modified:** `src/shared/db.rs`

```rust
// Rename current Mark to MarkV1
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 3, version = 1)]
#[native_db]
pub struct MarkV1 { ... }

// New Mark v2 with opened_at
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 3, version = 2, from = MarkV1)]
#[native_db]
pub struct Mark {
    #[primary_key]
    pub item_id: String,
    pub read: bool,
    pub starred: bool,
    pub note: Option<String>,
    pub read_later: bool,
    pub read_at: Option<String>,
    pub opened_at: Option<String>,  // NEW: set when opened in browser
    pub marked_at: String,
}

impl From<MarkV1> for Mark {
    fn from(old: MarkV1) -> Self {
        Mark {
            opened_at: None,
            // ... copy all other fields
        }
    }
}
```

---

## Phase 4: Auto-Cleanup + Bulk Actions

### 4.1 Auto-Cleanup

**Modified:** `src/commands/sync.rs`, `src/shared/config.rs`

Add to config:
```rust
#[derive(Deserialize, Default)]
pub struct RetentionConfig {
    pub auto_mark_read_after: Option<String>,  // duration like "30d"
    pub auto_delete_after: Option<String>,     // duration like "90d"
}
```

Add cleanup step at end of `sync_core()`:
```rust
async fn run_cleanup(db: Arc<Database<'static>>, retention: &RetentionConfig) -> Result<CleanupResult> {
    let mut marked = 0u32;
    let mut deleted = 0u32;

    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let now = Utc::now().naive_utc();

        if let Some(ref mark_after) = retention.auto_mark_read_after {
            let cutoff = now - parse_duration(mark_after)?;
            // Mark old unread items as read (same pattern as mark_all_read_core)
        }

        if let Some(ref delete_after) = retention.auto_delete_after {
            let cutoff = now - parse_duration(delete_after)?;
            // Delete old items, EXCEPT starred and items on boards
            // Check mark.starred and board_items before deleting
        }

        rw.commit()?;
        Ok(CleanupResult { marked, deleted })
    }).await?
}
```

### 4.2 Bulk Actions (TUI)

**Modified:** `src/tui/input.rs`, `src/tui/state.rs`, `src/tui/render.rs`

Add to App state:
```rust
visual_mode: bool,
selected_items: HashSet<String>,  // item IDs
```

Keybindings:
```rust
KeyCode::Char('v') => {
    self.visual_mode = !self.visual_mode;
    if !self.visual_mode { self.selected_items.clear(); }
}
KeyCode::Char('V') => {
    // Select all visible items
    self.visual_mode = true;
    self.selected_items = self.cached_visible_items.iter().map(|i| i.id.clone()).collect();
}
```

In visual mode, `Space` toggles selection instead of mark-read:
```rust
if self.visual_mode {
    KeyCode::Char(' ') => {
        if let Some(item) = self.current_item() {
            if !self.selected_items.remove(&item.id) {
                self.selected_items.insert(item.id.clone());
            }
            // Advance cursor
            self.items_index += 1;
        }
    }
    // Action keys apply to all selected
    KeyCode::Char('r') => { self.bulk_mark_read(); }
    KeyCode::Char('*') => { self.bulk_star(); }
    KeyCode::Char('b') => { self.bulk_add_to_board(); }
}
```

Render: selected items show `▪` prefix in accent color. Status bar shows "VISUAL (N selected)".

---

## Files Changed Summary

| Phase | New Files | Modified Files |
|-------|-----------|----------------|
| **1** | `src/tui/theme.rs` | `src/tui/render.rs`, `src/tui/input.rs`, `src/tui/state.rs`, `src/tui/mod.rs`, `src/tui/data.rs`, `src/tui/helpers.rs`, `src/tui/modals.rs` |
| **2** | — | `src/tui/theme.rs`, `src/shared/config.rs` |
| **3** | `src/commands/stats.rs` | `src/shared/db.rs`, `src/commands/mod.rs`, `src/main.rs`, `src/net/server.rs`, `src/net/client.rs`, `src/tui/data.rs` (mark opened_at on `o`) |
| **4** | — | `src/commands/sync.rs`, `src/shared/config.rs`, `src/tui/input.rs`, `src/tui/state.rs`, `src/tui/render.rs` |

## Crate Dependencies

No new crates needed. Everything uses ratatui's built-in `Color::from_u32()` for truecolor, `textwrap` for wrapping (already used), and block characters for the stats bar chart.
