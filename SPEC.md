# rssdude v3 — Feature Spec

## Goal

Make rssdude a beautiful, high-quality local-first RSS reader with a Charm Bracelet–level TUI. Replace Feedly entirely through superior UX, deterministic curation, and data-driven feed management — no algorithms, no AI.

## What's Done

Everything below shipped. This is the complete feature set.

### Core (v2)
- Feed management (add/remove/rename/move, folders with unlimited nesting, tags)
- Sync with conditional HTTP (ETag/If-Modified-Since), error tracking, auto-disable after 10 failures
- Read/unread/starred/read-later tracking, per-feed/folder unread counts
- Full TUI with 3-pane layout, keyboard nav, modals, mouse support
- Full-text search with feed/folder/starred scoping
- Curation: star, notes, export (md/json/txt), boards, saved searches (watches)
- Discovery: digest, trending, keyword match
- Content: full article fetch with readability extraction, cross-feed dedup
- Operations: OPML import/export, mark-all-read (scoped), mute filters with expiry
- Feed health: doctor command, error count tracking
- Sort order: newest/oldest toggle
- Recently read view, read-later queue
- Client/server mode with full REST API
- Async throughout (tokio)

### v3 Visual Overhaul
- Theme infrastructure: `Theme` struct with 14 color slots, 3 presets (dark/fuchsia, light/indigo, solarized)
- Config: `[ui] theme = "dark"` in config.toml
- Rounded borders (`╭╮╰╯`) on all panels and modals
- Vertical bar gutter (`┃`) selection replacing blue-background highlight
- 4-level text hierarchy: accent bold → normal bold → dim → faint
- Dot-separated metadata (`•`) on items and preview
- Unicode status icons: `●` (unread), `★` (starred)
- Folder collapse/expand with `▸`/`▾` indicators
- Status bar replacing 2-line footer (colored segments: app name/visual mode, scope, unread, sync/flash)
- Help overlay (`?`) with organized sections
- Tree connectors (`├──`/`╰──`) in sidebar folder hierarchy
- Feed error indicators (`!` prefix for feeds with errors)
- Hidden zero unread counts (only show count when > 0)
- Preview pane inner padding
- Mouse hover feedback on sidebar and items list

### v3 UX
- `j`/`k` navigate items from any pane (preview doesn't scroll on j/k)
- `Space` marks read + advances to next unread
- `o` opens in browser + marks as read
- `l`/`h` traverse all 3 panes linearly (not cycle)
- `Tab`/`Shift-Tab` cycle panes (wrapping)
- `Space` on sidebar folder collapses/expands
- Preview scrolling: `Ctrl+D`/`Ctrl+U`, `PageUp`/`PageDown`
- `v`/`V` visual mode with `▪` selection indicators
- Bulk actions in visual mode: `Space` marks all selected read, `*` stars all selected
- All v2 keybindings restored: `R` mark-all-read, `S` sort toggle, `=` dedup, `F` full article, `I` import OPML, `b` add to board, `B` create board, `W` create watch, `L` read-later

### v3 TUI Sidebar
- All Items, Starred, folder tree, uncategorized feeds
- Read Later section with count
- Recently Read section (sorted by read_at)
- Boards (`[B] name`) with item counts
- Watches (`[W] name`) as saved searches
- Scope filtering for all sidebar sections

### v3 Backend
- Reading stats: `rssdude stats --since 30d --dead 5` with activity, per-feed engagement, time-of-day, folder engagement, dead feed detection, streak
- Mark v2 model with `opened_at` field for tracking browser opens
- Stats server endpoint: `GET /api/stats`
- Auto-cleanup: `[retention]` config with `auto_mark_read_after` and `auto_delete_after`, runs after sync
- Mute filter application in TUI visible items

## Non-Goals

- **AI/ML features**: No summarization, no algorithmic ranking, no "Leo". The user controls what they see.
- **Social features**: No sharing to Twitter/LinkedIn. Use the OS clipboard or `export`.
- **Multi-user/teams**: Single-user, single-database. Server mode is for remote access, not collaboration.
- **Custom themes**: v3 ships presets only. Custom color definitions can come in v4.
- **Multiple TUI layouts**: One 3-pane layout. Density is controlled by content, not layout modes.
- **Color depth detection/fallback**: Use truecolor always. Terminals that don't support it are rare enough to ignore for now.
