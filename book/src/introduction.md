# Introduction

`rssdude` is a small, local-first RSS reader written in Rust. One binary, three ways to use it:

- a **TUI** for actually reading things (vim keys, mouse works too)
- a **CLI** for scripting (every command takes `--json`)
- an optional **HTTP server**, if you want one machine to hold the database and the rest to read from it

Everything lives in `~/.rssdude/`. No accounts, no cloud, nothing phones home.

## Why

Cloud readers keep dying. Self-hosted ones keep wanting a `docker-compose`, a Postgres, and a weekend. `rssdude` installs with one command, keeps your data in a single file, and stays out of the way.

The codebase is small on purpose — small enough to read end-to-end if you ever want to know what's happening to your feeds.

## What's in this book

- [Install](install.md) and [quick start](quickstart.md) — get reading in 30 seconds.
- [CLI reference](cli/reference.md) — every subcommand, every flag, with worked examples.
- [TUI keybindings](tui/keybindings.md) — the keys you'll use 95% of the time.
- [Server mode](server/setup.md) — run `rssdude` on one box, read from your laptop.
- [Configuration](config.md), [storage](storage.md), [troubleshooting](troubleshooting.md) — the rest.

## Built with

`tokio`, `native_db` on `redb`, `ratatui`, `axum`, `feed-rs`, `reqwest` (with conditional GETs), `tabled`, `html2text`. Most of the work is done by crates that already exist; the rest is glue.

## Source

Everything lives at [github.com/OlegHQ/rssdude](https://github.com/OlegHQ/rssdude). Issues and PRs welcome — see the contributing notes in `AGENTS.md`.
