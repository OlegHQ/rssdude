# Configuration

`~/.rssdude/config.toml`. Everything's optional — with no config file at all, you get sensible defaults and standalone mode.

## Full schema

```toml
# Server / client mode
[server]
bind = "0.0.0.0:8484"                  # server-side: where to listen
address = "homeserver.local:8484"      # client-side: where to send CLI commands
token = "shared-bearer-secret"         # both sides: optional auth

# TUI
[ui]
theme = "auto"                         # "dark" | "light" | "solarized" | "auto"

# Auto-cleanup
[retention]
auto_mark_read_after = "30d"           # auto-mark items older than this
auto_delete_after = "180d"             # delete items older than this
```

## Section: `[server]`

Controls multi-machine routing. See [server mode](server/setup.md) and [multi-machine setup](server/multi-machine.md).

| Field | Purpose |
|---|---|
| `bind` | The address the `serve` daemon listens on. Server-side only. |
| `address` | Where CLI commands are sent. **Presence flips this machine into client mode** — every command becomes an HTTP request. Client-side only. |
| `token` | Shared bearer token. If set, server rejects requests without it; client sends `Authorization: Bearer <token>`. |

## Section: `[ui]`

| Field | Purpose |
|---|---|
| `theme` | TUI palette: `dark`, `light`, `solarized`, or `auto` (OSC 11 detection — default). |

See [themes](tui/themes.md).

## Section: `[retention]`

Auto-cleanup runs after each `sync`. Useful for keeping the database from growing unbounded over years.

| Field | Purpose |
|---|---|
| `auto_mark_read_after` | Items older than this become read automatically. Doesn't delete. |
| `auto_delete_after` | Items older than this are removed from the database. Marks (read/star/note) are also removed for those items. |

Durations are `humantime` syntax: `30d`, `6mo`, `1y`. (Note: `mo` and `y` work in retention only — CLI flags accept the more conservative `30m`, `24h`, `7d`, `4w`.)

## Environment variables

| Var | Purpose |
|---|---|
| `RSSDUDE_DB_PATH` | Override the database file location. Default: `~/.rssdude/rssdude.redb`. Useful for scratch libraries, tests, or running multiple isolated databases. |
| `BROWSER` | Browser to launch on `read --open` and TUI `o` key. Default: OS default. |
| `LIBRARY_PATH` | macOS source-builds need `/opt/homebrew/opt/libiconv/lib` for the linker. The Makefile sets it; `cargo build` outside the Makefile won't. |

## Inspect the active config

`rssdude` doesn't expose a `config print` subcommand — TOML is short, just `cat ~/.rssdude/config.toml`. If a config field has a typo, the parser ignores it (no warning). Double-check spelling against the schema above if a setting doesn't seem to apply.
