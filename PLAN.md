# rssdude — Implementation Plan

## Project Structure

```
rssdude/
  Cargo.toml
  SPEC.md
  PLAN.md
  src/
    main.rs          — Entry point, clap CLI definition, command dispatch
    db.rs            — native_db models, database setup, query helpers
    feed.rs          — HTTP fetching + feed-rs parsing
    output.rs        — Unified output formatting (table vs JSON)
    commands/
      mod.rs         — Re-exports
      feed_mgmt.rs   — add, list, remove
      sync.rs        — sync, status
      read.rs        — items, read, search
      curate.rs      — mark, starred, export
      discover.rs    — digest, trending, match
```

## Dependencies

- clap 4 (derive) — CLI parsing
- native_db 0.8 + native_model 0.4 — embedded NoSQL (on redb)
- feed-rs 2 — RSS/Atom parsing
- reqwest 0.12 (async, rustls-tls) — HTTP
- tokio (full) — async runtime
- serde + serde_json — JSON output
- chrono — timestamps
- anyhow — error handling
- nanoid — short IDs
- itertools — scan iterator collection
- once_cell — static model registry

## Data Model

3 native_db models: Feed (id=1), Item (id=2), Mark (id=3)
All DB access via Arc<Database<'static>> + spawn_blocking

## Implementation Phases

1. **Skeleton + Storage**: Cargo.toml, main.rs with clap, db.rs with migrations, output.rs
2. **Feed Management**: add, list, remove + feed.rs for fetching/parsing
3. **Syncing**: sync, status
4. **Reading**: items, read, search
5. **Curation**: mark, starred, export
6. **Discovery**: digest, trending, match

## Key Decisions

- Fully async (tokio + reqwest async) — TUI-ready architecture
- native_db on redb — typed embedded NoSQL, no SQL strings
- DB ops via spawn_blocking — keeps async runtime unblocked
- nanoid for short human-friendly IDs
- Mark model with read/starred/note fields
- upsert for dedup on sync (returns None if new insert)
- --json as global flag via output.rs
- DB at ~/.rssdude/rssdude.redb, overridable with RSSDUDE_DB_PATH
- Conditional HTTP requests (ETag/Last-Modified) for efficient sync
