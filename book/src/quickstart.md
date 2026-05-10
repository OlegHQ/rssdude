# Quick start

## Add a feed and sync

```bash
rssdude add https://hnrss.org/frontpage --tag tech
rssdude add https://blog.anthropic.com/rss --tag ai
rssdude sync
```

`rssdude add` performs an initial fetch on its own — the explicit `sync` after is optional, but it's cheap (conditional GETs return 304 for unchanged feeds) so it's habit.

## Open the TUI

```bash
rssdude
```

Three panes: sidebar, item list, preview. Vim keys work (`j`/`k` to scroll, `h`/`l` to switch panes), mouse works too. See [TUI keybindings](tui/keybindings.md) for the full set.

Press `q` to quit.

## Or stay on the CLI

```bash
rssdude items --unread --limit 10        # what's new
rssdude search "transformers"             # full-text across the archive
rssdude digest --since 24h                # clustered rundown of the last day
rssdude trending                          # cross-feed topic frequency
```

## Curate

```bash
ITEM_ID=$(rssdude items --unread --limit 1 --json | jq -r '.[0].id')
rssdude mark $ITEM_ID --star --note "follow up"
rssdude starred                            # what you've starred
```

## Organize

```bash
TECH=$(rssdude folder create "Tech" --json | jq -r .id)
rssdude folder create "AI & ML" --parent $TECH
rssdude move-feed <feed-id> --folder $TECH
rssdude folder list                        # tree view
```

Folders nest as deep as you want. Unread counts roll up the tree.

## Where to go next

- The full [CLI reference](cli/reference.md) — every subcommand and flag.
- [Server mode](server/setup.md) — run `rssdude` on one box, read from your laptop.
- [TUI themes](tui/themes.md) — auto-detected dark/light + a Solarized preset.
- [Backup](storage.md) — copy a single file to back up everything.
