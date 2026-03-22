# rssdude

## Overview

Local-first RSS feed reader and content curation CLI tool built in Rust.

rssdude is a command-line tool for subscribing to RSS/Atom feeds, syncing content locally, and curating items for later use. All data lives in a local embedded database. There is no server, no account, no cloud sync. You own your feeds and your reading history.

The binary is called `rssdude`.

---

## Commands

All commands support `--json` for machine-readable output (see [Output Format](#output-format)).

---

### Feed Management

#### `rssdude add <url> [--tag <tag>]`

Subscribe to a feed. Fetches the feed immediately to resolve its title and description. The `--tag` flag can be repeated to apply multiple tags.

```
$ rssdude add https://blog.anthropic.com/rss --tag ai --tag research
Added feed: Anthropic Blog (https://blog.anthropic.com/rss)
Tags: ai, research
Synced 12 items.
```

| Flag | Description |
|------|-------------|
| `--tag <tag>` | Tag to associate with the feed. Repeatable. |

#### `rssdude remove <url|id>`

Unsubscribe from a feed. Accepts either the feed URL or its short ID. Removes all associated items and marks.

```
$ rssdude remove abc1
Removed feed: Anthropic Blog (12 items deleted)
```

Prompts for confirmation unless `--yes` is passed.

| Flag | Description |
|------|-------------|
| `--yes` | Skip confirmation prompt. |

#### `rssdude list [--tag <tag>]`

List all subscribed feeds. Optionally filter by tag.

```
$ rssdude list
ID    TITLE              URL                                    TAGS           ITEMS  LAST SYNCED
abc1  Anthropic Blog     https://blog.anthropic.com/rss         ai, research   42     2h ago
def2  Simon Willison     https://simonwillison.net/atom/all/    ai, python     118    2h ago
ghi3  Hacker News        https://hnrss.org/frontpage            tech           30     2h ago
```

| Flag | Description |
|------|-------------|
| `--tag <tag>` | Filter feeds by tag. |

---

### Syncing

#### `rssdude sync [--feed <id>]`

Fetch new items from all feeds, or a single feed if `--feed` is specified. Deduplicates by item URL.

```
$ rssdude sync
Syncing 3 feeds...
  Anthropic Blog       3 new items
  Simon Willison       7 new items
  Hacker News          12 new items
Done. 22 new items.
```

| Flag | Description |
|------|-------------|
| `--feed <id>` | Sync only the specified feed. |

#### `rssdude status`

Show sync status: when each feed was last synced, total item counts, and unread counts.

```
$ rssdude status
Feeds: 3
Items: 190 total, 34 unread, 5 starred
Last sync: 2026-03-15 08:12:00 (3h ago)

FEED               LAST SYNCED          ITEMS  UNREAD
Anthropic Blog     2026-03-15 08:12     42     3
Simon Willison     2026-03-15 08:12     118    19
Hacker News        2026-03-15 08:12     30     12
```

---

### Reading

#### `rssdude items [--limit N] [--since <date>] [--tag <tag>] [--unread]`

List items across all feeds. Items are sorted by publication date, newest first.

```
$ rssdude items --unread --limit 5
ID      SOURCE            TITLE                                      PUBLISHED
a1b2c3  Anthropic Blog    Claude's new tool use capabilities         10m ago
d4e5f6  Simon Willison    Weeknotes: datasette-enrichments           2h ago
g7h8i9  Hacker News       Show HN: I built a local-first RSS tool    3h ago
j0k1l2  Simon Willison    Building AI agents with tool use           5h ago
m3n4o5  Anthropic Blog    Research update: constitutional AI v2      8h ago
```

| Flag | Description |
|------|-------------|
| `--limit N` | Maximum number of items to show. Default: 20. |
| `--since <date>` | Only items published after this date. Accepts ISO 8601 dates or relative durations like `24h`, `7d`, `1w`. |
| `--tag <tag>` | Only items from feeds with this tag. |
| `--unread` | Only items not yet marked as read. |
| `--feed <id>` | Only items from the specified feed. |

#### `rssdude read <item-id>`

Display the full content or summary of a single item. Automatically marks the item as read.

```
$ rssdude read a1b2c3
Anthropic Blog
Claude's new tool use capabilities
Published: 2026-03-10 12:00 UTC
URL: https://blog.anthropic.com/claude-tool-use

We're announcing new capabilities for Claude's tool use system. The updated
architecture allows Claude to chain multiple tool calls in a single turn,
reducing latency and improving reliability for complex workflows...

[Content truncated. Open URL with --open]
```

| Flag | Description |
|------|-------------|
| `--open` | Open the item URL in the default browser. |
| `--raw` | Show raw HTML content instead of rendered text. |

#### `rssdude search <query> [--limit N]`

Search across item titles, content, and summaries in the local store.

```
$ rssdude search "tool use" --limit 3
ID      SOURCE            TITLE                                      PUBLISHED
a1b2c3  Anthropic Blog    Claude's new tool use capabilities         10m ago
j0k1l2  Simon Willison    Building AI agents with tool use           5h ago
x9y8z7  Hacker News       Comparing tool use across LLM providers    2d ago
```

| Flag | Description |
|------|-------------|
| `--limit N` | Maximum results. Default: 10. |

---

### Curation

#### `rssdude mark <item-id> --read`

Mark an item as read.

```
$ rssdude mark a1b2c3 --read
Marked as read: Claude's new tool use capabilities
```

#### `rssdude mark <item-id> --star`

Star an item for later review. Toggles: run again to unstar.

```
$ rssdude mark a1b2c3 --star
Starred: Claude's new tool use capabilities
```

#### `rssdude mark <item-id> --note "..."`

Attach a free-text note to an item. Overwrites any existing note.

```
$ rssdude mark a1b2c3 --note "good angle for LinkedIn post on tool use patterns"
Note added: Claude's new tool use capabilities
```

Multiple flags can be combined in a single call:

```
$ rssdude mark a1b2c3 --read --star --note "write about this"
Marked as read, starred, note added: Claude's new tool use capabilities
```

#### `rssdude starred [--limit N]`

Show all starred items, newest first.

```
$ rssdude starred
ID      SOURCE            TITLE                                      NOTE                              PUBLISHED
a1b2c3  Anthropic Blog    Claude's new tool use capabilities         good angle for LinkedIn post...   10m ago
p6q7r8  Simon Willison    The future of local-first software         compare with rssdude approach     3d ago
```

| Flag | Description |
|------|-------------|
| `--limit N` | Maximum results. Default: all starred items. |

#### `rssdude export <item-id> --format md`

Export an item's content for use in drafting. Writes to stdout.

```
$ rssdude export a1b2c3 --format md
# Claude's new tool use capabilities

**Source:** Anthropic Blog
**Published:** 2026-03-10
**URL:** https://blog.anthropic.com/claude-tool-use
**Note:** good angle for LinkedIn post on tool use patterns

---

We're announcing new capabilities for Claude's tool use system. The updated
architecture allows Claude to chain multiple tool calls in a single turn,
reducing latency and improving reliability for complex workflows...
```

| Flag | Description |
|------|-------------|
| `--format <fmt>` | Output format. Supported: `md` (Markdown), `json`, `text`. Default: `md`. |

Pipe to a file for drafting: `rssdude export a1b2c3 --format md > draft.md`

---

### Discovery

#### `rssdude digest [--since 24h]`

Produce a summary of new items grouped by feed, with counts.

```
$ rssdude digest --since 24h
Digest: 22 new items since 2026-03-14 11:00

Anthropic Blog (3 new)
  - Claude's new tool use capabilities (10m ago)
  - Research update: constitutional AI v2 (8h ago)
  - Hiring: applied AI team (20h ago)

Simon Willison (7 new)
  - Weeknotes: datasette-enrichments (2h ago)
  - Building AI agents with tool use (5h ago)
  - ...and 5 more

Hacker News (12 new)
  - Show HN: I built a local-first RSS tool (3h ago)
  - ...and 11 more
```

| Flag | Description |
|------|-------------|
| `--since <duration>` | Time window. Default: `24h`. Accepts `12h`, `7d`, `1w`, etc. |
| `--tag <tag>` | Only include feeds with this tag. |

#### `rssdude trending`

Identify topics that appear across multiple feeds within the last 48 hours. Uses term-frequency analysis on titles and content.

```
$ rssdude trending
TOPIC              MENTIONS  FEEDS  LATEST
tool use           5         3      10m ago
local-first        3         2      3h ago
constitutional ai  2         2      8h ago
rust async         4         2      1d ago
```

#### `rssdude match <keywords>`

Find items matching any of the provided comma-separated keywords. Unlike `search`, this is optimized for multi-keyword scanning across recent items.

```
$ rssdude match "agents,claude,cursor"
ID      SOURCE            TITLE                                      MATCHED     PUBLISHED
a1b2c3  Anthropic Blog    Claude's new tool use capabilities         claude      10m ago
j0k1l2  Simon Willison    Building AI agents with tool use           agents      5h ago
s1t2u3  Hacker News       Cursor 1.0 released with agent mode        cursor      1d ago
```

| Flag | Description |
|------|-------------|
| `--limit N` | Maximum results. Default: 20. |
| `--since <duration>` | Time window to search within. Default: `7d`. |

---

## Data Model

All data is stored in a local `native_db` database backed by `redb`.

### `FeedV1`

- Migration source only.
- Retained to migrate older databases forward.

### `Feed`

- Primary key: `id`
- Unique secondary key: `url`
- Optional secondary key: `folder_id`
- Stores title, description, tags, sync metadata, and folder placement.

### `Item`

- Primary key: `id`
- Secondary key: `feed_id`
- Unique secondary key: `guid`
- Stores title, link, content, summary, publication time, and fetch time.

### `Mark`

- Primary key: `item_id`
- Stores read state, star state, note, and mark timestamp.

### `Folder`

- Primary key: `id`
- Optional secondary key: `parent_id`
- Supports unlimited folder nesting.

---

## Output Format

All commands default to human-readable table output suitable for terminal use.

Passing `--json` to any command switches to JSON output. List commands emit a JSON array. Single-item commands emit a JSON object.

### Example: Item JSON

```json
{
  "id": "abc123",
  "title": "Claude's new tool use capabilities",
  "url": "https://blog.anthropic.com/claude-tool-use",
  "source": "Anthropic Blog",
  "published": "2026-03-10T12:00:00Z",
  "summary": "We're announcing new capabilities for Claude's tool use system...",
  "tags": ["ai", "research"],
  "read": false,
  "starred": false
}
```

### Example: Feed JSON

```json
{
  "id": "abc1",
  "url": "https://blog.anthropic.com/rss",
  "title": "Anthropic Blog",
  "description": "Updates from Anthropic",
  "tags": ["ai", "research"],
  "item_count": 42,
  "unread_count": 3,
  "added_at": "2026-01-15T09:00:00Z",
  "last_synced": "2026-03-15T08:12:00Z"
}
```

---

## Storage

- Database location: `~/.rssdude/rssdude.redb`
- Auto-created on first run of any command.
- Override with `RSSDUDE_DB_PATH` environment variable.
- Managed by `native_db` on top of `redb`.

---

## Configuration

No configuration file is required. All behavior is controlled via command flags and environment variables.

| Variable | Description |
|----------|-------------|
| `RSSDUDE_DB_PATH` | Override default database path. |
| `NO_COLOR` | Disable colored output (respects the [NO_COLOR](https://no-color.org/) convention). |

---

## Workflow Example

A typical daily content curation workflow:

```bash
# 1. Morning: sync feeds and see what's new
rssdude sync && rssdude digest --since 24h

# 2. Find angles for content you're working on
rssdude match "agents,claude,cursor"

# 3. Star interesting items with notes
rssdude mark a1b2c3 --star --note "good for LinkedIn post on tool use"
rssdude mark j0k1l2 --star --note "compare with our agent architecture"

# 4. Later: review everything you starred
rssdude starred

# 5. Export an item to start drafting
rssdude export a1b2c3 --format md > draft.md
```

---

## Error Handling

- Network errors during sync are reported per-feed; other feeds continue.
- Invalid feed URLs produce a clear error with the HTTP status or parse failure.
- Unknown item or feed IDs return a "not found" error with exit code 1.
- All errors go to stderr. Normal output goes to stdout.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success. |
| 1 | General error (invalid input, not found, etc.). |
| 2 | Network error (unreachable feed, timeout). |
