---
name: rssdude
description: Operating guide for rssdude — the local-first RSS reader CLI in this repo. Use whenever the user asks about adding/syncing/reading feeds, organizing folders, curating with stars/notes, what's worth reading, dead/low-engagement feeds, OPML migration, or running rssdude in client/server mode across machines. Trigger phrases include "add this feed", "sync my RSS", "what's new", "what should I read", "dead feeds", "unsubscribe", "clean up my feeds", "rssdude server", "import OPML", or any reference to the user's personal feed reader.
---

# rssdude

`rssdude --help` enumerates every command and flag — read it for command names. This skill covers what help **doesn't** tell you.

Binary: `target/release/rssdude` (build with `LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo build --release` if missing). `--json` is a global flag — use it when chaining ids; drop it for human output.

## Behaviors that look like bugs but aren't

- **`add` and `opml import` already perform an initial fetch.** A follow-up `sync` is fine but redundant — it'll report "0 new". Don't chain them out of habit.
- **`sync` is cheap.** Uses ETag/If-Modified-Since; unchanged feeds 304 and are skipped. Run it freely.
- **`items --folder <id>` recurses into subfolders.** Querying a parent folder returns items from every nested feed. To get only-direct items, query each leaf folder yourself.
- **`folder delete` reparents children by default.** Subfolders and feeds inside the deleted folder move up to its parent (or root). Pass `--recursive` only when the user wants to nuke the whole subtree.
- **`remove` is interactive.** Prompts `[y/N]`. Pass `--yes` when running non-interactively or you'll appear to hang.
- **`mark` flags are additive, not toggles.** `--read` and `--star` only set to true; there's no flag to unstar or mark unread from the CLI.

## "What should I unsubscribe from?"

`stats --dead 5` lists feeds whose engagement is below 5% over the last 30 days. This is the right command for cleanup asks — it's buried in `stats --help` and easy to miss. Output ends with `Consider: rssdude remove <id>` so the next step is obvious.

## "What's worth reading?"

`digest [--since 24h]` is the right default — it clusters by feed and surfaces summaries. `trending` shows cross-feed topic frequency. `match "k1,k2"` filters by keywords (default 7d window). Reach for `items --unread` only when the others are too lossy.

## Defaults to know

`digest --since` 24h · `match --since` 7d · `stats --since` 30d, `--dead` 5% · `items --limit` 20.
Durations parse `humantime` style: `30m`, `24h`, `7d`, `1w`. When the user says "this week" → `7d`; "last month" → `30d`.

## Multi-machine: client/server mode

`~/.rssdude/config.toml` controls routing. The presence of `[server] address = ...` silently flips **every CLI command** into a thin HTTP client to that server.

```toml
# Server box (the one with the DB)
[server]
bind = "0.0.0.0:8484"        # listen on all interfaces; --bind on `serve` overrides

# Client boxes (laptop, etc.)
[server]
address = "homeserver.local:8484"   # send all CLI calls here
# token = "secret"                  # only if the server requires auth
```

On the server: `rssdude serve` (uses `bind` from config, or `--bind` flag).
On the client: any normal CLI command — it's transparently remote.
With no `[server]` block: standalone, uses local DB at `~/.rssdude/rssdude.redb`.

When the user reports "command timed out" / "connection refused", first check whether `address` is set and whether `serve` is actually running on the other end.

## Misc

- `RSSDUDE_DB_PATH=/path/to.redb` overrides the DB location — useful for scratch libraries or testing.
- Bare `rssdude` (no subcommand) launches the interactive TUI. Recommend it when the user wants to browse rather than query.
- If something isn't in `--help`, it doesn't exist. Don't invent flags.
