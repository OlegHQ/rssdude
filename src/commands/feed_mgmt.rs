use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::feed;
use crate::shared::output::*;

/// Core: add feed and return (Feed, items_synced).
pub async fn add_core(
    db: Arc<Database<'static>>,
    url: String,
    tags: Vec<String>,
    folder: Option<String>,
) -> Result<(Feed, usize)> {
    let result = feed::fetch_feed(&url).await?;
    let title = result.feed.title.as_ref().map(|t| t.content.clone()).unwrap_or_else(|| url.clone());
    let description = result.feed.description.as_ref().map(|t| t.content.clone());
    let now = Utc::now().to_rfc3339();

    if let Some(ref fid) = folder {
        let db2 = Arc::clone(&db);
        let fid2 = fid.clone();
        let f = spawn_blocking(move || {
            let r = db2.r_transaction()?;
            Ok::<_, anyhow::Error>(r.get().primary::<Folder>(fid2.clone())?)
        }).await??;
        if f.is_none() {
            anyhow::bail!("Folder not found: {fid}");
        }
    }

    let new_feed = Feed {
        id: gen_id(), url: url.clone(), title: Some(title), description,
        tags: tags.join(","), added_at: now.clone(), last_synced: Some(now.clone()),
        etag: result.etag.clone(), last_modified: result.last_modified.clone(),
        folder_id: folder,
        last_error: None, error_count: 0, last_success_at: Some(now.clone()),
        custom_title: None,
    };

    let items = feed::entries_to_items(&result.feed.entries, &new_feed.id, &now);
    let item_count = items.len();
    let feed_clone = new_feed.clone();
    let db2 = Arc::clone(&db);
    spawn_blocking(move || {
        let rw = db2.rw_transaction()?;
        rw.insert(feed_clone)?;
        for item in items {
            let existing: Option<Item> = rw.get().secondary(ItemKey::guid, item.guid.clone()).ok().flatten();
            if existing.is_none() { rw.insert(item)?; }
        }
        rw.commit()?;
        Ok::<_, anyhow::Error>(())
    }).await??;

    Ok((new_feed, item_count))
}

pub async fn add(db: Arc<Database<'static>>, json: bool, url: String, tags: Vec<String>, folder: Option<String>) -> Result<()> {
    let (feed, item_count) = add_core(db, url.clone(), tags, folder).await?;
    if json {
        print_json(&FeedJson::from(&feed));
    } else {
        let title = feed.title.as_deref().unwrap_or(&url);
        println!("Added feed: {title} ({url})");
        println!("Synced {item_count} items.");
    }
    Ok(())
}

/// Core: list feeds, returns (feeds_with_counts, folders).
pub async fn list_core(db: Arc<Database<'static>>, tag: Option<String>) -> Result<Vec<FeedJson>> {
    let db2 = Arc::clone(&db);
    let feeds = spawn_blocking(move || {
        let r = db2.r_transaction()?;
        let all: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        Ok::<_, anyhow::Error>(all)
    }).await??;

    let filtered: Vec<FeedJson> = feeds.iter()
        .filter(|f| match &tag {
            Some(t) => f.tags.split(',').any(|s| s.trim() == t.as_str()),
            None => true,
        })
        .map(FeedJson::from)
        .collect();
    Ok(filtered)
}

pub async fn list(db: Arc<Database<'static>>, json: bool, tag: Option<String>) -> Result<()> {
    let db2 = Arc::clone(&db);
    let data = spawn_blocking(move || {
        let r = db2.r_transaction()?;
        let feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let folders: Vec<Folder> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut item_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for item in &items { *item_counts.entry(item.feed_id.clone()).or_default() += 1; }
        let feed_data: Vec<(Feed, usize)> = feeds.into_iter()
            .map(|f| { let c = item_counts.get(&f.id).copied().unwrap_or(0); (f, c) }).collect();
        Ok::<_, anyhow::Error>((feed_data, folders))
    }).await??;

    let (feed_data, folders) = data;
    let filtered: Vec<&(Feed, usize)> = feed_data.iter()
        .filter(|(f, _)| match &tag {
            Some(t) => f.tags.split(',').any(|s| s.trim() == t.as_str()),
            None => true,
        }).collect();

    if json {
        let json_feeds: Vec<FeedJson> = filtered.iter().map(|(f, _)| FeedJson::from(f)).collect();
        print_json(&json_feeds);
    } else {
        let rows: Vec<Vec<String>> = filtered.iter().map(|(f, count)| {
            let folder_name = f.folder_id.as_ref()
                .and_then(|fid| folders.iter().find(|fl| fl.id == *fid))
                .map(|fl| fl.name.clone()).unwrap_or_default();
            vec![
                f.id.clone(), f.display_title().to_string(), f.url.clone(),
                f.tags.clone(), folder_name, count.to_string(),
                f.last_synced.as_deref().map(time_ago).unwrap_or_else(|| "never".into()),
            ]
        }).collect();
        print_table(&["ID", "TITLE", "URL", "TAGS", "FOLDER", "ITEMS", "LAST SYNCED"], &rows);
    }
    Ok(())
}

/// Core: remove feed, returns (title, items_deleted).
pub async fn remove_core(db: Arc<Database<'static>>, id: String) -> Result<(String, usize)> {
    let db2 = Arc::clone(&db);
    spawn_blocking(move || {
        let rw = db2.rw_transaction()?;
        let feed: Feed = rw.get().primary(id.clone())?
            .ok_or_else(|| anyhow::anyhow!("Feed not found: {id}"))?;
        let title = feed.display_title().to_string();
        let items: Vec<Item> = rw.scan().secondary::<Item>(ItemKey::feed_id)?
            .all()?.filter_map(|i| i.ok()).filter(|i: &Item| i.feed_id == feed.id).collect();
        let count = items.len();
        for item in &items {
            if let Ok(Some(m)) = rw.get().primary::<Mark>(item.id.clone()) { let _ = rw.remove(m); }
            rw.remove(item.clone())?;
        }
        rw.remove(feed)?;
        rw.commit()?;
        Ok((title, count))
    }).await?
}

pub async fn remove(db: Arc<Database<'static>>, json: bool, id: String, yes: bool) -> Result<()> {
    if !yes && !json {
        let db_peek = Arc::clone(&db);
        let id_peek = id.clone();
        let (peek_title, peek_count) = spawn_blocking(move || {
            let r = db_peek.r_transaction()?;
            let feed: Feed = r.get().primary(id_peek.clone())?
                .ok_or_else(|| anyhow::anyhow!("Feed not found: {id_peek}"))?;
            let title = feed.display_title().to_string();
            let count: usize = r.scan().secondary::<Item>(ItemKey::feed_id)?
                .all()?.filter_map(|i| i.ok()).filter(|i: &Item| i.feed_id == feed.id).count();
            Ok::<_, anyhow::Error>((title, count))
        }).await??;
        eprint!("Remove feed \"{peek_title}\" and {peek_count} items? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let (title, removed_count) = remove_core(db, id).await?;
    if json {
        print_json(&serde_json::json!({"title": title, "items_deleted": removed_count}));
    } else {
        println!("Removed feed: {title} ({removed_count} items deleted)");
    }
    Ok(())
}

/// Core: move feed to folder (or root if None), returns (feed_title, folder_name).
pub async fn move_to_folder_core(db: Arc<Database<'static>>, feed_id: String, folder_id: Option<String>) -> Result<(String, String)> {
    let db2 = Arc::clone(&db);
    spawn_blocking(move || {
        let rw = db2.rw_transaction()?;
        let old_feed: Feed = rw.get().primary(feed_id.clone())?
            .ok_or_else(|| anyhow::anyhow!("Feed not found: {feed_id}"))?;
        let folder_name = if let Some(ref fid) = folder_id {
            let folder: Folder = rw.get().primary(fid.clone())?
                .ok_or_else(|| anyhow::anyhow!("Folder not found: {fid}"))?;
            folder.name.clone()
        } else {
            "Uncategorized".to_string()
        };
        let feed_title = old_feed.display_title().to_string();
        let mut new_feed = old_feed.clone();
        new_feed.folder_id = folder_id;
        rw.update(old_feed, new_feed)?;
        rw.commit()?;
        Ok((feed_title, folder_name))
    }).await?
}

pub async fn move_to_folder(db: Arc<Database<'static>>, json: bool, feed_id: String, folder_id: String) -> Result<()> {
    let (feed_title, folder_name) = move_to_folder_core(db, feed_id.clone(), Some(folder_id.clone())).await?;
    if json {
        print_json(&serde_json::json!({"feed_id": feed_id, "folder_id": folder_id, "feed_title": feed_title, "folder_name": folder_name}));
    } else {
        println!("Moved feed \"{feed_title}\" to folder \"{folder_name}\"");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Doctor (feed health)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct FeedHealth {
    pub id: String,
    pub title: String,
    pub url: String,
    pub error_count: u32,
    pub last_error: Option<String>,
    pub last_success: Option<String>,
    pub auto_disabled: bool,
}

pub async fn doctor_core(db: Arc<Database<'static>>) -> Result<Vec<FeedHealth>> {
    spawn_blocking(move || -> Result<Vec<FeedHealth>> {
        let r = db.r_transaction()?;
        let feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let unhealthy: Vec<FeedHealth> = feeds.iter()
            .filter(|f| f.error_count > 0)
            .map(|f| FeedHealth {
                id: f.id.clone(),
                title: f.display_title().to_string(),
                url: f.url.clone(),
                error_count: f.error_count,
                last_error: f.last_error.clone(),
                last_success: f.last_success_at.clone(),
                auto_disabled: f.error_count >= 10,
            })
            .collect();
        Ok(unhealthy)
    }).await?
}

pub async fn doctor(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let results = doctor_core(db).await?;
    if json { print_json(&results); }
    else if results.is_empty() {
        println!("All feeds healthy.");
    } else {
        let rows: Vec<Vec<String>> = results.iter().map(|f| vec![
            f.id.clone(),
            f.title.clone(),
            f.error_count.to_string(),
            f.last_error.clone().unwrap_or("-".into()),
            if f.auto_disabled { "DISABLED".into() } else { "active".into() },
        ]).collect();
        print_table(&["ID", "FEED", "ERRORS", "LAST ERROR", "STATUS"], &rows);
    }
    Ok(())
}

pub async fn doctor_reset_core(db: Arc<Database<'static>>, feed_id: String) -> Result<String> {
    spawn_blocking(move || -> Result<String> {
        let rw = db.rw_transaction()?;
        let old: Feed = rw.get().primary(feed_id.clone())?
            .with_context(|| format!("feed {feed_id} not found"))?;
        let title = old.display_title().to_string();
        let mut updated = old.clone();
        updated.error_count = 0;
        updated.last_error = None;
        rw.update(old, updated)?;
        rw.commit()?;
        Ok(title)
    }).await?
}

pub async fn doctor_reset(db: Arc<Database<'static>>, json: bool, feed_id: String) -> Result<()> {
    let title = doctor_reset_core(db, feed_id).await?;
    if json { print_json(&serde_json::json!({"reset": title})); }
    else { println!("Reset error count for: {title}"); }
    Ok(())
}

// ---------------------------------------------------------------------------
// Rename Feed
// ---------------------------------------------------------------------------

pub async fn rename_feed_core(db: Arc<Database<'static>>, id: String, title: String) -> Result<Feed> {
    spawn_blocking(move || -> Result<Feed> {
        let rw = db.rw_transaction()?;
        let old: Feed = rw.get().primary(id.clone())?
            .with_context(|| format!("feed {id} not found"))?;
        let mut updated = old.clone();
        updated.custom_title = Some(title);
        rw.update(old, updated.clone())?;
        rw.commit()?;
        Ok(updated)
    }).await?
}

pub async fn rename_feed(db: Arc<Database<'static>>, json: bool, id: String, title: String) -> Result<()> {
    let feed = rename_feed_core(db, id, title).await?;
    if json { print_json(&FeedJson::from(&feed)); }
    else { println!("Renamed feed to: {}", feed.custom_title.as_deref().unwrap_or("?")); }
    Ok(())
}
