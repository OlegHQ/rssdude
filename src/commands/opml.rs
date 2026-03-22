use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::feed;
use crate::shared::output::*;

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
                let now = Utc::now().to_rfc3339();
                let (new_feed, items) = match feed::fetch_feed(&url).await {
                    Ok(result) => {
                        let f = Feed {
                            id: gen_id(),
                            url: url.clone(),
                            title: result.feed.title.map(|t| t.content),
                            description: result.feed.description.map(|d| d.content),
                            tags: String::new(),
                            added_at: now.clone(),
                            last_synced: Some(now.clone()),
                            etag: result.etag,
                            last_modified: result.last_modified,
                            folder_id: parent_folder_id.clone(),
                            last_error: None,
                            error_count: 0,
                            last_success_at: Some(now.clone()),
                            custom_title: None,
                        };
                        let items = feed::entries_to_items(&result.feed.entries, &f.id, &now);
                        (f, items)
                    }
                    Err(_) => {
                        let f = Feed {
                            id: gen_id(),
                            url: url.clone(),
                            title: if outline.text.is_empty() { None } else { Some(outline.text.clone()) },
                            description: None,
                            tags: String::new(),
                            added_at: now.clone(),
                            last_synced: None,
                            etag: None,
                            last_modified: None,
                            folder_id: parent_folder_id.clone(),
                            last_error: Some("initial fetch failed".into()),
                            error_count: 1,
                            last_success_at: None,
                            custom_title: None,
                        };
                        (f, vec![])
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

/// Export all folders and feeds as OPML XML string.
pub async fn export_core(db: Arc<Database<'static>>) -> Result<String> {
    spawn_blocking(move || -> Result<String> {
        let r = db.r_transaction()?;
        let folders: Vec<Folder> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();

        let doc_head = opml::Head {
            title: Some("rssdude feeds".into()),
            ..Default::default()
        };
        let mut doc = opml::OPML { head: Some(doc_head), ..Default::default() };

        // Group feeds by folder_id
        let mut feeds_by_folder: std::collections::HashMap<Option<String>, Vec<&Feed>> = std::collections::HashMap::new();
        for feed in &feeds {
            feeds_by_folder.entry(feed.folder_id.clone()).or_default().push(feed);
        }

        // Group folders by parent_id
        let mut folders_by_parent: std::collections::HashMap<Option<String>, Vec<&Folder>> = std::collections::HashMap::new();
        for folder in &folders {
            folders_by_parent.entry(folder.parent_id.clone()).or_default().push(folder);
        }

        fn build_outline(
            folder: &Folder,
            folders_by_parent: &std::collections::HashMap<Option<String>, Vec<&Folder>>,
            feeds_by_folder: &std::collections::HashMap<Option<String>, Vec<&Feed>>,
        ) -> opml::Outline {
            let mut outline = opml::Outline {
                text: folder.name.clone(),
                ..Default::default()
            };
            // Add feeds in this folder
            if let Some(folder_feeds) = feeds_by_folder.get(&Some(folder.id.clone())) {
                for feed in folder_feeds {
                    let display_title = feed.display_title().to_string();
                    outline.outlines.push(opml::Outline {
                        text: display_title,
                        xml_url: Some(feed.url.clone()),
                        html_url: None,
                        r#type: Some("rss".into()),
                        ..Default::default()
                    });
                }
            }
            // Add child folders
            if let Some(children) = folders_by_parent.get(&Some(folder.id.clone())) {
                for child in children {
                    outline.outlines.push(build_outline(child, folders_by_parent, feeds_by_folder));
                }
            }
            outline
        }

        // Add root folders
        if let Some(root_folders) = folders_by_parent.get(&None) {
            for folder in root_folders {
                doc.body.outlines.push(build_outline(folder, &folders_by_parent, &feeds_by_folder));
            }
        }
        // Add uncategorized feeds (no folder)
        if let Some(root_feeds) = feeds_by_folder.get(&None) {
            for feed in root_feeds {
                let display_title = feed.display_title().to_string();
                doc.body.outlines.push(opml::Outline {
                    text: display_title,
                    xml_url: Some(feed.url.clone()),
                    r#type: Some("rss".into()),
                    ..Default::default()
                });
            }
        }

        doc.to_string().context("failed to serialize OPML")
    }).await?
}

pub async fn import(db: Arc<Database<'static>>, json: bool, path: String) -> Result<()> {
    let content = tokio::fs::read_to_string(&path).await
        .with_context(|| format!("failed to read {path}"))?;
    let result = import_core(db, content).await?;
    if json {
        print_json(&result);
    } else {
        println!("Imported {} feeds, created {} folders ({} skipped).",
            result.feeds_added, result.folders_created, result.feeds_skipped);
    }
    Ok(())
}

pub async fn export(db: Arc<Database<'static>>, _json: bool) -> Result<()> {
    let xml = export_core(db).await?;
    println!("{xml}");
    Ok(())
}
