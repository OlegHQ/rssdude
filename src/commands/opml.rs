use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::feed;

#[derive(Serialize)]
pub struct ImportResult {
    pub folders_created: usize,
    pub feeds_added: usize,
    pub feeds_skipped: usize,
}

async fn feed_url_exists(db: &Arc<Database<'static>>, url: &str) -> Result<bool> {
    let db = Arc::clone(db);
    let url = url.to_string();
    spawn_blocking(move || -> Result<bool> {
        let r = db.r_transaction()?;
        Ok(r.get().secondary::<Feed>(FeedKey::url, url)?.is_some())
    }).await?
}

async fn insert_feed(db: &Arc<Database<'static>>, feed: Feed, items: Vec<Item>) -> Result<()> {
    let db = Arc::clone(db);
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        rw.insert(feed)?;
        for item in items {
            let _: Option<Item> = rw.upsert(item)?;
        }
        rw.commit()?;
        Ok(())
    }).await?
}

/// Import OPML: create folder hierarchy, add feeds, skip duplicates by URL.
pub async fn import_core(db: Arc<Database<'static>>, content: String) -> Result<ImportResult> {
    let doc = opml::OPML::from_str(&content).context("failed to parse OPML")?;
    let mut folders_created = 0usize;
    let mut feeds_added = 0usize;
    let mut feeds_skipped = 0usize;

    async fn process_outlines(
        db: &Arc<Database<'static>>,
        outlines: &[opml::Outline],
        parent_folder_id: Option<String>,
        folders_created: &mut usize,
        feeds_added: &mut usize,
        feeds_skipped: &mut usize,
    ) -> Result<()> {
        for outline in outlines {
            if let Some(ref xml_url) = outline.xml_url {
                let url = xml_url.clone();
                if feed_url_exists(db, &url).await? {
                    *feeds_skipped += 1;
                    continue;
                }
                let (new_feed, items) = match feed::fetch_feed(&url).await {
                    Ok(result) => feed::feed_from_fetch_result(url.clone(), &result, Vec::new(), parent_folder_id.clone()),
                    Err(_) => {
                        let title = if outline.text.is_empty() { None } else { Some(outline.text.clone()) };
                        (feed::feed_from_failed_fetch(url.clone(), title, parent_folder_id.clone()), vec![])
                    }
                };
                insert_feed(db, new_feed, items).await?;
                *feeds_added += 1;
            } else if !outline.outlines.is_empty() {
                let folder_name = if outline.text.is_empty() {
                    outline.title.clone().unwrap_or_else(|| "Unnamed".to_string())
                } else {
                    outline.text.clone()
                };
                let folder = Folder {
                    id: gen_id(),
                    name: folder_name,
                    parent_id: parent_folder_id.clone(),
                    created_at: Utc::now().to_rfc3339(),
                };
                let folder_id = folder.id.clone();
                let db2 = Arc::clone(db);
                spawn_blocking(move || -> Result<()> {
                    let rw = db2.rw_transaction()?;
                    rw.insert(folder)?;
                    rw.commit()?;
                    Ok(())
                }).await??;
                *folders_created += 1;
                Box::pin(process_outlines(db, &outline.outlines, Some(folder_id), folders_created, feeds_added, feeds_skipped)).await?;
            }
        }
        Ok(())
    }

    process_outlines(&db, &doc.body.outlines, None, &mut folders_created, &mut feeds_added, &mut feeds_skipped).await?;

    Ok(ImportResult { folders_created, feeds_added, feeds_skipped })
}
