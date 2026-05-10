use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};
use chrono::Utc;
use native_db::Database;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

use crate::shared::config::RetentionConfig;
use crate::shared::db::*;
use crate::shared::feed;
use crate::shared::output::*;

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

/// Callback type for sync progress: (completed, total, &result, total_new_so_far).
pub type SyncProgressCb = Box<dyn Fn(usize, usize, &SyncFeedResult, usize) + Send + Sync>;

#[derive(Serialize, Deserialize, Clone)]
pub struct SyncFeedResult {
    pub feed: String,
    pub title: String,
    pub new_items: usize,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SyncResult {
    pub feeds_synced: usize,
    pub new_items: usize,
    pub results: Vec<SyncFeedResult>,
}

/// Core: sync feeds, returns structured result.
/// Optional `on_progress` callback is called after each feed with (completed, total, &SyncFeedResult, total_new_so_far).
pub async fn sync_core(
    db: Arc<Database<'static>>,
    feed_id: Option<String>,
    on_progress: Option<SyncProgressCb>,
) -> Result<SyncResult> {
    let db2 = Arc::clone(&db);
    let feeds: Vec<Feed> = spawn_blocking(move || {
        let r = db2.r_transaction()?;
        Ok::<_, anyhow::Error>(r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect())
    }).await??;

    let feeds: Vec<Feed> = if let Some(ref id) = feed_id {
        let matched: Vec<Feed> = feeds.into_iter().filter(|f| f.id == *id).collect();
        if matched.is_empty() { bail!("Feed not found: {id}"); }
        matched
    } else { feeds };

    let mut total_new = 0;
    let mut sync_results = Vec::new();

    for (i, f) in feeds.iter().enumerate() {
        let title = f.title.clone().unwrap_or_else(|| f.url.clone());
        let result = match feed::fetch_feed_conditional(&f.url, f.etag.as_deref(), f.last_modified.as_deref()).await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("{e:#}");
                record_feed_error(Arc::clone(&db), f.id.clone(), err_msg.clone()).await.ok();
                let r = SyncFeedResult {
                    feed: f.id.clone(), title, new_items: 0, status: "error".into(), error: Some(err_msg),
                };
                if let Some(cb) = &on_progress { cb(i + 1, feeds.len(), &r, total_new); }
                sync_results.push(r);
                continue;
            }
        };

        let fetch = match result {
            Some(r) => r,
            None => {
                // 304 still counts as a successful sync — clear error state.
                record_feed_success(Arc::clone(&db), f.id.clone()).await.ok();
                let r = SyncFeedResult {
                    feed: f.id.clone(), title, new_items: 0, status: "up_to_date".into(), error: None,
                };
                if let Some(cb) = &on_progress { cb(i + 1, feeds.len(), &r, total_new); }
                sync_results.push(r);
                continue;
            }
        };

        let now = Utc::now().to_rfc3339();
        let items = feed::entries_to_items(&fetch.feed.entries, &f.id, &now);
        let new_etag = fetch.etag.clone();
        let new_lm = fetch.last_modified.clone();
        let feed_id_clone = f.id.clone();
        let now2 = now.clone();
        let db3 = Arc::clone(&db);

        let new_count = spawn_blocking(move || {
            let rw = db3.rw_transaction()?;
            let mut new = 0usize;
            for item in items {
                let existing: Option<Item> = rw.get().secondary(ItemKey::guid, item.guid.clone()).ok().flatten();
                if existing.is_none() {
                    rw.insert(item)?;
                    new += 1;
                }
            }
            if let Some(updated) = rw.get().primary::<Feed>(feed_id_clone)? {
                let old = updated.clone();
                let mut next = updated;
                next.last_synced = Some(now2.clone());
                next.last_success_at = Some(now2);
                next.last_error = None;
                next.error_count = 0;
                next.etag = new_etag;
                next.last_modified = new_lm;
                rw.update(old, next)?;
            }
            rw.commit()?;
            Ok::<_, anyhow::Error>(new)
        }).await??;

        total_new += new_count;
        let result = SyncFeedResult {
            feed: f.id.clone(), title, new_items: new_count, status: "synced".into(), error: None,
        };
        if let Some(cb) = &on_progress { cb(i + 1, feeds.len(), &result, total_new); }
        sync_results.push(result);
    }

    Ok(SyncResult { feeds_synced: feeds.len(), new_items: total_new, results: sync_results })
}

/// Record a sync error on a feed: increment `error_count`, set `last_error`.
async fn record_feed_error(db: Arc<Database<'static>>, feed_id: String, err: String) -> Result<()> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        if let Some(updated) = rw.get().primary::<Feed>(feed_id)? {
            let old = updated.clone();
            let mut next = updated;
            next.last_error = Some(err);
            next.error_count = next.error_count.saturating_add(1);
            rw.update(old, next)?;
        }
        rw.commit()?;
        Ok::<_, anyhow::Error>(())
    }).await?
}

/// Record a successful 304-Not-Modified sync: refresh timestamps, clear error state.
async fn record_feed_success(db: Arc<Database<'static>>, feed_id: String) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        if let Some(updated) = rw.get().primary::<Feed>(feed_id)? {
            let old = updated.clone();
            let mut next = updated;
            next.last_synced = Some(now.clone());
            next.last_success_at = Some(now);
            next.last_error = None;
            next.error_count = 0;
            rw.update(old, next)?;
        }
        rw.commit()?;
        Ok::<_, anyhow::Error>(())
    }).await?
}

pub async fn sync(db: Arc<Database<'static>>, json: bool, feed_id: Option<String>) -> Result<()> {
    if !json { println!("Syncing..."); }
    let result = sync_core(Arc::clone(&db), feed_id, None).await?;

    // Auto-cleanup
    let config = crate::shared::config::Config::load().unwrap_or_default();
    let cleanup = if config.retention.auto_mark_read_after.is_some() || config.retention.auto_delete_after.is_some() {
        run_cleanup(db, &config.retention).await.ok()
    } else {
        None
    };

    if json {
        print_json(&result);
    } else {
        for r in &result.results {
            match r.status.as_str() {
                "up_to_date" => println!("  {} up to date", r.title),
                "error" => eprintln!("  {}: error: {}", r.title, r.error.as_deref().unwrap_or("?")),
                _ if r.new_items == 0 => println!("  {} up to date", r.title),
                _ => println!("  {}  {} new items", r.title, r.new_items),
            }
        }
        println!("Done. {} new items.", result.new_items);
        if let Some(ref c) = cleanup {
            if c.marked_read > 0 || c.deleted > 0 {
                println!("Auto-cleanup: marked {} old items as read, deleted {} expired items.", c.marked_read, c.deleted);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Auto-cleanup
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct CleanupResult {
    pub marked_read: u32,
    pub deleted: u32,
}

pub async fn run_cleanup(db: Arc<Database<'static>>, retention: &RetentionConfig) -> Result<CleanupResult> {
    let mark_after = retention.auto_mark_read_after.as_ref().map(|s| parse_duration(s)).transpose()?;
    let delete_after = retention.auto_delete_after.as_ref().map(|s| parse_duration(s)).transpose()?;

    spawn_blocking(move || {
        let now = chrono::Utc::now().naive_utc();
        let rw = db.rw_transaction()?;
        let all_items: Vec<Item> = rw.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut marked_read = 0u32;
        let mut deleted = 0u32;

        for item in &all_items {
            let Some(pub_dt) = item.published_at.as_ref().and_then(|p| parse_datetime(p).ok()) else { continue };
            let age = now - pub_dt;

            // Decide delete first: if we're going to delete the item, skip the
            // auto-mark-read upsert entirely so we don't leave an orphaned Mark.
            let mark: Option<Mark> = rw.get().primary(item.id.clone()).ok().flatten();
            let will_delete = delete_after.as_ref()
                .is_some_and(|dur| age > *dur && !mark.as_ref().is_some_and(|m| m.starred));

            if !will_delete {
                if let Some(ref dur) = mark_after {
                    if age > *dur && !mark.as_ref().is_some_and(|m| m.read) {
                        let mut new_mark = Mark::from_existing(item.id.clone(), mark.as_ref());
                        new_mark.read = true;
                        new_mark.read_at = Some(new_mark.marked_at.clone());
                        let _: Option<Mark> = rw.upsert(new_mark)?;
                        marked_read += 1;
                    }
                }
            }

            if will_delete {
                rw.remove(item.clone())?;
                if let Some(m) = mark { rw.remove(m)?; }
                deleted += 1;
            }
        }

        rw.commit()?;
        Ok(CleanupResult { marked_read, deleted })
    }).await?
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StatusResult {
    pub feeds: usize,
    pub items: usize,
    pub unread: usize,
    pub starred: usize,
    pub last_sync: Option<String>,
    pub feed_stats: Vec<FeedStat>,
    pub folder_stats: Vec<FolderStat>,
}

#[derive(Serialize)]
pub struct FeedStat {
    pub title: String,
    pub last_synced: String,
    pub items: usize,
    pub unread: usize,
    pub starred: usize,
}

#[derive(Serialize)]
pub struct FolderStat {
    pub name: String,
    pub feeds: usize,
    pub unread: usize,
}

/// Core: compute status, returns structured result.
pub async fn status_core(db: Arc<Database<'static>>) -> Result<StatusResult> {
    spawn_blocking(move || {
        let r = db.r_transaction()?;
        let all_feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let all_marks: Vec<Mark> = r.scan().primary()?.all()?.filter_map(|m| m.ok()).collect();
        let all_folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|f| f.ok()).collect();

        let stats_map = compute_feed_stats(&all_items, &all_marks);
        let mut feed_stats = Vec::new();
        let mut feed_unread: HashMap<String, usize> = HashMap::new();
        let mut total_items = 0;
        let mut total_unread = 0;
        let mut total_starred = 0;

        for feed in &all_feeds {
            let s = stats_map.get(&feed.id).cloned().unwrap_or_default();
            feed_unread.insert(feed.id.clone(), s.unread);
            total_items += s.items;
            total_unread += s.unread;
            total_starred += s.starred;
            feed_stats.push(FeedStat {
                title: feed.title.clone().unwrap_or_else(|| feed.url.clone()),
                last_synced: feed.last_synced.clone().unwrap_or_default(),
                items: s.items, unread: s.unread, starred: s.starred,
            });
        }

        let folder_stats = build_folder_stats(&all_folders, &all_feeds, &feed_unread);
        let last_sync = all_feeds.iter().filter_map(|f| f.last_synced.as_deref()).max().map(String::from);

        Ok(StatusResult {
            feeds: all_feeds.len(), items: total_items, unread: total_unread,
            starred: total_starred, last_sync, feed_stats, folder_stats,
        })
    }).await?
}

pub async fn status(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let s = status_core(db).await?;
    if json {
        print_json(&s);
    } else {
        println!("Feeds: {}", s.feeds);
        println!("Items: {} total, {} unread, {} starred", s.items, s.unread, s.starred);
        println!("Last sync: {}", s.last_sync.as_deref().map(time_ago).unwrap_or_else(|| "never".into()));
        println!();
        let rows: Vec<Vec<String>> = s.feed_stats.iter().map(|fs| {
            vec![
                fs.title.clone(),
                if fs.last_synced.is_empty() { "never".into() } else { time_ago(&fs.last_synced) },
                fs.items.to_string(), fs.unread.to_string(),
            ]
        }).collect();
        print_table(&["FEED", "LAST SYNCED", "ITEMS", "UNREAD"], &rows);
        if !s.folder_stats.is_empty() {
            println!("\nPer-folder:");
            let frows: Vec<Vec<String>> = s.folder_stats.iter().map(|fs| {
                vec![fs.name.clone(), fs.feeds.to_string(), fs.unread.to_string()]
            }).collect();
            print_table(&["FOLDER", "FEEDS", "UNREAD"], &frows);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Folder stats helper
// ---------------------------------------------------------------------------

fn roll_up(
    folder: &Folder,
    children: &HashMap<Option<String>, Vec<&Folder>>,
    feeds_by_folder: &HashMap<String, Vec<&Feed>>,
    feed_unread: &HashMap<String, usize>,
    rows: &mut Vec<FolderStat>,
) -> (usize, usize) {
    let direct = feeds_by_folder.get(&folder.id).cloned().unwrap_or_default();
    let mut fc = direct.len();
    let mut ur: usize = direct.iter().map(|f| feed_unread.get(&f.id).copied().unwrap_or(0)).sum();
    if let Some(kids) = children.get(&Some(folder.id.clone())) {
        for kid in kids {
            let (cf, cu) = roll_up(kid, children, feeds_by_folder, feed_unread, rows);
            fc += cf; ur += cu;
        }
    }
    rows.push(FolderStat { name: folder.name.clone(), feeds: fc, unread: ur });
    (fc, ur)
}

fn build_folder_stats(folders: &[Folder], feeds: &[Feed], feed_unread: &HashMap<String, usize>) -> Vec<FolderStat> {
    let mut children_by_parent: HashMap<Option<String>, Vec<&Folder>> = HashMap::new();
    for folder in folders { children_by_parent.entry(folder.parent_id.clone()).or_default().push(folder); }
    let mut feeds_by_folder: HashMap<String, Vec<&Feed>> = HashMap::new();
    for feed in feeds {
        if let Some(ref fid) = feed.folder_id { feeds_by_folder.entry(fid.clone()).or_default().push(feed); }
    }
    let mut rows = Vec::new();
    for folder in children_by_parent.get(&None).cloned().unwrap_or_default() {
        roll_up(folder, &children_by_parent, &feeds_by_folder, feed_unread, &mut rows);
    }
    let uncategorized_feeds: usize = feeds.iter().filter(|f| f.folder_id.is_none()).count();
    if uncategorized_feeds > 0 {
        let uncategorized_unread: usize = feeds.iter()
            .filter(|f| f.folder_id.is_none())
            .map(|f| feed_unread.get(&f.id).copied().unwrap_or(0)).sum();
        rows.push(FolderStat { name: "Uncategorized".into(), feeds: uncategorized_feeds, unread: uncategorized_unread });
    }
    rows
}

