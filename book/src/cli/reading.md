# Reading & search

## Browse items

```bash
rssdude items --limit 20                       # latest 20 across all feeds
rssdude items --unread                         # only unread
rssdude items --since 24h                      # last day
rssdude items --tag tech --unread              # filtered
rssdude items --folder <id> --unread           # folder + recurses subfolders
rssdude items --feed <id> --limit 50           # one feed
```

The default `--limit` is **20**. Bump it if you've got more unread than that and want them all in one shot.

## Read an item

```bash
rssdude read <item-id>                # plain-text rendering
rssdude read <item-id> --raw          # original HTML
rssdude read <item-id> --open         # open the source URL in $BROWSER
```

The default rendering pipes through `html2text`. If a particular post looks broken (mangled tables, weird newlines), `--raw` shows you the original markup so you can decide whether to file an issue or just open it in a browser.

## Search

```bash
rssdude search "transformers"
rssdude search "kernel panic" --limit 50
```

Full-text across titles and content. Returns latest-first. There's no fancy ranking — items are ordered by `published_at`.

## Chaining ids

When you need to feed an id from one command into another, use `--json` and `jq`:

```bash
ITEM_ID=$(rssdude items --unread --limit 1 --json | jq -r '.[0].id')
rssdude read $ITEM_ID
rssdude mark $ITEM_ID --read
```

The id format is short (8 chars, base62-ish), so you can also paste from a `list`/`items` table by hand if you prefer.
