# OPML import/export

OPML is the lingua franca of RSS subscription lists. Every reader exports it; `rssdude` round-trips it.

## Migrate from another reader

Export your subscriptions from the old reader (Feedly, Inoreader, Reeder — they all have an OPML export option), then:

```bash
rssdude opml import ~/Downloads/feedly.opml
```

That's the whole migration. The importer:

- creates folders matching the OPML's outline structure
- adds every feed under the right folder
- performs an initial fetch of each feed (you don't need to run `sync` after)
- skips duplicates if a feed URL already exists

Output looks like:

```text
Imported 47 feeds, 9 folders (3 feeds skipped as duplicates).
```

## Back up your subscriptions

```bash
rssdude opml export > rssdude-backup.opml
rssdude opml export --output rssdude-backup.opml
```

This exports feeds + folder hierarchy. It does **not** export marks (read/starred/notes) — those live only in the database file. To back those up, copy `~/.rssdude/rssdude.redb`. See [storage & backup](../storage.md).

## Round-trip

```bash
rssdude opml export > backup.opml
# ...later or on another machine:
rssdude opml import backup.opml
```

You'll get the same feed-and-folder structure back. Read state and stars don't ride along — that's what the redb file is for.
