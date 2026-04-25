# 📡 rssdude

**Your feeds. Your machine. Your rules.**

rssdude is a local-first RSS reader written in Rust. It speaks three dialects out of the same binary:

- 🖥️  a vim-flavoured **TUI** for reading
- ⚡ a scriptable **CLI** for automation and pipelines
- 🌐 an optional **HTTP server** when you want one machine to own the database and the rest to be thin clients

No accounts. No cloud. No tracking. One file in `~/.rssdude/` holds everything you've ever subscribed to.

---

## ✨ Why

Cloud readers vanish (RIP Reader, RIP everything since). Self-hosted readers want a docker-compose, a database, and your weekend. rssdude wants `cargo install` and ~5 MB of disk.

It's also a deliberate experiment in keeping a codebase small: the entire thing — TUI, CLI, server, parser glue, storage layer — sits under 8k lines of Rust. Every file is auditable in an afternoon.

---

## 🚀 Install

### macOS (Homebrew + cargo)

```bash
brew install libiconv          # keg-only, needed for the linker
git clone https://github.com/OlegHQ/rssdude && cd rssdude
make install                    # installs to ~/.cargo/bin/rssdude
```

### Linux

```bash
git clone https://github.com/OlegHQ/rssdude && cd rssdude
cargo install --path .          # ~/.cargo/bin/rssdude
```

Make sure `~/.cargo/bin` is on your `PATH`. Override the install prefix with `make install PREFIX=/usr/local`.

### Other Make targets

| Target | What it does |
|--------|--------------|
| `make build`     | Release build into `target/release/rssdude` |
| `make install`   | `cargo install` to `$(PREFIX)/bin` (default `~/.cargo`) |
| `make uninstall` | Remove the installed binary |
| `make test`      | Run unit + snapshot tests |
| `make clippy`    | Lint with `-D warnings` |
| `make run ARGS="items --unread"` | Quick `cargo run` passthrough |

---

## 🏃 Quick start

```bash
# subscribe
rssdude add https://hnrss.org/frontpage --tag tech
rssdude add https://blog.anthropic.com/rss --tag ai

# pull new content
rssdude sync

# read in the TUI
rssdude browse

# or stay in the shell
rssdude items --unread --limit 10
rssdude search "transformer"
rssdude digest --since 24h
rssdude trending
```

Every command supports `--json` for clean machine-readable output. Pipe into `jq`, glue into shell scripts, ship summaries to Slack — the CLI is designed to compose.

---

## ⌨️ TUI keybindings

The TUI is three panes — sidebar, item list, preview — and a vim-shaped brain.

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `h` / `l` | Switch panes |
| `Tab` / `Shift-Tab` | Cycle panes |
| `1` / `3` | Collapse sidebar / preview |
| `Enter` | Open item, or descend into a pane |
| `Space` | Toggle read |
| `*` | Toggle star |
| `N` | Add or edit note |
| `o` | Open link in `$BROWSER` |
| `/` | Search across all items |
| `r` | Sync everything |
| `s` | Sync the focused feed |
| `Ctrl-d` / `Ctrl-u` | Half-page scroll |
| `q` | Quit |

Mouse works too — click panes to focus, click URLs in the preview to open them, scroll wheel scrolls.

---

## 🛠 CLI reference

```text
rssdude add <url> [--tag <tag>] [--folder <id>]
rssdude remove <id> [--yes]
rssdude list [--tag <tag>]

rssdude sync [--feed <id>]
rssdude status                        # per-folder unread counts

rssdude items   [--limit N] [--since 24h] [--tag <tag>]
                [--unread] [--feed <id>] [--folder <id>]
rssdude read    <id> [--open] [--raw]
rssdude search  <query> [--limit N]

rssdude mark    <id> [--read] [--star] [--note "..."]
rssdude starred [--limit N]
rssdude export  <id> --format md|json|text

rssdude digest   [--since 24h] [--tag <tag>]
rssdude trending
rssdude match    <keywords> [--limit N] [--since 7d]

rssdude folder create|list|rename|move|delete
rssdude move-feed <feed-id> --folder <folder-id>

rssdude serve [--bind 0.0.0.0:8484]
```

Durations parse the obvious things: `24h`, `7d`, `2w`, `90m`. Folders nest arbitrarily deep and inherit unread counts up the tree.

---

## 🌐 Server mode

Want one machine to be the source of truth and your laptop / phone-tunnel / second laptop to read from it? Drop a config file:

```toml
# ~/.rssdude/config.toml
[server]
address = "192.168.1.100:8484"
# token = "optional-bearer"
```

```bash
rssdude serve --bind 0.0.0.0:8484   # on the host
rssdude items --unread               # transparently proxies on the client
```

Same binary on both ends. No config = standalone mode, no behaviour change. The HTTP layer is a thin shell over the same command functions the CLI calls.

---

## 💾 Where things live

| Path | Purpose |
|------|---------|
| `~/.rssdude/rssdude.redb`   | The database — single file, easy to back up or move |
| `~/.rssdude/config.toml`    | Optional server / client config |
| `$RSSDUDE_DB_PATH`          | Override the DB location |

The DB is `native_db` on top of `redb` — embedded, typed, transactional. No SQL, no migrations folder, no daemon.

---

## 🧱 Built with

[`tokio`](https://tokio.rs) for async  ·  [`native_db`](https://github.com/vincent-herleworx/native_db) on [`redb`](https://github.com/cberner/redb) for storage  ·  [`ratatui`](https://ratatui.rs) for the TUI  ·  [`axum`](https://github.com/tokio-rs/axum) for the server  ·  [`feed-rs`](https://github.com/feed-rs/feed-rs) for parsing  ·  [`reqwest`](https://github.com/seanmonstar/reqwest) for fetching with conditional GETs  ·  [`tabled`](https://github.com/zhiburt/tabled) for table output  ·  [`html2text`](https://github.com/jugglerchris/rust-html2text) for the preview pane

---

## 🤝 Contributing

The codebase enforces a strict simplification pipeline (see `AGENTS.md`):

- ✋ no backwards-compatibility shims, ever
- 📚 prefer crates over hand-rolled utilities
- 🔁 duplicate logic gets extracted on sight
- 📏 functions stay under 40 lines, files prefer single responsibility
- 🧪 tests must catch real bugs, not assert tautologies

PRs that move us toward fewer lines doing the same job are the most welcome kind.

---

## 📄 License

MIT. Do whatever you want.
