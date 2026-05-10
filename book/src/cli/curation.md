# Curation

## Mark as read, starred, or with a note

```bash
rssdude mark <item-id> --read
rssdude mark <item-id> --star
rssdude mark <item-id> --note "follow up next week"
rssdude mark <item-id> --read --star --note "..."   # all in one call
```

`mark` flags are **additive**, not toggles — passing `--star` sets it to true; there's no flag to unstar from the CLI. (You can unstar from the TUI.)

`--star` and `--read` are independent — starring does not auto-mark read.

Marks (read/starred/note) are stored separately from items, so a feed re-sync never wipes them.

## See what you've starred

```bash
rssdude starred                # latest first, all of them
rssdude starred --limit 20     # cap the output
```

## Export an article

```bash
rssdude export <item-id> --format md > article.md
rssdude export <item-id> --format html > article.html
rssdude export <item-id> --format json > article.json
```

`md` is the default and is the right call when you're saving to a notes app (Obsidian, Bear, Notion-via-clipboard). `html` keeps the original markup. `json` includes the full record (id, feed, tags, mark state) — useful for piping into other tools.

## Bulk star / mark from the TUI

The CLI doesn't have a bulk-mark mode, but the TUI does:

- `v` enters visual mode
- `j`/`k` extend the selection
- `Space` marks all selected as read
- `*` stars all selected

See [TUI keybindings](../tui/keybindings.md).
