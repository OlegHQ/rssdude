# CLI reference

Every command supports the global `--json` flag for machine-readable output.

## Feeds

```text
rssdude add <url> [--tag <tag>...] [--folder <id>]
rssdude list [--tag <tag>]
rssdude remove <id> [--yes]
rssdude move-feed <feed-id> --folder <folder-id>
```

## Sync & status

```text
rssdude sync [--feed <id>]
rssdude status                        # per-folder unread + last-sync table
rssdude stats [--since 30d] [--dead 5]
```

## Reading

```text
rssdude items   [--limit N] [--since 24h] [--tag <tag>]
                [--unread] [--feed <id>] [--folder <id>]
rssdude read    <id> [--open] [--raw]
rssdude search  <query> [--limit N]
```

## Curation

```text
rssdude mark    <id> [--read] [--star] [--note "..."]
rssdude starred [--limit N]
rssdude export  <id> [--format md|json|html]
```

## Discovery

```text
rssdude digest    [--since 24h] [--tag <tag>]
rssdude trending
rssdude match     <keywords> [--limit N] [--since 7d]
```

`<keywords>` is a comma-separated list: `rssdude match "rust,async,tokio"`.

## Folders

```text
rssdude folder create <name> [--parent <id>]
rssdude folder list                   # tree view
rssdude folder rename <id> <name>
rssdude folder move <id> --parent <id>
rssdude folder delete <id> [--recursive]
```

By default, `folder delete` reparents children to the deleted folder's parent. `--recursive` deletes the whole subtree.

## OPML

```text
rssdude opml import <file>
rssdude opml export [--output <file>]
```

Both feeds and the folder hierarchy round-trip.

## Server

```text
rssdude serve [--bind 0.0.0.0:8484]
```

See [server setup](../server/setup.md).

## TUI

```text
rssdude                                # no subcommand → TUI
```

## Durations

Anywhere a `--since` or `--limit` flag accepts a duration, `humantime` syntax works:

```text
30m   24h   2d   1w   30d
```

## Defaults to remember

| Command | Default |
|---|---|
| `digest --since` | 24h |
| `match --since` | 7d |
| `stats --since` | 30d, `--dead 5%` |
| `items --limit` | 20 |
| `starred --limit` | unbounded |
