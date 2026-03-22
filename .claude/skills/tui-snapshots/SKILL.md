---
name: tui-snapshots
description: Guide for writing and verifying ratatui TUI snapshot tests using insta. Use after any TUI render changes to verify visual output — covers TestBackend setup, snapshot creation, frame dumping, mock data fixtures, and cargo insta workflow.
---

# TUI Snapshot Testing with insta + ratatui

Verify TUI visual output without a real terminal. Render into ratatui's `TestBackend`, snapshot with `insta`, review diffs with `cargo insta review`.

## Dependencies

```toml
[dev-dependencies]
insta = "1"
```

ratatui's `TestBackend` is built-in (no extra features needed). Already using ratatui 0.29.

## Core Pattern: Full Frame Snapshot

Render the entire app into a `TestBackend` and snapshot it:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::{backend::TestBackend, Terminal};

    fn test_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(120, 40)).unwrap()
    }

    #[test]
    fn test_main_view() {
        let mut terminal = test_terminal();
        let mut app = App::with_test_data(); // see fixtures below
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert_snapshot!(terminal.backend());
    }
}
```

**How it works:**
- `TestBackend::new(width, height)` creates an in-memory screen buffer
- `terminal.draw()` calls your render code exactly like production
- `TestBackend` implements `Display` — outputs one quoted line per row
- `assert_snapshot!` saves/compares against `.snap` files in `snapshots/` directory
- First run creates `.snap.new` files; accept with `cargo insta review`

## Individual Widget Snapshots

For testing a single widget without the full app:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_sidebar_widget() {
    let area = Rect::new(0, 0, 30, 20);
    let mut buf = Buffer::empty(area);

    // Render just the sidebar
    render_sidebar(&mut buf, area, &test_state(), &test_theme());

    assert_snapshot!(buf); // Buffer also implements Display
}
```

This is faster and more focused than full-frame tests.

## Standard Terminal Sizes

Use consistent sizes for reproducible snapshots:

```rust
const TERM_WIDE: (u16, u16) = (120, 40);   // standard wide
const TERM_NARROW: (u16, u16) = (80, 24);   // minimum supported
const TERM_TALL: (u16, u16) = (120, 60);    // tall for long lists
```

Test at multiple sizes to catch layout issues.

## Test Data Fixtures

Create a helper that builds realistic app state without touching the DB:

```rust
impl App {
    #[cfg(test)]
    pub fn with_test_data() -> Self {
        let mut app = App::default();

        // Add mock feeds
        app.feeds = vec![
            Feed { id: "f1".into(), title: Some("Hacker News".into()), url: "https://hn.rss".into(), folder_id: Some("fld1".into()), ..Default::default() },
            Feed { id: "f2".into(), title: Some("Lobsters".into()), url: "https://lobste.rs/rss".into(), folder_id: Some("fld1".into()), ..Default::default() },
            Feed { id: "f3".into(), title: Some("Simon Willison".into()), url: "https://simonwillison.net/atom".into(), folder_id: None, ..Default::default() },
        ];

        // Add mock folders
        app.folders = vec![
            Folder { id: "fld1".into(), name: "Tech".into(), parent_id: None },
            Folder { id: "fld2".into(), name: "AI & ML".into(), parent_id: Some("fld1".into()) },
        ];

        // Add mock items (mix of read/unread/starred)
        app.items = vec![
            Item { id: "i1".into(), feed_id: "f1".into(), title: Some("Rust 2026 Edition Released".into()), published_at: Some("2026-03-22T10:00:00Z".into()), ..Default::default() },
            Item { id: "i2".into(), feed_id: "f1".into(), title: Some("Show HN: A new terminal emulator".into()), published_at: Some("2026-03-22T09:00:00Z".into()), ..Default::default() },
            Item { id: "i3".into(), feed_id: "f2".into(), title: Some("Why I switched to NixOS".into()), published_at: Some("2026-03-21T15:00:00Z".into()), ..Default::default() },
        ];

        // Add mock marks
        app.marks.insert("i1".into(), Mark { item_id: "i1".into(), read: false, starred: true, ..Default::default() });
        app.marks.insert("i2".into(), Mark { item_id: "i2".into(), read: true, starred: false, ..Default::default() });

        app.rebuild_all();
        app
    }
}
```

The fixture should cover: unread items, read items, starred items, nested folders, feeds with errors, empty feeds. Keep it small (3-5 feeds, 5-10 items) for readable snapshots.

## What to Snapshot

Create one snapshot test per major visual state:

| Test Name | State | What It Verifies |
|-----------|-------|------------------|
| `main_view_sidebar_focus` | Default state, sidebar focused | 3-pane layout, sidebar highlight, unread counts |
| `main_view_items_focus` | Items pane focused | Item selection, metadata rendering |
| `main_view_preview_focus` | Preview pane focused, article loaded | Preview content, title, URL rendering |
| `sidebar_collapsed_folder` | Folder collapsed | Collapse indicator, hidden children |
| `items_unread_filter` | Unread-only toggle on | Filtered item list |
| `modal_add_feed` | Add feed modal open | Modal overlay, input fields |
| `modal_confirm_delete` | Confirm dialog open | Confirm dialog rendering |
| `modal_picker` | Picker modal open | Picker list, selection |
| `status_bar_normal` | Normal mode | Status segments, unread count |
| `status_bar_syncing` | Sync in progress | Sync indicator |
| `status_bar_visual` | Visual select mode | VISUAL mode indicator, selection count |
| `help_overlay` | Help modal open | Keybinding reference |
| `digest_overlay` | Digest view | Digest content |
| `empty_state` | No feeds, no items | Empty state messaging |

## Workflow

### Writing new snapshots

```bash
# Run tests — new snapshots create .snap.new files
LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo test tui::tests

# Review snapshots interactively (TUI diff viewer)
cargo insta review

# Accept all pending snapshots (after visual review)
cargo insta accept
```

### After TUI render changes

```bash
# Run snapshot tests — changed renders create .snap.new files
LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo test tui::tests

# Review the diffs
cargo insta review

# If changes look correct, accept. If not, fix the render code.
```

### CI

```bash
# Fails if any snapshot doesn't match (no .snap.new files should exist)
cargo insta test --check
```

## Snapshot File Location

insta stores snapshots next to the test file:

```
src/tui/
  snapshots/
    rssdude__tui__tests__main_view_sidebar_focus.snap
    rssdude__tui__tests__main_view_items_focus.snap
    ...
```

Commit `.snap` files to git. Never commit `.snap.new` files.

## Dumping a Frame for Manual Inspection

For quick visual checks during development:

```rust
#[test]
fn dump_current_ui() {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut app = App::with_test_data();
    terminal.draw(|frame| app.draw(frame)).unwrap();

    // Print to stdout (visible with `cargo test -- --nocapture`)
    println!("{}", terminal.backend());

    // Or write to file
    std::fs::write("frame_dump.txt", terminal.backend().to_string()).unwrap();
}
```

Run with: `cargo test dump_current_ui -- --nocapture`

## Limitations

- **Text only**: Snapshots capture character content and layout but NOT colors/styles. Color changes won't show up in snapshots.
- **Fixed size**: Snapshots are tied to terminal dimensions. Test at consistent sizes.
- **Timing**: Anything time-dependent (flash messages, relative timestamps) needs to be mocked or frozen for deterministic snapshots.

For color verification, iterate over `backend.buffer().content()` and check `cell.fg`, `cell.bg`, `cell.modifier` in assertion-based tests (not snapshots).

## Color Verification (Non-Snapshot)

When you need to verify theme colors are applied correctly:

```rust
#[test]
fn test_selected_item_uses_accent_color() {
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut app = App::with_test_data();
    app.focus = Focus::Items;
    app.items_index = 0;
    terminal.draw(|frame| app.draw(frame)).unwrap();

    let buf = terminal.backend().buffer();
    // Check that the selected item row uses accent foreground
    let cell = &buf[(items_x_offset, items_y_offset)];
    assert_eq!(cell.fg, theme.accent);
}
```

Use this sparingly — only for verifying the theme system applies colors to the right elements.
