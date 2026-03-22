use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

pub struct ItemsQuery {
    pub limit: usize,
    pub since: Option<String>,
    pub tag: Option<String>,
    pub unread: bool,
    pub feed_id: Option<String>,
    pub folder_id: Option<String>,
}

/// Core: query items with filters, returns ItemJson list.
pub async fn items_core(db: Arc<Database<'static>>, query: ItemsQuery) -> Result<Vec<ItemJson>> {
    let results = spawn_blocking(move || -> Result<Vec<ItemJson>> {
        let r = db.r_transaction().context("read transaction")?;
        let cutoff = if let Some(ref s) = query.since {
            Some(Utc::now().naive_utc() - parse_duration(s)?)
        } else { None };

        let valid_feed_ids: Option<HashSet<String>> = if let Some(ref folder_id) = query.folder_id {
            let all_folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|f| f.ok()).collect();
            let descendant_ids = collect_descendant_ids(&all_folders, folder_id);
            let all_feeds: Vec<Feed> = r.scan().primary::<Feed>()?.all()?.filter_map(|f| f.ok()).collect();
            Some(all_feeds.iter()
                .filter(|f| f.folder_id.as_ref().is_some_and(|fid| descendant_ids.contains(fid)))
                .map(|f| f.id.clone()).collect())
        } else { None };

        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut collected = Vec::new();

        for item in all_items.into_iter().rev() {
            if let Some(ref fid) = query.feed_id { if item.feed_id != *fid { continue; } }
            if let Some(ref valid) = valid_feed_ids { if !valid.contains(&item.feed_id) { continue; } }
            if let Some(ref cutoff) = cutoff {
                match item.published_at.as_ref().and_then(|p| parse_datetime(p).ok()) {
                    Some(dt) if dt >= *cutoff => {}
                    _ => continue,
                }
            }
            let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
            if let Some(ref t) = query.tag {
                match &feed {
                    Some(f) if f.tags.split(',').any(|ft| ft.trim().eq_ignore_ascii_case(t)) => {}
                    _ => continue,
                }
            }
            let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
            if query.unread { if let Some(ref m) = mark { if m.read { continue; } } }
            collected.push(ItemJson::from_parts(&item, feed.as_ref(), mark.as_ref()));
            if collected.len() >= query.limit { break; }
        }
        Ok(collected)
    }).await??;
    Ok(results)
}

pub async fn items(db: Arc<Database<'static>>, json: bool, query: ItemsQuery) -> Result<()> {
    let results = items_core(db, query).await?;
    if json {
        print_json(&results);
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|ij| {
            vec![
                ij.id.clone(),
                ij.source.clone().unwrap_or("-".into()),
                ij.title.clone().unwrap_or("-".into()),
                ij.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
            ]
        }).collect();
        print_table(&["ID", "SOURCE", "TITLE", "PUBLISHED"], &rows);
    }
    Ok(())
}

/// Core: read a single item, auto-marks as read, returns ItemJson.
pub async fn read_item_core(db: Arc<Database<'static>>, id: String) -> Result<ItemJson> {
    spawn_blocking(move || -> Result<ItemJson> {
        let r = db.r_transaction()?;
        let item: Item = r.get().primary(id.clone())?.with_context(|| format!("item {id} not found"))?;
        let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
        let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
        drop(r);

        let rw = db.rw_transaction()?;
        let now = Utc::now().to_rfc3339();
        let was_read = mark.as_ref().is_some_and(|m| m.read);
        let new_mark = Mark {
            item_id: item.id.clone(), read: true,
            starred: mark.as_ref().is_some_and(|m| m.starred),
            note: mark.as_ref().and_then(|m| m.note.clone()),
            read_at: if !was_read { Some(now.clone()) } else { mark.as_ref().and_then(|m| m.read_at.clone()) },
            opened_at: mark.as_ref().and_then(|m| m.opened_at.clone()),
            read_later: mark.as_ref().is_some_and(|m| m.read_later),
            marked_at: now,
        };
        let _: Option<Mark> = rw.upsert(new_mark.clone())?;
        rw.commit()?;
        Ok(ItemJson::from_parts(&item, feed.as_ref(), Some(&new_mark)))
    }).await?
}

pub async fn read_item(db: Arc<Database<'static>>, json: bool, id: String, open: bool, raw: bool) -> Result<()> {
    let ij = read_item_core(db, id).await?;
    if json {
        print_json(&ij);
    } else {
        println!("Source:    {}", ij.source.as_deref().unwrap_or("-"));
        println!("Title:     {}", ij.title.as_deref().unwrap_or("(untitled)"));
        println!("Published: {}", ij.published_at.as_deref().unwrap_or("-"));
        println!("URL:       {}", ij.link.as_deref().unwrap_or("-"));
        println!("---");
        let body = ij.content.as_deref().or(ij.summary.as_deref()).unwrap_or("(no content)");
        if raw {
            println!("{body}");
        } else {
            println!("{}", html2text::from_read(body.as_bytes(), 80).trim());
        }
    }
    if open {
        if let Some(ref url) = ij.link { let _ = std::process::Command::new("open").arg(url).spawn(); }
    }
    Ok(())
}

/// Core: search items, returns ItemJson list.
pub async fn search_core(db: Arc<Database<'static>>, query: String, limit: usize) -> Result<Vec<ItemJson>> {
    spawn_blocking(move || -> Result<Vec<ItemJson>> {
        let r = db.r_transaction()?;
        let query_lower = query.to_lowercase();
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut hits: Vec<Item> = all_items.into_iter().filter(|item| {
            item.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&query_lower))
            || item.content.as_ref().is_some_and(|c| c.to_lowercase().contains(&query_lower))
            || item.summary.as_ref().is_some_and(|s| s.to_lowercase().contains(&query_lower))
        }).collect();
        hits.sort_by(|a, b| b.published_at.as_deref().unwrap_or("").cmp(a.published_at.as_deref().unwrap_or("")));
        let mut matched = Vec::new();
        for item in hits.into_iter().take(limit) {
            let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
            let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
            matched.push(ItemJson::from_parts(&item, feed.as_ref(), mark.as_ref()));
        }
        Ok(matched)
    }).await?
}

pub async fn search(db: Arc<Database<'static>>, json: bool, query: String, limit: usize) -> Result<()> {
    let results = search_core(db, query, limit).await?;
    if json {
        print_json(&results);
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|ij| {
            vec![
                ij.id.clone(),
                ij.source.clone().unwrap_or("-".into()),
                ij.title.clone().unwrap_or("-".into()),
                ij.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
            ]
        }).collect();
        print_table(&["ID", "SOURCE", "TITLE", "PUBLISHED"], &rows);
    }
    Ok(())
}

