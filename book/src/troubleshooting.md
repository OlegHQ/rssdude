# Troubleshooting

## Build fails on macOS with linker errors about `iconv`

```bash
brew install libiconv
```

`libiconv` is keg-only on macOS — it's installed but not on the linker path. The Makefile sets `LIBRARY_PATH` automatically; if you build with bare `cargo build`, export it yourself:

```bash
export LIBRARY_PATH=/opt/homebrew/opt/libiconv/lib
cargo build --release
```

## `rssdude` not found after install

Add `~/.cargo/bin` to your `PATH`. For zsh:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

## Feed says "0 items" after `add`

`add` performs an initial fetch on its own. If items still don't show up, the most likely causes are:

1. The URL is an HTML page, not an RSS/Atom feed. Check with `curl -sI <url>` — the content type should be `application/rss+xml`, `application/atom+xml`, or generic XML.
2. The feed is empty (some feeds publish only when there's news; not all are continuously active).
3. The feed returned a non-200 status. `rssdude status` will show the per-feed last-sync state.

## `sync` is silent but nothing changes

`sync` uses conditional GETs (`If-Modified-Since` / `If-None-Match`). When a feed responds 304 Not Modified, it's deliberately a no-op. The normal output:

```text
Syncing...
  Hacker News up to date
  Lobsters: 3 new items
Done. 3 new items.
```

"Up to date" means the server told us nothing changed; that's working as intended.

## `sync` errors on one feed

Per-feed errors don't stop other feeds from syncing. Failed feeds keep an error count; after 10 consecutive failures, the feed is auto-disabled. Use `rssdude status` to see error counts. The fix:

```bash
# Re-enable manually (any successful sync clears the counter)
rssdude sync --feed <id>
```

If it keeps failing, the URL probably moved — find the new one and `rssdude remove` + `rssdude add`.

## Server mode: "connection refused"

The client can't reach the server. Check, in order:

1. `ps aux | grep rssdude` on the server — is `serve` actually running?
2. Is the server bound to `0.0.0.0:8484`, not `127.0.0.1:8484`? With `127.0.0.1`, only the server box itself can connect.
3. `nc -vz <server-ip> 8484` from the client. If `nc` fails, it's a network/firewall issue, not `rssdude`.
4. macOS host firewall: allow inbound for the `rssdude` binary, or temporarily disable to test.

## Server mode: "401 Unauthorized" / "403 Forbidden"

Token mismatch. Ensure both `~/.rssdude/config.toml` files (server and client) have the same `[server] token = "..."`. The strings must match exactly.

## TUI looks wrong (colors, missing characters)

- **Colors look weird**: your terminal probably doesn't support truecolor. Test with `printf '\x1b[38;2;255;100;0mHello\x1b[0m\n'` — if you don't see orange, upgrade your terminal.
- **Missing box-drawing characters** (you see `?` or `□` instead of `┌`, `╭`, `├`): your font doesn't include those Unicode glyphs. Install a programming font like JetBrains Mono, Fira Code, or any Nerd Font.
- **Theme isn't auto-detecting your light terminal**: OSC 11 detection isn't supported by every terminal emulator. Pin it explicitly: `[ui] theme = "light"`.

## Database appears corrupted

`redb` is crash-safe — corruption is unlikely. If `rssdude` reports a DB error on startup, before doing anything destructive:

```bash
cp ~/.rssdude/rssdude.redb ~/.rssdude/rssdude.redb.broken
```

Then file an issue at the [GitHub repo](https://github.com/OlegHQ/rssdude/issues) with the error message. The DB is small enough that you can probably just send it.

To get back up and running fast, restore from your most recent backup, or:

```bash
rm ~/.rssdude/rssdude.redb
rssdude opml import last-backup.opml
```

(Requires you to have an OPML backup. See [storage & backup](storage.md).)

## `add` says "feed already exists"

A feed with that URL is already in the DB. `rssdude list | grep <domain>` to find the existing entry. If you want to re-add fresh, `remove` the old one first.

## "Folder not found" on `move-feed` or `add`

The folder id you passed doesn't exist. `rssdude folder list --json` to see the current set. Folder ids look like 8 char base62 strings; they're never the folder name.

## Where to file issues

[github.com/OlegHQ/rssdude/issues](https://github.com/OlegHQ/rssdude/issues). Include:

- `rssdude --version`
- the command that failed
- the error message (use `RUST_LOG=debug rssdude ...` for verbose output)
- whether you're running standalone or in client/server mode
