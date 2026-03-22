---
name: native-db
description: Guide for using native_db (Rust embedded database on redb) with typed models, primary/secondary keys, transactions, scans, and async patterns via spawn_blocking. Use when writing Rust code that uses native_db, native_model, or redb for embedded storage — including model definitions, CRUD operations, queries, indexes, migrations, and database setup.
---

# native_db — Rust Embedded Database

Typed embedded NoSQL database for Rust, built on redb. Models use derive macros with primary/secondary key attributes. All operations go through explicit read or read-write transactions.

## Dependencies

```toml
native_db = "0.8"
native_model = "0.4"
serde = { version = "1", features = ["derive"] }
once_cell = "1"
```

Add `itertools = "0.12"` if using scan iterators (for `.try_collect()`).

## Model Definition

```rust
use native_db::*;
use native_model::native_model;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 1, version = 1)]
#[native_db]
struct Item {
    #[primary_key]
    id: String,

    #[secondary_key]
    feed_id: String,

    #[secondary_key(unique)]
    guid: String,

    #[secondary_key(optional)]
    tag: Option<String>,

    title: String,
}
```

Key attributes:
- `#[primary_key]` — exactly one per struct
- `#[secondary_key]` — non-unique index, multiple allowed
- `#[secondary_key(unique)]` — unique index, enables `.get().secondary()`
- `#[secondary_key(optional)]` — for `Option<T>`, `None` values skipped in index
- `#[native_model(id = N, version = N)]` — each model needs unique `id`, increment `version` for migrations

The macro generates `ItemKey::feed_id`, `ItemKey::guid`, `ItemKey::tag` enum variants for secondary lookups.

## Database Setup

```rust
use once_cell::sync::Lazy;

static MODELS: Lazy<Models> = Lazy::new(|| {
    let mut models = Models::new();
    models.define::<Feed>().unwrap();
    models.define::<Item>().unwrap();
    models.define::<Mark>().unwrap();
    models
});

// File-based
let db = Builder::new().create(&MODELS, "path/to/db.redb")?;

// In-memory (tests)
let db = Builder::new().create_in_memory(&MODELS)?;
```

Note: `create()` creates or opens. Register ALL model types before opening.

## CRUD Operations

All writes require `rw_transaction()` + explicit `commit()`. Reads use `r_transaction()`.

```rust
// INSERT (errors if PK exists)
let rw = db.rw_transaction()?;
rw.insert(item)?;
rw.commit()?;

// UPSERT (insert or replace, returns old value)
let rw = db.rw_transaction()?;
let old: Option<Item> = rw.upsert(item)?;
rw.commit()?;

// UPDATE (by primary key, returns old value, None if not found)
let rw = db.rw_transaction()?;
let old: Option<Item> = rw.auto_update(updated_item)?;
rw.commit()?;

// REMOVE (takes full item, returns removed)
let rw = db.rw_transaction()?;
let removed: Item = rw.remove(item)?;
rw.commit()?;
```

## Read Operations

```rust
let r = db.r_transaction()?;

// By primary key
let item: Option<Item> = r.get().primary("abc123")?;

// By unique secondary key
let item: Option<Item> = r.get().secondary(ItemKey::guid, "some-guid")?;

// Count
let n: u64 = r.len().primary::<Item>()?;
```

## Scan Operations

Scans return iterators. Use `itertools::Itertools::try_collect()` to collect.

```rust
use itertools::Itertools;
let r = db.r_transaction()?;

// All items
let all: Vec<Item> = r.scan().primary()?.all()?.try_collect()?;

// Range
let range: Vec<Item> = r.scan().primary()?.range("a".."z")?.try_collect()?;

// By secondary key — all with that index
let by_feed: Vec<Item> = r.scan().secondary(ItemKey::feed_id)?.all()?.try_collect()?;

// Secondary key — exact match
let feed_items: Vec<Item> = r.scan().secondary(ItemKey::feed_id)?.equal("feed1")?.try_collect()?;

// Secondary key — prefix
let prefixed: Vec<Item> = r.scan().secondary(ItemKey::feed_id)?.start_with("tech")?.try_collect()?;

// Reverse iteration
let newest: Vec<Item> = r.scan().primary()?.all()?.rev().try_collect()?;

// Set operations (AND / OR across secondary scans)
let both: Vec<Item> = r.scan().secondary(ItemKey::feed_id)?.equal("f1")?
    .and(r.scan().secondary(ItemKey::tag)?.equal("ai")?)
    .try_collect()?;
```

## Async Pattern (tokio spawn_blocking)

native_db is sync. Wrap in `spawn_blocking` for async contexts:

```rust
use std::sync::Arc;

let db = Arc::new(Builder::new().create(&MODELS, path)?);

// Read
let db2 = db.clone();
let items: Vec<Item> = tokio::task::spawn_blocking(move || {
    let r = db2.r_transaction()?;
    let items: Vec<Item> = r.scan().primary()?.all()?.try_collect()?;
    Ok::<_, anyhow::Error>(items)
}).await??;

// Write
let db2 = db.clone();
tokio::task::spawn_blocking(move || {
    let rw = db2.rw_transaction()?;
    rw.upsert(item)?;
    rw.commit()?;
    Ok::<_, anyhow::Error>(())
}).await??;
```

## Model Versioning

For schema migrations, keep old model, create new version with `from`:

```rust
#[derive(Serialize, Deserialize, Clone)]
#[native_model(id = 1, version = 2, from = ItemV1)]
#[native_db]
struct ItemV2 {
    #[primary_key]
    id: String,
    #[secondary_key]
    feed_id: String,
    new_field: String,  // added field
}

impl From<ItemV1> for ItemV2 { /* ... */ }
impl From<ItemV2> for ItemV1 { /* ... */ }

// Register both, then migrate
models.define::<ItemV1>().unwrap();
models.define::<ItemV2>().unwrap();
let rw = db.rw_transaction()?;
rw.migrate::<ItemV2>()?;
rw.commit()?;
```

## Types That Implement ToKey

`u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64, String, &str, Vec<u8>, &[u8]`

## Full API reference

See [references/api_reference.md](references/api_reference.md) for complete method signatures, watch API, and advanced patterns.
