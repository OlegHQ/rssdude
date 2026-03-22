# rssdude

Local-first RSS feed reader and content curation CLI built in Rust.

## Build

```bash
LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo build --release
```

Requires: `brew install libiconv` (keg-only, needed for native_model linker).

Binary: `target/release/rssdude`

## Code Philosophy

**Every line of code is a liability.** The goal is the smallest correct codebase that delivers the spec.

### NO BACKWARDS COMPATIBILITY. ONLY BREAKING CHANGES.

- Rename, delete, restructure freely. No shims, no deprecated wrappers, no `_old` suffixes.
- If a function signature should change, change it everywhere. Don't add a new function alongside the old one.
- If a type should change, change it. Don't keep the old type around.
- If an API endpoint changes shape, update `net/client.rs` to match. Don't version the API.
- Never add compatibility layers. The cost of breaking is zero; the cost of compat code is permanent.

Before writing code, exhaust these options in order:

1. **Use a library** — if a crate exists for the job, use it. Hand-rolling what a crate does is a bug, not a feature.
2. **Reuse an existing internal function** — search the codebase before writing anything. If similar logic exists, extract and share it.
3. **Write it once, in one place** — if you must write new code, put it in the most reusable location and call it from everywhere.

### Code Budget Rules

- **Penalize code generation.** If a change adds >50 new lines, justify why a library or existing abstraction can't do it. If a change adds >100 lines, it almost certainly needs a different approach.
- **Measure twice, cut once.** Before implementing, grep for existing solutions in the codebase. Duplicate code is the #1 enemy.
- **One function, one place.** If the same logic appears in 2+ files, extract it immediately. Never copy-paste with modifications — parameterize instead.
- **Server must call commands, not reimplement them.** `server.rs` handlers must delegate to `commands/` functions. Zero business logic in HTTP handlers.
- **Client is a thin HTTP wrapper.** `client.rs` methods should be generated or macro-driven, not hand-written per endpoint.

### Preferred Libraries (use these, don't hand-roll)

| Task | Crate | Replaces |
|------|-------|----------|
| Table output | `tabled` | Custom `print_table()` |
| Duration parsing | `humantime` or `duration-str` | Custom `parse_duration()` |
| HTML to text | `html2text` | Custom `strip_html()` |
| Stopwords | `stop-words` | Hardcoded `STOPWORDS` array |
| Tree operations | `indextree` | Manual BFS/recursive tree traversal |
| TUI snapshot testing | `insta` (dev-dep) | Manual visual verification |

When evaluating a new dependency: prefer crates with >1M downloads, recent maintenance, and minimal transitive deps. A 10-line dependency is better than a 10-line hand-rolled function because the crate handles edge cases you haven't thought of.

## Code Simplification Pipeline

When working on this codebase, run this checklist on every change:

### Step 1: Deduplication Scan
Before writing any new function, search the codebase:
```
grep -r "function_name_or_key_logic_phrase" src/
```
If similar logic exists anywhere, extract it to a shared location instead of writing new code.

**Known duplication hotspots** (FIXED — verify they stay deduplicated):
- `parse_datetime()` — ONLY in `shared/output.rs`. If you see it anywhere else, delete it.
- `collect_descendant_ids()` — ONLY in `shared/db.rs`. If you see it anywhere else, delete it.
- `entries_to_items()` — ONLY in `shared/feed.rs`. If you see entry→Item loops anywhere else, delete them.
- `compute_feed_stats()` — ONLY in `shared/db.rs`. Unread count calculation is centralized here.
- `strip_html()` — ONLY in `shared/output.rs`. If you see `html2text::from_read` anywhere else, replace with this.
- Server business logic — `net/server.rs` calls `*_core()` functions. If you see DB transactions in server.rs, extract to commands/.
- Digest/trending — TUI calls `commands::discover::*_core()`. If you see reimplemented word-frequency or digest logic in tui/, delete it.
- TUI sync — calls `sync_core()` with a progress callback. If you see duplicated fetch/store logic in tui/, delete it.

### Step 2: Library Replacement Check
For any hand-rolled utility function, ask: "Does a well-maintained crate do this?" If yes, replace.

### Step 3: Abstraction Extraction
If you see the same 3+ line pattern repeated, extract it. Common patterns to watch for:
- DB read transaction → query → collect → return
- HTTP request → deserialize → error handling
- Build table rows from model structs

### Step 4: Verify No Business Logic Leaks
- `net/server.rs` should only: parse HTTP request → call command function → serialize response
- `net/client.rs` should only: serialize request → HTTP call → deserialize response
- `main.rs` should only: parse CLI args → dispatch to command or client
- All business logic lives in `commands/` and shared helpers in `shared/`

### Step 5: Test Audit
Only useful tests. Delete tests that:
- Test trivial/obvious behavior (arithmetic, simple pattern matching, Option::map)
- Are tautologies (assert what the code literally does with no edge cases)
- Test implementation details rather than behavior
- Test what the type system already guarantees
- Are longer than the function they test with no edge case coverage
- Are redundant with other tests of the same function

A test is useful ONLY if:
- It tests interaction between multiple conditions (e.g., timing + identity)
- It tests multi-step transformations where ordering matters
- It tests edge cases that could realistically break
- It tests behavior that has regressed before or is error-prone
- Failure would indicate a real bug, not just "the code changed"

**Do not write tests for**: simple getters, trivial pattern matches, standard library wrappers, pure arithmetic, BFS/DFS on small trees, functions under 5 lines with obvious behavior.

### Step 5b: TUI Snapshot Verification
After ANY change to `src/tui/render.rs`, `src/tui/modals.rs`, or `src/tui/theme.rs`:

1. Run snapshot tests: `LIBRARY_PATH="/opt/homebrew/opt/libiconv/lib" cargo test tui::tests`
2. Review visual diffs: `cargo insta review`
3. Accept if correct, fix render code if not
4. Commit `.snap` files, never commit `.snap.new` files

See the `tui-snapshots` skill (`.claude/skills/tui-snapshots/SKILL.md`) for full guide on writing snapshot tests, mock fixtures, and color verification.

### Step 6: Line Count Check
After completing a change, verify:
- No function exceeds 40 lines (excluding struct definitions)
- Files should generally stay under 400 lines, but **coherent single responsibility beats arbitrary line limits**. A 500-line file with one clear purpose is better than two 250-line files that split a cohesive concept. Only split when a file genuinely has multiple independent concerns — never split for the sake of hitting a number.
- Total codebase stays as small as possible
- `cargo clippy` passes clean

## Reusable Abstractions

These shared helpers exist. Use them. Do not duplicate.

### Shared Helpers
```rust
// shared/output.rs — datetime/duration/formatting/html
output::parse_datetime(s: &str) -> Result<NaiveDateTime>    // THE ONLY datetime parser
output::parse_duration(s: &str) -> Result<Duration>          // "24h", "7d", "1w"
output::time_ago(iso: &str) -> String                        // relative time
output::strip_html(html: &str, width: usize) -> String      // THE ONLY HTML→text converter
output::print_table(headers, rows)                           // table output
output::print_json(data)                                     // JSON output

// shared/db.rs — folder tree operations + stats
db::collect_descendant_ids(folders, root_id) -> HashSet<String>  // BFS folder tree
db::compute_feed_stats(items, marks) -> HashMap<String, FeedStatsEntry>  // per-feed unread/starred

// shared/feed.rs — feed entry conversion
feed::entries_to_items(entries, feed_id, now) -> Vec<Item>   // THE ONLY entry→Item converter
```

### Core/CLI Split Pattern

Every command module exposes `*_core()` functions that return data, plus CLI wrappers that format output. Server.rs calls `*_core()` functions only.

```rust
// Pattern:
pub async fn foo_core(db, args) -> Result<FooResult> { /* business logic */ }
pub async fn foo(db, json, args) -> Result<()> {
    let result = foo_core(db, args).await?;
    if json { print_json(&result) } else { /* human output */ }
}

// server.rs handler:
async fn foo_handler(State(s), ...) -> ApiResult<Value> {
    let result = commands::foo_core(s.db, args).await?;
    Ok(Json(json!(result)))
}
```

## Architecture

- **Async runtime**: tokio — all I/O is non-blocking, TUI-ready
- **Database**: native_db (on redb) — embedded NoSQL with typed models and secondary indexes
- **DB access pattern**: `Arc<Database<'static>>` + `tokio::task::spawn_blocking` for all DB ops
- **HTTP**: reqwest (async) with conditional requests (ETag/If-Modified-Since)
- **Feed parsing**: feed-rs (RSS + Atom)
- **CLI**: clap derive with `--json` global flag
- **Server**: axum HTTP server — thin layer over command functions
- **Config**: toml — `~/.rssdude/config.toml` for server/client settings

## Performance Guidelines

- Keep DB transactions short — open, read/write, commit, drop
- Use secondary key scans with `.equal()` instead of full table scans when filtering by a known key
- Avoid holding `r_transaction` or `rw_transaction` across `.await` points — always inside `spawn_blocking`
- For sync, use conditional HTTP requests (304 Not Modified) to skip unchanged feeds
- When iterating items, apply filters early and break on limit to avoid scanning entire table
- Clone `Arc<Database>` cheaply — never clone the Database itself
- Folder tree unread counts roll up recursively — compute per-feed first, then aggregate

## Project Structure

```
src/
  main.rs              — clap CLI, command dispatch, client/server mode routing
  shared/              — core infrastructure shared across all layers
    db.rs              — models, DB setup, transaction wrappers, shared query helpers
    config.rs          — config.toml loading (server address, token, bind)
    feed.rs            — async HTTP fetch + feed-rs parsing + entries_to_items()
    output.rs          — print_table, print_json, time_ago, parse_duration, parse_datetime
  commands/            — business logic (core functions + CLI wrappers)
    feed_mgmt.rs       — add, list, remove (with --yes), move_to_folder
    folder.rs          — create, list (tree), rename, move_folder, delete
    sync.rs            — sync, status (with per-folder stats)
    read.rs            — items (with --folder filter), read_item (with --raw), search
    curate.rs          — mark, starred, export
    discover.rs        — digest (with --tag), trending, match_keywords
  tui/                 — ratatui interactive terminal UI
    mod.rs             — main event loop, app state, types
    state.rs           — state management, refresh, sync orchestration
    render.rs          — widget rendering, sidebar/items/preview panes
    input.rs           — keyboard/mouse input handling
    helpers.rs         — utility functions, text wrapping, filtering
    data.rs            — data loading, sync task, DB action wrappers
    modals.rs          — input/picker/confirm modal dialogs
    actions.rs         — action dispatch (digest, trending, mark, open)
  net/                 — HTTP server + client for remote access
    server.rs          — axum HTTP server, thin handlers that call commands/
    client.rs          — HTTP client, thin wrappers per endpoint
```

## Data Model (native_db)

Five models:

- **FeedV1** (id=1, v1): migration source — do not use directly
- **Feed** (id=1, v2): PK `id`, unique SK `url`, optional SK `folder_id`
- **Item** (id=2, v1): PK `id`, SK `feed_id`, unique SK `guid`
- **Mark** (id=3, v1): PK `item_id`
- **Folder** (id=4, v1): PK `id`, optional SK `parent_id`

Folders support unlimited nesting via `parent_id`. Feeds link to folders via `folder_id`.

DB stored at `~/.rssdude/rssdude.redb` — single file, easy to migrate. Override with `RSSDUDE_DB_PATH` env var.

## Server Mode

rssdude supports client/server mode for remote access to a shared database.

Config at `~/.rssdude/config.toml`:
```toml
[server]
address = "192.168.1.100:8484"   # if set, CLI becomes a thin client
# token = "optional-bearer-token"
# bind = "0.0.0.0:8484"          # override bind address for serve
```

- `rssdude serve [--bind 0.0.0.0:8484]` — starts the HTTP server, owns the DB
- Any CLI command with `address` configured — proxies to the remote server
- Without config or address — runs standalone (original behavior)

API endpoints mirror CLI commands: `/api/feeds`, `/api/items`, `/api/sync`, etc.

## Testing

```bash
# Folder hierarchy
rssdude folder create "Tech"
rssdude folder create "AI & ML" --parent <tech-id>
rssdude folder list

# Add feed into folder
rssdude add https://hnrss.org/frontpage --tag tech --folder <folder-id>

# Move feed between folders
rssdude move-feed <feed-id> --folder <folder-id>

# Filter items by folder (includes subfolders)
rssdude items --folder <folder-id> --limit 5

# Sync and check with per-folder stats
rssdude sync
rssdude status

# Search and curate
rssdude search "agents"
rssdude mark <id> --star --note "interesting"
rssdude starred

# Server mode
rssdude serve --bind 127.0.0.1:8484

# Client mode (with config.toml pointing to server)
rssdude list
rssdude sync
rssdude items --unread --limit 5
```
