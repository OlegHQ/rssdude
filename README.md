# rssdude

Local-first RSS feed reader and content curation tool built in Rust. No accounts, no cloud, no nonsense — just your feeds on your machine.

rssdude gives you a full-featured TUI for browsing feeds, a CLI for scripting and automation, and an optional HTTP server for remote access. Everything lives in a single embedded database file.

## What it does

- Subscribe to RSS and Atom feeds
- Sync content locally with conditional HTTP requests (304 support)
- Browse feeds in an interactive terminal UI with vim-style keybindings
- Star, annotate, and mark items as read
- Search across all your content
- Get daily digests and trending topic detection
- Export items as Markdown, JSON, or plain text
- Organize feeds into nested folders with drag-and-drop in TUI
- Run as a server for multi-machine access

## Install

Requires Rust toolchain and `libiconv` on macOS:

```bash
brew install libiconv  # macOS only, keg-only
LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo build --release
```

Binary lands at `target/release/rssdude`.

## Quick start

```bash
# Add some feeds
rssdude add https://blog.anthropic.com/rss --tag ai
rssdude add https://hnrss.org/frontpage --tag tech

# Sync and browse
rssdude sync
rssdude browse  # opens TUI

# Or use the CLI
rssdude items --unread --limit 10
rssdude search "agents"
rssdude digest --since 24h
rssdude trending
```

## TUI keybindings

| Key | Action |
|-----|--------|
| `j`/`k` | Move up/down |
| `h`/`l` | Switch panes left/right |
| `Tab` | Cycle panes forward |
| `Enter` | Open item / enter pane |
| `Space` | Toggle read |
| `*` | Toggle star |
| `N` | Add note |
| `o` | Open link in browser |
| `/` | Search |
| `r` | Sync all feeds |
| `s` | Sync current feed |
| `Ctrl+d`/`Ctrl+u` | Half-page scroll |
| `q` | Quit |

Click URLs in the preview pane to open them in your browser.

## CLI commands

All commands support `--json` for machine-readable output.

```
rssdude add <url> [--tag <tag>] [--folder <id>]
rssdude remove <id> [--yes]
rssdude list [--tag <tag>]
rssdude sync [--feed <id>]
rssdude status
rssdude items [--limit N] [--since 24h] [--tag <tag>] [--unread] [--feed <id>] [--folder <id>]
rssdude read <id> [--open] [--raw]
rssdude search <query> [--limit N]
rssdude mark <id> [--read] [--star] [--note "..."]
rssdude starred [--limit N]
rssdude export <id> --format md|json|text
rssdude digest [--since 24h] [--tag <tag>]
rssdude trending
rssdude match <keywords> [--limit N] [--since 7d]
rssdude folder create|list|rename|move|delete
rssdude move-feed <feed-id> --folder <folder-id>
rssdude serve [--bind 0.0.0.0:8484]
```

## Server mode

rssdude can run as an HTTP server for remote access to a shared database.

```toml
# ~/.rssdude/config.toml
[server]
address = "192.168.1.100:8484"
# token = "optional-bearer-token"
```

```bash
rssdude serve --bind 0.0.0.0:8484  # on the server
rssdude list                         # from any client with config
```

## Storage

Database lives at `~/.rssdude/rssdude.redb` (single file, easy to back up). Override with `RSSDUDE_DB_PATH` env var.

## Built with

[tokio](https://tokio.rs) for async, [native_db](https://github.com/vincent-herleworx/native_db) on [redb](https://github.com/cberner/redb) for storage, [ratatui](https://ratatui.rs) for the TUI, [axum](https://github.com/tokio-rs/axum) for HTTP, [feed-rs](https://github.com/feed-rs/feed-rs) for parsing.

## License

MIT
