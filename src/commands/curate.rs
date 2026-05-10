use std::sync::Arc;

use anyhow::{Context, Result};
use native_db::Database;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

/// Core: apply mark flags to an item, returns (Item, Mark).
///
/// - `read`: None = no change, Some(v) = set read to v
/// - `star`: None = no change, Some(v) = set starred to v
/// - `note`: None = no change, Some("") = clear note, Some(s) = set note
pub async fn mark_core(
    db: Arc<Database<'static>>,
    id: String,
    read: Option<bool>,
    star: Option<bool>,
    note: Option<String>,
) -> Result<(Item, Mark)> {
    spawn_blocking(move || -> Result<(Item, Mark)> {
        let rw = db.rw_transaction()?;
        let item: Item = rw.get().primary(id.clone())?
            .with_context(|| format!("item {id} not found"))?;
        let existing: Option<Mark> = rw.get().primary(id.clone()).ok().flatten();

        let was_read = existing.as_ref().is_some_and(|m| m.read);
        let mut new_mark = Mark::from_existing(id, existing.as_ref());
        if let Some(v) = read { new_mark.read = v; }
        if let Some(v) = star { new_mark.starred = v; }
        if let Some(n) = note { new_mark.note = if n.is_empty() { None } else { Some(n) }; }
        if new_mark.read && !was_read { new_mark.read_at = Some(new_mark.marked_at.clone()); }
        let _: Option<Mark> = rw.upsert(new_mark.clone())?;
        rw.commit()?;
        Ok((item, new_mark))
    }).await?
}

pub async fn mark(db: Arc<Database<'static>>, json: bool, id: String, read: bool, star: bool, note: Option<String>) -> Result<()> {
    // CLI: read/star are additive flags (set-only, never toggle-off)
    let (item, new_mark) = mark_core(db, id, read.then_some(true), star.then_some(true), note).await?;
    let title = item.title.as_deref().unwrap_or("(untitled)");
    if json {
        print_json(&new_mark);
    } else {
        let mut parts = Vec::new();
        if new_mark.read { parts.push("read"); }
        if new_mark.starred { parts.push("starred"); }
        if new_mark.note.is_some() { parts.push("noted"); }
        let status = if parts.is_empty() { "updated".into() } else { parts.join(", ") };
        println!("Marked \"{title}\" as {status}.");
    }
    Ok(())
}

/// Core: mark all items in a scope as read. Scope is "all", "feed", or "folder".
/// Returns the number of items newly marked read.
pub async fn mark_all_read_core(
    db: Arc<Database<'static>>,
    scope: String,
    scope_id: Option<String>,
) -> Result<usize> {
    spawn_blocking(move || -> Result<usize> {
        let rw = db.rw_transaction()?;
        let items: Vec<Item> = rw.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let folders: Vec<Folder> = rw.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();

        let target_ids: std::collections::HashSet<String> = match (scope.as_str(), &scope_id) {
            ("feed", Some(fid)) => items.iter().filter(|i| i.feed_id == *fid).map(|i| i.id.clone()).collect(),
            ("folder", Some(fid)) => {
                let desc = collect_descendant_ids(&folders, fid);
                let feeds: Vec<Feed> = rw.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
                let feed_ids: std::collections::HashSet<String> = feeds.iter()
                    .filter(|f| f.folder_id.as_ref().is_some_and(|id| desc.contains(id)))
                    .map(|f| f.id.clone()).collect();
                items.iter().filter(|i| feed_ids.contains(&i.feed_id)).map(|i| i.id.clone()).collect()
            }
            _ => items.iter().map(|i| i.id.clone()).collect(),
        };

        let mut count = 0usize;
        for item_id in &target_ids {
            let existing: Option<Mark> = rw.get().primary(item_id.clone()).ok().flatten();
            if existing.as_ref().is_some_and(|m| m.read) { continue; }
            let mut mark = Mark::from_existing(item_id.clone(), existing.as_ref());
            mark.read = true;
            mark.read_at = Some(mark.marked_at.clone());
            let _: Option<Mark> = rw.upsert(mark)?;
            count += 1;
        }
        rw.commit()?;
        Ok(count)
    }).await?
}

/// Core: toggle the read_later flag on an item. Returns (item, is_now_read_later).
pub async fn toggle_read_later_core(
    db: Arc<Database<'static>>,
    item_id: String,
) -> Result<(Item, bool)> {
    spawn_blocking(move || -> Result<(Item, bool)> {
        let rw = db.rw_transaction()?;
        let item: Item = rw.get().primary(item_id.clone())?
            .with_context(|| format!("item {item_id} not found"))?;
        let existing: Option<Mark> = rw.get().primary(item_id.clone()).ok().flatten();
        let was_later = existing.as_ref().is_some_and(|m| m.read_later);
        let mut mark = Mark::from_existing(item_id, existing.as_ref());
        mark.read_later = !was_later;
        let now_later = mark.read_later;
        let _: Option<Mark> = rw.upsert(mark)?;
        rw.commit()?;
        Ok((item, now_later))
    }).await?
}

/// Core: fetch the full article body for an item, cache it on the Item, return text.
/// If already cached, returns the cached text without refetching.
pub async fn fetch_full_article_core(
    db: Arc<Database<'static>>,
    item_id: String,
) -> Result<String> {
    let db2 = Arc::clone(&db);
    let id2 = item_id.clone();
    let (url, existing_full) = spawn_blocking(move || -> Result<(Option<String>, Option<String>)> {
        let r = db2.r_transaction()?;
        let item: Item = r.get().primary(id2)?.context("item not found")?;
        Ok((item.link.clone(), item.full_content.clone()))
    }).await??;
    if let Some(content) = existing_full { return Ok(content); }
    let url = url.context("item has no link")?;
    let text = crate::shared::feed::fetch_full_article(&url).await?;
    let text2 = text.clone();
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        let old: Item = rw.get().primary(item_id.clone())?.context("item not found")?;
        let mut updated = old.clone();
        updated.full_content = Some(text2);
        rw.update(old, updated)?;
        rw.commit()?;
        Ok(())
    }).await??;
    Ok(text)
}

/// Core: get starred items.
pub async fn starred_core(db: Arc<Database<'static>>, limit: Option<usize>) -> Result<Vec<ItemJson>> {
    spawn_blocking(move || -> Result<Vec<ItemJson>> {
        let r = db.r_transaction()?;
        let all_marks: Vec<Mark> = r.scan().primary()?.all()?.filter_map(|m| m.ok()).collect();
        let mut collected = Vec::new();
        for m in all_marks {
            if !m.starred { continue; }
            if let Some(lim) = limit { if collected.len() >= lim { break; } }
            if let Ok(Some(item)) = r.get().primary::<Item>(m.item_id.clone()) {
                let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
                collected.push(ItemJson::from_parts(&item, feed.as_ref(), Some(&m)));
            }
        }
        Ok(collected)
    }).await?
}

pub async fn starred(db: Arc<Database<'static>>, json: bool, limit: Option<usize>) -> Result<()> {
    let results = starred_core(db, limit).await?;
    if json {
        print_json(&results);
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|ij| {
            vec![
                ij.id.clone(),
                ij.source.clone().unwrap_or("-".into()),
                ij.title.clone().unwrap_or("-".into()),
                ij.note.clone().unwrap_or_default(),
                ij.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
            ]
        }).collect();
        print_table(&["ID", "SOURCE", "TITLE", "NOTE", "PUBLISHED"], &rows);
    }
    Ok(())
}

/// Core: export item data.
pub async fn export_core(db: Arc<Database<'static>>, id: String) -> Result<ItemJson> {
    spawn_blocking(move || -> Result<ItemJson> {
        let r = db.r_transaction()?;
        let item: Item = r.get().primary(id.clone())?
            .with_context(|| format!("item {id} not found"))?;
        let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
        let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
        Ok(ItemJson::from_parts(&item, feed.as_ref(), mark.as_ref()))
    }).await?
}

pub async fn export(db: Arc<Database<'static>>, _json: bool, id: String, format: String) -> Result<()> {
    let item_json = export_core(db, id).await?;
    print!("{}", crate::shared::output::format_export(&item_json, &format)?);
    Ok(())
}
