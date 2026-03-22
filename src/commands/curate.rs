use std::sync::Arc;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

/// Core: apply mark flags to an item, returns (Item, Mark).
pub async fn mark_core(
    db: Arc<Database<'static>>,
    id: String,
    read: bool,
    star: bool,
    note: Option<String>,
) -> Result<(Item, Mark)> {
    spawn_blocking(move || -> Result<(Item, Mark)> {
        let rw = db.rw_transaction()?;
        let item: Item = rw.get().primary(id.clone())?
            .with_context(|| format!("item {id} not found"))?;
        let existing: Option<Mark> = rw.get().primary(id.clone()).ok().flatten();

        let now = Utc::now().to_rfc3339();
        let was_read = existing.as_ref().is_some_and(|m| m.read);
        let new_read = if read { true } else { was_read };
        let new_mark = Mark {
            item_id: id,
            read: new_read,
            starred: if star { !existing.as_ref().is_some_and(|m| m.starred) } else { existing.as_ref().is_some_and(|m| m.starred) },
            note: if note.is_some() { note } else { existing.as_ref().and_then(|m| m.note.clone()) },
            read_at: if new_read && !was_read { Some(now.clone()) } else { existing.as_ref().and_then(|m| m.read_at.clone()) },
            opened_at: existing.as_ref().and_then(|m| m.opened_at.clone()),
            read_later: existing.as_ref().is_some_and(|m| m.read_later),
            marked_at: now,
        };
        let _: Option<Mark> = rw.upsert(new_mark.clone())?;
        rw.commit()?;
        Ok((item, new_mark))
    }).await?
}

pub async fn mark(db: Arc<Database<'static>>, json: bool, id: String, read: bool, star: bool, note: Option<String>) -> Result<()> {
    let (item, new_mark) = mark_core(db, id, read, star, note).await?;
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

/// Core: get starred items.
pub async fn starred_core(db: Arc<Database<'static>>, limit: Option<usize>) -> Result<Vec<(Item, Option<Feed>, Mark)>> {
    spawn_blocking(move || -> Result<Vec<(Item, Option<Feed>, Mark)>> {
        let r = db.r_transaction()?;
        let all_marks: Vec<Mark> = r.scan().primary()?.all()?.filter_map(|m| m.ok()).collect();
        let mut collected = Vec::new();
        for m in all_marks {
            if !m.starred { continue; }
            if let Some(lim) = limit { if collected.len() >= lim { break; } }
            if let Ok(Some(item)) = r.get().primary::<Item>(m.item_id.clone()) {
                let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
                collected.push((item, feed, m));
            }
        }
        Ok(collected)
    }).await?
}

pub async fn starred(db: Arc<Database<'static>>, json: bool, limit: Option<usize>) -> Result<()> {
    let results = starred_core(db, limit).await?;
    if json {
        let json_items: Vec<ItemJson> = results.iter()
            .map(|(item, feed, mark)| ItemJson::from_parts(item, feed.as_ref(), Some(mark))).collect();
        print_json(&json_items);
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|(item, feed, mark)| {
            vec![
                item.id.clone(),
                feed.as_ref().and_then(|f| f.title.clone()).unwrap_or("-".into()),
                item.title.clone().unwrap_or("-".into()),
                mark.note.clone().unwrap_or_default(),
                item.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
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
    let title = item_json.title.as_deref().unwrap_or("(untitled)");
    let source = item_json.source.as_deref().unwrap_or("-");
    let published = item_json.published_at.as_deref().unwrap_or("-");
    let url = item_json.link.as_deref().unwrap_or("-");
    let note = item_json.note.as_deref().unwrap_or("");
    let content = item_json.content.as_deref().or(item_json.summary.as_deref()).unwrap_or("");

    match format.as_str() {
        "md" | "markdown" => {
            println!("# {title}\n");
            println!("**Source:** {source}");
            println!("**Published:** {published}");
            println!("**URL:** {url}");
            if !note.is_empty() { println!("**Note:** {note}"); }
            println!("\n---\n");
            println!("{content}");
        }
        "json" => print_json(&item_json),
        "text" | "txt" => {
            println!("{title}");
            println!("Source: {source}");
            println!("Published: {published}");
            println!("URL: {url}");
            if !note.is_empty() { println!("Note: {note}"); }
            println!();
            println!("{content}");
        }
        _ => bail!("unsupported format: {format} (expected md, json, or text)"),
    }
    Ok(())
}
