# Feeds & folders

## Add a feed

```bash
rssdude add https://hnrss.org/frontpage --tag tech
```

`add` performs an initial fetch on its own, so an explicit `sync` immediately after is redundant — it'll report "0 new". The fetch happens because we want to surface parse errors at add time rather than letting them sit until the next sync run.

You can pin a feed to a folder at add time:

```bash
TECH=$(rssdude folder create "Tech" --json | jq -r .id)
rssdude add https://lobste.rs/rss --tag tech --folder $TECH
```

`--tag` is repeatable: `--tag rust --tag systems`.

## List & remove

```bash
rssdude list                  # all feeds
rssdude list --tag tech       # filtered
rssdude remove <feed-id>      # interactive prompt
rssdude remove <feed-id> --yes  # skip the prompt
```

`--yes` matters in scripts and any non-interactive context — without it, `remove` prompts `[y/N]` on stdin and will appear to hang.

## Folders

Unlimited nesting via `--parent`:

```bash
TECH=$(rssdude folder create "Tech" --json | jq -r .id)
AI=$(rssdude folder create "AI & ML" --parent $TECH --json | jq -r .id)
LINUX=$(rssdude folder create "Linux" --parent $TECH --json | jq -r .id)

rssdude folder list
# Tech
# ├── AI & ML
# └── Linux
```

## Move feeds between folders

```bash
rssdude move-feed <feed-id> --folder $LINUX
```

## Filter items by folder

```bash
rssdude items --folder $TECH --limit 20
```

This **recurses into subfolders** — querying `Tech` returns items from feeds under `Tech/AI & ML` and `Tech/Linux` too. To get only-direct items from a specific subfolder, pass that subfolder's id.

Same recursion applies to `folder list` unread counts: `Tech (45 unread)` is the rolled-up total across everything below it.

## Delete a folder

```bash
rssdude folder delete <id>             # reparents children to root (or grandparent)
rssdude folder delete <id> --recursive # nukes the whole subtree
```

The default reparenting behavior is the safer choice — children survive, you just lose the grouping.
