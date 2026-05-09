# rssdude

A small, local-first RSS reader written in Rust. One binary, three ways to use it:

- a TUI for actually reading things (vim keys)
- a CLI for scripting (every command takes `--json`)
- an optional HTTP server, if you want one machine to hold the database and the rest to read from it

Everything lives in `~/.rssdude/`. No accounts, no cloud, nothing phones home.

![rssdude](demo/rssdude.gif)

If you'd rather scrub through it yourself: `asciinema play demo/rssdude.cast`.

## Why

Cloud readers keep dying. Self-hosted ones keep wanting a docker-compose, a Postgres, and a weekend. I wanted something that installs with one command, keeps my data in a single file, and stays out of the way.

The codebase is small on purpose — small enough to read end-to-end if you ever want to know what's happening to your feeds.

## Install

On Apple Silicon macs:

```bash
brew tap OlegHQ/tap
brew install rssdude
```

From source on any macOS:

```bash
brew install libiconv      # keg-only, the linker needs it
git clone https://github.com/OlegHQ/rssdude && cd rssdude
make install               # drops the binary in ~/.cargo/bin
```

On Linux:

```bash
git clone https://github.com/OlegHQ/rssdude && cd rssdude
cargo install --path .
```

Make sure `~/.cargo/bin` is on your `PATH`. Pass `make install PREFIX=/usr/local` if you'd rather it land somewhere else.

The other Make targets, for when you want them:

| Target | What it does |
|--------|--------------|
| `make build`     | Release build into `target/release/rssdude` |
| `make install`   | `cargo install` to `$(PREFIX)/bin` |
| `make uninstall` | Remove the installed binary |
| `make test`      | Unit + snapshot tests |
| `make clippy`    | Lint with `-D warnings` |
| `make run ARGS="items --unread"` | `cargo run` passthrough |

## Quick start

```bash
rssdude add https://hnrss.org/frontpage --tag tech
rssdude add https://blog.anthropic.com/rss --tag ai

rssdude sync
rssdude                          # opens the TUI

# or skip the TUI entirely:
rssdude items --unread --limit 10
rssdude search "transformer"
rssdude digest --since 24h
rssdude trending
```

Every command takes `--json`, so output pipes cleanly into `jq` and the rest of your shell.

## TUI keybindings

Three panes: sidebar, item list, preview.

| Key | Action |
|-----|--------|
| `j` / `k` | Down / up |
| `h` / `l` | Switch panes |
| `Tab` / `Shift-Tab` | Cycle panes |
| `1` / `3` | Toggle sidebar / preview |
| `Enter` | Open item, or descend into a pane |
| `Space` | Toggle read |
| `*` | Toggle star |
| `N` | Add or edit a note |
| `o` | Open link in `$BROWSER` |
| `/` | Search across all items |
| `r` | Sync everything |
| `s` | Sync the focused feed |
| `Ctrl-d` / `Ctrl-u` | Half-page scroll |
| `q` | Quit |

The mouse works too — click to focus a pane, click a link in the preview to open it, scroll wheel scrolls.

## CLI reference

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

Durations are what you'd expect: `24h`, `7d`, `2w`, `90m`. Folders nest as deep as you want and unread counts roll up the tree.

## Server mode

If you want one machine to hold the database and read from your laptop, a phone over a tunnel, or another box on the LAN, drop a config file:

```toml
# ~/.rssdude/config.toml
[server]
address = "192.168.1.100:8484"
# token = "optional-bearer"
```

Then:

```bash
rssdude serve --bind 0.0.0.0:8484    # on the host
rssdude items --unread                # on the client — talks to the server
```

Same binary on both sides. With no config, it stays standalone. The HTTP layer is a thin shell over the same command functions the CLI uses, so behaviour is the same wherever you run it.

## Theme

The TUI ships with three palettes: `dark`, `light`, and `solarized`. By default it queries the terminal background (OSC 11) and picks `dark` or `light` automatically. Override in `~/.rssdude/config.toml`:

```toml
[ui]
theme = "auto"   # or "dark" | "light" | "solarized"
```

## Where things live

| Path | Purpose |
|------|---------|
| `~/.rssdude/rssdude.redb`   | The database. One file — copy it to back up. |
| `~/.rssdude/config.toml`    | Optional server/client config |
| `$RSSDUDE_DB_PATH`          | Overrides the DB path if you want it somewhere else |

Storage is `native_db` on top of `redb` — embedded, typed, transactional. No SQL, no migration scripts, no daemon to babysit.

## Built with

`tokio`, `native_db` on `redb`, `ratatui`, `axum`, `feed-rs`, `reqwest` (with conditional GETs), `tabled`, `html2text`. Most of the work is done by crates that already exist; the rest is glue.

## Contributing

There's a simplification pipeline in `AGENTS.md`. The short version:

- no backwards-compatibility shims
- prefer a crate over a hand-rolled utility
- duplicated logic gets extracted
- functions stay under 40 lines
- tests should catch real bugs, not just restate the code

PRs that delete code while keeping behaviour are the most welcome kind.

## License

MIT.
