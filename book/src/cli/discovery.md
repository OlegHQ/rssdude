# Discovery

Four commands that all answer some flavor of "what's worth reading?" Pick by intent.

## `digest` — clustered rundown

```bash
rssdude digest                           # last 24h
rssdude digest --since 7d                # this week
rssdude digest --since 7d --tag tech     # this week, tech only
```

Default window is **24h**. Items are clustered per feed with summaries — this is the right command for the open-ended "give me a summary of what's new" ask.

## `trending` — cross-feed topic frequency

```bash
rssdude trending
```

Surfaces topics that appear across multiple feeds in the last week. Useful when you want to know "what's everyone talking about" rather than "what's new in my feeds".

The output is thin if your subscription set is small (the algorithm needs overlap to detect trends). Two or three feeds rarely produce useful trending output.

## `match` — keyword filter

```bash
rssdude match "rust,async,tokio"          # default 7d window
rssdude match "kernel,exploit" --since 30d --limit 50
```

Comma-separated keywords. Returns items whose title or summary contains any of them. Good for "show me anything mentioning X" without having to scan everything.

## `items --unread` — raw inbox

```bash
rssdude items --unread --since 24h
```

The flat list. Use this when the others are too lossy — for example when you want the *full* list of what changed, not a curated subset.

## `stats` — what you've actually been reading

Not strictly discovery, but worth knowing:

```bash
rssdude stats                          # last 30d, all-up
rssdude stats --since 7d
rssdude stats --dead 5                 # surface feeds with <5% engagement
```

`stats --dead` is the right command for "what should I unsubscribe from" — it's specifically designed to flag low-engagement feeds. Output ends with a `Consider: rssdude remove <id>` hint.

If you've just installed `rssdude`, give it a couple of weeks of read activity before trusting `stats --dead` — with no read history, every feed flags as 0% engagement.
