# Storage & backup

## Where things live

| Path | Purpose |
|---|---|
| `~/.rssdude/rssdude.redb` | The database. One file — copy it to back up. |
| `~/.rssdude/config.toml` | Optional config (server/client, theme, retention) |
| `$RSSDUDE_DB_PATH` | Overrides the DB path if you want it somewhere else |

## The database

`native_db` on top of `redb` — embedded, typed, transactional. No SQL, no migration scripts, no daemon to babysit. It's a single file; everything you've ever read, starred, or noted is in there.

The file format is forward-compatible. If a future version ever needs a migration, `rssdude` runs it on first launch.

## Backup

```bash
cp ~/.rssdude/rssdude.redb ~/Dropbox/rssdude-backup.redb
```

That's it. The DB is internally consistent at any moment (the redb format is crash-safe), so you can copy it while `rssdude` is running and the result will be valid.

For automated backups, anything that snapshots that file works:

- a daily `cron` / `launchd` job that copies it somewhere safe
- `restic` / `borg` / `tarsnap` pointing at `~/.rssdude/`
- Time Machine
- a Dropbox/iCloud-synced location (if you don't use server mode — sync conflicts on the live DB are a bad time)

## Restore

```bash
cp ~/Dropbox/rssdude-backup.redb ~/.rssdude/rssdude.redb
```

If `rssdude` is running, stop it first.

## Multiple databases

Use `RSSDUDE_DB_PATH` to point at a different file:

```bash
RSSDUDE_DB_PATH=~/work-feeds.redb rssdude items --unread
RSSDUDE_DB_PATH=~/personal-feeds.redb rssdude items --unread
```

Useful for keeping work and personal subscriptions separate without running a server.

## OPML alongside binary backups

Binary backups (`.redb`) preserve everything: feeds, folders, items, marks, notes, fetch state.

OPML exports preserve only feeds and folder structure — no items, no marks, no notes. Use OPML when migrating to or from another reader; use the redb file for actual backups.

```bash
# Subscriptions backup (portable, human-readable):
rssdude opml export --output subscriptions.opml

# Full backup (everything):
cp ~/.rssdude/rssdude.redb backup.redb
```

## Database location

If you want the DB outside the home directory (a network drive, a different disk, an encrypted volume), point `RSSDUDE_DB_PATH` at it permanently:

```bash
# in ~/.zshrc or ~/.bashrc
export RSSDUDE_DB_PATH=/Volumes/Encrypted/rssdude.redb
```

The directory must exist; `rssdude` creates the file but not its parent directory.
