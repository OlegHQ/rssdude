use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use serde::Serialize;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::feed;
use crate::shared::output::print_json;

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

/// Look up an existing folder by `(name, parent_id)`. Imports reuse the match
/// so re-running on the same OPML doesn't multiply folders.
async fn find_folder_by_name(db: &Arc<Database<'static>>, name: String, parent_id: Option<String>) -> Result<Option<Folder>> {
    let db = Arc::clone(db);
    spawn_blocking(move || -> Result<Option<Folder>> {
        let r = db.r_transaction()?;
        let folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|f| f.ok()).collect();
        Ok(folders.into_iter().find(|f| f.name == name && f.parent_id == parent_id))
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
            } else {
                // Folder outline (no xml_url). Always materialize, even when empty,
                // so round-tripping `opml export | opml import` preserves structure.
                let folder_name = match (outline.text.as_str(), outline.title.as_deref()) {
                    ("", Some(t)) => t.to_string(),
                    ("", None) => continue,
                    (text, _) => text.to_string(),
                };
                let folder_id = match find_folder_by_name(db, folder_name.clone(), parent_folder_id.clone()).await? {
                    Some(existing) => existing.id,
                    None => {
                        let folder = Folder {
                            id: gen_id(),
                            name: folder_name,
                            parent_id: parent_folder_id.clone(),
                            created_at: Utc::now().to_rfc3339(),
                        };
                        let new_id = folder.id.clone();
                        let db2 = Arc::clone(db);
                        spawn_blocking(move || -> Result<()> {
                            let rw = db2.rw_transaction()?;
                            rw.insert(folder)?;
                            rw.commit()?;
                            Ok(())
                        }).await??;
                        *folders_created += 1;
                        new_id
                    }
                };
                Box::pin(process_outlines(db, &outline.outlines, Some(folder_id), folders_created, feeds_added, feeds_skipped)).await?;
            }
        }
        Ok(())
    }

    process_outlines(&db, &doc.body.outlines, None, &mut folders_created, &mut feeds_added, &mut feeds_skipped).await?;

    Ok(ImportResult { folders_created, feeds_added, feeds_skipped })
}

/// Export the current feed/folder graph as an OPML 2.0 document.
pub async fn export_core(db: Arc<Database<'static>>) -> Result<String> {
    let (folders, feeds) = spawn_blocking(move || -> Result<(Vec<Folder>, Vec<Feed>)> {
        let r = db.r_transaction()?;
        let folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|f| f.ok()).collect();
        let feeds: Vec<Feed> = r.scan().primary::<Feed>()?.all()?.filter_map(|f| f.ok()).collect();
        Ok((folders, feeds))
    }).await??;

    let mut by_parent: HashMap<Option<String>, Vec<&Folder>> = HashMap::new();
    for f in &folders { by_parent.entry(f.parent_id.clone()).or_default().push(f); }
    for v in by_parent.values_mut() { v.sort_by(|a, b| a.name.cmp(&b.name)); }

    let mut feeds_in: HashMap<Option<String>, Vec<&Feed>> = HashMap::new();
    for f in &feeds { feeds_in.entry(f.folder_id.clone()).or_default().push(f); }
    for v in feeds_in.values_mut() {
        v.sort_by(|a, b| a.display_title().cmp(b.display_title()));
    }

    let mut doc = opml::OPML {
        head: Some(opml::Head { title: Some("rssdude".into()), ..Default::default() }),
        ..Default::default()
    };
    let body = &mut doc.body;
    for folder in by_parent.get(&None).cloned().unwrap_or_default() {
        body.outlines.push(folder_to_outline(folder, &by_parent, &feeds_in));
    }
    for feed in feeds_in.get(&None).cloned().unwrap_or_default() {
        body.outlines.push(feed_outline(feed));
    }
    doc.to_string().context("failed to serialize OPML")
}

fn feed_outline(feed: &Feed) -> opml::Outline {
    opml::Outline {
        text: feed.display_title().to_string(),
        title: feed.title.clone(),
        xml_url: Some(feed.url.clone()),
        r#type: Some("rss".into()),
        ..Default::default()
    }
}

fn folder_to_outline(
    folder: &Folder,
    by_parent: &HashMap<Option<String>, Vec<&Folder>>,
    feeds_in: &HashMap<Option<String>, Vec<&Feed>>,
) -> opml::Outline {
    let mut out = opml::Outline { text: folder.name.clone(), ..Default::default() };
    for child in by_parent.get(&Some(folder.id.clone())).cloned().unwrap_or_default() {
        out.outlines.push(folder_to_outline(child, by_parent, feeds_in));
    }
    for feed in feeds_in.get(&Some(folder.id.clone())).cloned().unwrap_or_default() {
        out.outlines.push(feed_outline(feed));
    }
    out
}

// ---------------------------------------------------------------------------
// CLI wrappers
// ---------------------------------------------------------------------------

pub async fn import(db: Arc<Database<'static>>, json: bool, file: String) -> Result<()> {
    let content = tokio::fs::read_to_string(&file)
        .await
        .with_context(|| format!("failed to read OPML file: {file}"))?;
    let result = import_core(db, content).await?;
    if json {
        print_json(&result);
    } else {
        println!(
            "Imported {} feeds, {} folders ({} feeds skipped as duplicates).",
            result.feeds_added, result.folders_created, result.feeds_skipped
        );
    }
    Ok(())
}

pub async fn export(db: Arc<Database<'static>>, json: bool, output: Option<String>) -> Result<()> {
    let xml = export_core(db).await?;
    if json {
        print_json(&serde_json::json!({"opml": xml}));
        return Ok(());
    }
    match output {
        Some(path) => {
            tokio::fs::write(&path, &xml)
                .await
                .with_context(|| format!("failed to write OPML to {path}"))?;
            eprintln!("Wrote OPML to {path}");
        }
        None => println!("{xml}"),
    }
    Ok(())
}
