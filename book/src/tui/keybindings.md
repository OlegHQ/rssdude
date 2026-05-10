# TUI keybindings

Three panes: sidebar, item list, preview. Vim keys work, the mouse works.

## Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Down / up (works from any pane — preview doesn't intercept) |
| `h` / `l` | Switch panes (linear, not cycle) |
| `Tab` / `Shift-Tab` | Cycle panes (wrapping) |
| `1` / `3` | Toggle sidebar / preview pane |
| `Ctrl-d` / `Ctrl-u` | Half-page scroll |
| `PageUp` / `PageDown` | Full-page scroll in preview |
| `Enter` | Open item, or descend into a pane |
| `q` | Quit |

## Reading

| Key | Action |
|-----|--------|
| `Space` | Toggle read (and advance to next unread) |
| `*` | Toggle star |
| `N` | Add or edit a note |
| `o` | Open link in `$BROWSER` (also marks read) |
| `/` | Search across all items |
| `r` | Sync everything |
| `s` | Sync the focused feed |

## Visual mode (bulk actions)

| Key | Action |
|-----|--------|
| `v` / `V` | Enter visual mode |
| `j` / `k` | Extend selection |
| `Space` | Mark all selected as read |
| `*` | Star all selected |
| `Esc` | Exit visual mode |

## Sidebar

| Key | Action |
|-----|--------|
| `Space` (on a folder) | Collapse / expand |
| `Enter` (on a folder) | Set as scope filter |
| `▸` / `▾` | Visual indicator for collapse state |

## Other

| Key | Action |
|-----|--------|
| `R` | Mark all in current scope as read |
| `S` | Toggle sort order (newest / oldest) |
| `=` | Run cross-feed dedup |
| `F` | Fetch full article (readability extraction) |
| `I` | Import OPML |
| `b` / `B` | Add to board / create board |
| `W` | Create watch (saved search) |
| `L` | Add to read-later |
| `?` | Help overlay |

## Mouse

- Click any pane to focus it
- Click an item to select it
- Click a link in the preview to open it in `$BROWSER`
- Scroll wheel scrolls the focused pane
