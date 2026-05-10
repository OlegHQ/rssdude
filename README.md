# rssdude

A small, local-first RSS reader written in Rust. One binary; TUI for reading, CLI for scripting, optional HTTP server if you want one box to hold the database.

Everything lives in `~/.rssdude/`. No accounts, no cloud, nothing phones home.

![rssdude](demo/rssdude.gif)

## Install

Apple Silicon mac, the easy way:

```bash
brew tap OlegHQ/tap
brew install rssdude
```

From source on macOS or Linux:

```bash
brew install libiconv     # macOS only — keg-only, the linker needs it
git clone https://github.com/OlegHQ/rssdude && cd rssdude
make install              # drops the binary in ~/.cargo/bin
```

Make sure `~/.cargo/bin` is on your `PATH`.

## Quick start

```bash
rssdude add https://hnrss.org/frontpage --tag tech
rssdude sync
rssdude                            # open the TUI
```

Or skip the TUI:

```bash
rssdude items --unread --limit 10
rssdude digest --since 24h
rssdude trending
```

Every command takes `--json`. Durations are `humantime` style: `24h`, `7d`, `2w`, `90m`.

## Documentation

The full guide lives at **[oleghq.github.io/rssdude](https://oleghq.github.io/rssdude)** — install, TUI keybindings, every CLI flag, multi-machine server setup, OPML migration, themes, backup, troubleshooting.

You can also build it locally:

```bash
cargo install mdbook
mdbook serve book   # http://localhost:3000
```

## Built with

`tokio`, `native_db` on `redb`, `ratatui`, `axum`, `feed-rs`, `reqwest` (with conditional GETs), `tabled`, `html2text`. Most of the work is done by crates that already exist; the rest is glue.

## Contributing

There's a simplification pipeline in `AGENTS.md`. The short version: no backwards-compatibility shims, prefer a crate over a hand-rolled utility, duplicated logic gets extracted, functions stay under 40 lines. PRs that delete code while keeping behaviour are the most welcome kind.

## License

MIT.
