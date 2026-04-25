use std::sync::Arc;

use anyhow::Result;
use native_db::Database;
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

    let result = feed::fetch_feed(&url).await?;
    let (new_feed, items) = feed::feed_from_fetch_result(url, &result, tags, folder);
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

/// Core: list feeds with item counts and folder names.
pub async fn list_core(db: Arc<Database<'static>>, tag: Option<String>) -> Result<Vec<FeedJson>> {
    spawn_blocking(move || {
        let r = db.r_transaction()?;
        let feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let folders: Vec<Folder> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut item_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for item in &items { *item_counts.entry(item.feed_id.clone()).or_default() += 1; }
        let folder_map: std::collections::HashMap<String, &Folder> = folders.iter().map(|f| (f.id.clone(), f)).collect();
        let result: Vec<FeedJson> = feeds.iter()
            .filter(|f| tag.as_ref().is_none_or(|t| f.has_tag(t)))
            .map(|f| {
                let mut j = FeedJson::from(f);
                j.item_count = item_counts.get(&f.id).copied().unwrap_or(0);
                j.folder_name = f.folder_id.as_ref().and_then(|fid| folder_map.get(fid)).map(|fl| fl.name.clone());
                j
            })
            .collect();
        Ok::<_, anyhow::Error>(result)
    }).await?
}

pub async fn list(db: Arc<Database<'static>>, json: bool, tag: Option<String>) -> Result<()> {
    let feeds = list_core(db, tag).await?;
    if json {
        print_json(&feeds);
    } else {
        let rows: Vec<Vec<String>> = feeds.iter().map(|f| {
            vec![
                f.id.clone(), f.title.as_deref().unwrap_or(&f.url).to_string(), f.url.clone(),
                f.tags.join(","), f.folder_name.clone().unwrap_or_default(), f.item_count.to_string(),
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

