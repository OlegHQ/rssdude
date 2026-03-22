# native_db API Reference

## Table of Contents
- [Database Methods](#database-methods)
- [Read Transaction (RTransaction)](#read-transaction)
- [Write Transaction (RwTransaction)](#write-transaction)
- [Get Operations](#get-operations)
- [Scan Operations](#scan-operations)
- [Len Operations](#len-operations)
- [Watch API](#watch-api)
- [Builder Options](#builder-options)
- [Error Handling](#error-handling)

## Database Methods

```rust
// Create/open file-based database
let db = Builder::new().create(&models, "path.redb")?;

// Open existing (fails if not exists)
let db = Builder::new().open("path.redb", &models)?;

// In-memory
let db = Builder::new().create_in_memory(&models)?;

// Maintenance
db.compact()?;
db.check_integrity()?;
```

## Read Transaction

```rust
let r: RTransaction = db.r_transaction()?;
// Available: r.get(), r.scan(), r.len()
// Concurrent readers allowed
// Dropped automatically (no commit needed)
```

## Write Transaction

```rust
let rw: RwTransaction = db.rw_transaction()?;
// Available: rw.get(), rw.scan(), rw.len() (reads within write tx)
// Mutations: rw.insert(), rw.upsert(), rw.auto_update(), rw.remove()
// Migration: rw.migrate::<T>()?, rw.convert_all::<Old, New>()?
rw.commit()?;  // or drop to abort
```

### Insert
```rust
rw.insert(item: T) -> Result<()>
```
Errors if primary key already exists.

### Upsert
```rust
rw.upsert(item: T) -> Result<Option<T>>
```
Returns `Some(old)` if key existed, `None` if fresh insert.

### Auto-Update (preferred over deprecated update)
```rust
rw.auto_update(item: T) -> Result<Option<T>>
```
Returns `Some(old)` if found and updated, `None` if not found. Does NOT insert if missing.

### Remove
```rust
rw.remove(item: T) -> Result<T>
```
Takes full item (not just key). Returns removed item.

### Migrate
```rust
rw.migrate::<NewVersion>() -> Result<()>
```
Converts all records from old version to new version using `From` impl.

### Convert All
```rust
rw.convert_all::<OldType, NewType>() -> Result<()>
```
Converts between different model IDs (not just versions).

## Get Operations

```rust
let r = db.r_transaction()?;

// Primary key lookup
r.get().primary::<T>(key) -> Result<Option<T>>

// Unique secondary key lookup (only for #[secondary_key(unique)])
r.get().secondary::<T>(TKey::field, key) -> Result<Option<T>>
```

## Scan Operations

All scans return iterators yielding `Result<T>`. Collect with `itertools::try_collect()`.

### Primary Scans
```rust
let r = db.r_transaction()?;

r.scan().primary::<T>()?.all()?          // all records
r.scan().primary::<T>()?.range(a..b)?    // range (supports .., ..b, a.., a..b, a..=b)
r.scan().primary::<T>()?.start_with(p)?  // prefix scan (String keys)
```

### Secondary Scans
```rust
r.scan().secondary::<T>(TKey::field)?.all()?          // all indexed
r.scan().secondary::<T>(TKey::field)?.equal(val)?     // exact match
r.scan().secondary::<T>(TKey::field)?.range(a..b)?    // range
r.scan().secondary::<T>(TKey::field)?.start_with(p)?  // prefix
```

### Iterator Features
```rust
// Reverse
iter.rev()

// Set operations (combine two secondary scan iterators)
iter_a.and(iter_b)  // intersection
iter_a.or(iter_b)   // union (deduped)
```

### Scan Within Write Transaction
```rust
let rw = db.rw_transaction()?;
// Same scan API available on rw
let items: Vec<T> = rw.scan().primary()?.all()?.try_collect()?;
```

## Len Operations

```rust
let r = db.r_transaction()?;
r.len().primary::<T>()? -> u64
r.len().secondary::<T>(TKey::field)? -> u64
```

## Watch API

Real-time change notifications via channels.

```rust
let watch = db.watch();

// Watch specific primary key
let recv = watch.get().primary::<T>(key)?;

// Watch specific secondary key
let recv = watch.get().secondary::<T>(TKey::field, value)?;

// Watch all changes to a type
let recv = watch.scan().primary::<T>()?.all()?;

// Watch with range/prefix
let recv = watch.scan().secondary::<T>(TKey::field)?.start_with("prefix")?;
```

Returns `std::sync::mpsc::Receiver<WatchEvent>` (or tokio channel with `tokio` feature).

## Builder Options

```rust
Builder::new()
    .create(&models, path)?      // create or open file DB
    .open(path, &models)?        // open existing only
    .create_in_memory(&models)?  // in-memory DB
```

## Error Handling

native_db uses `db_type::Error`. Common variants:
- Key already exists (on insert with duplicate PK)
- Item not found (on remove with missing item)
- Transaction errors

Wrap with `anyhow` for ergonomic propagation:
```rust
use anyhow::Context;
rw.insert(item).context("failed to insert item")?;
```
