use super::*;

pub(super) async fn load_browser_data(db: Arc<Database<'static>>) -> Result<BrowserData> {
    spawn_blocking(move || {
        let r = db.r_transaction().context("read transaction")?;
        let mut feeds: Vec<Feed> = r
            .scan()
            .primary()?
            .all()?
            .filter_map(|row| row.ok())
            .collect();
        let mut folders: Vec<Folder> = r
            .scan()
            .primary::<Folder>()?
            .all()?
            .filter_map(|row| row.ok())
            .collect();
        let mut items: Vec<Item> = r
            .scan()
            .primary()?
            .all()?
            .filter_map(|row| row.ok())
            .collect();
        let marks_vec: Vec<Mark> = r
            .scan()
            .primary()?
            .all()?
            .filter_map(|row| row.ok())
            .collect();
        let mute_filters: Vec<MuteFilter> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let boards: Vec<Board> = r.scan().primary()?.all()?.filter_map(|b| b.ok()).collect();
        let board_items: Vec<BoardItem> = r.scan().primary()?.all()?.filter_map(|b| b.ok()).collect();
        let watches: Vec<SavedSearch> = r.scan().primary()?.all()?.filter_map(|s| s.ok()).collect();

        feeds.sort_by_key(helpers::feed_sort_key);
        folders.sort_by_cached_key(|f| f.name.to_lowercase());
        items.sort_by(|a, b| helpers::item_time_key(b).cmp(helpers::item_time_key(a)));

        let feed_lookup: HashMap<String, Feed> = feeds
            .iter()
            .cloned()
            .map(|feed| (feed.id.clone(), feed))
            .collect();
        let marks: HashMap<String, Mark> = marks_vec
            .into_iter()
            .map(|mark| (mark.item_id.clone(), mark))
            .collect();

        let marks_vec: Vec<Mark> = marks.values().cloned().collect();
        let feed_stats = compute_feed_stats(&items, &marks_vec);

        let total_unread = feed_stats.values().map(|stats| stats.unread).sum();
        let total_starred = feed_stats.values().map(|stats| stats.starred).sum();

        Ok::<_, anyhow::Error>(BrowserData {
            feeds,
            folders,
            items,
            feed_lookup,
            marks,
            feed_stats,
            total_unread,
            total_starred,
            mute_filters,
            boards,
            board_items,
            watches,
        })
    })
    .await?
}

pub(super) async fn run_sync_task(
    db: Arc<Database<'static>>,
    feed_id: Option<String>,
    tx: Sender<AppMessage>,
) {
    let sync_result = sync_feeds_action(db, feed_id, tx.clone()).await;
    if let Err(err) = sync_result {
        // Ensure syncing state is always cleared, even on error
        let _ = tx.send(AppMessage::SyncFinished {
            total_new: 0,
            errors: vec![format!("{err:#}")],
        });
    }
}

async fn sync_feeds_action(
    db: Arc<Database<'static>>,
    feed_id: Option<String>,
    tx: Sender<AppMessage>,
) -> Result<()> {
    let tx2 = tx.clone();
    let result = crate::commands::sync::sync_core(
        db,
        feed_id,
        Some(Box::new(move |completed, total, _feed_result, _total_new| {
            if completed == 1 {
                let _ = tx2.send(AppMessage::SyncStarted {
                    total,
                    scope: if total == 1 { "feed".to_string() } else { "feeds".to_string() },
                });
            }
            let _ = tx2.send(AppMessage::SyncProgress {
                completed,
                total,
            });
        })),
    ).await?;

    let errors: Vec<String> = result.results.iter()
        .filter(|r| r.status == "error")
        .map(|r| format!("{}: {}", r.title, r.error.as_deref().unwrap_or("?")))
        .collect();
    let _ = tx.send(AppMessage::SyncFinished { total_new: result.new_items, errors });
    Ok(())
}

pub(super) async fn add_feed_action(
    db: Arc<Database<'static>>,
    url: String,
    tags: Vec<String>,
    folder_id: Option<String>,
) -> Result<String> {
    let (feed, count) = crate::commands::feed_mgmt::add_core(db, url, tags, folder_id).await?;
    let title = feed.title.as_deref().unwrap_or("(untitled)");
    Ok(format!("Added feed \"{title}\" with {count} items."))
}

pub(super) async fn create_folder_action(
    db: Arc<Database<'static>>,
    name: String,
    parent_id: Option<String>,
) -> Result<String> {
    let folder = crate::commands::folder::create_core(db, name, parent_id).await?;
    Ok(format!("Created folder \"{}\".", folder.name))
}

pub(super) async fn rename_folder_action(
    db: Arc<Database<'static>>,
    folder_id: String,
    name: String,
) -> Result<String> {
    let folder = crate::commands::folder::rename_core(db, folder_id, name).await?;
    Ok(format!("Renamed folder \"{}\".", folder.name))
}

pub(super) async fn move_folder_action(
    db: Arc<Database<'static>>,
    folder_id: String,
    parent_id: Option<String>,
) -> Result<String> {
    let folder = crate::commands::folder::move_folder_core(db, folder_id, parent_id).await?;
    Ok(format!("Moved folder \"{}\".", folder.name))
}

pub(super) async fn delete_folder_action(
    db: Arc<Database<'static>>,
    folder_id: String,
    recursive: bool,
) -> Result<String> {
    let result = crate::commands::folder::delete_core(db, folder_id, recursive).await?;
    let name = result["folder"].as_str().unwrap_or("?");
    if recursive {
        Ok(format!("Deleted folder \"{name}\" recursively."))
    } else {
        Ok(format!("Deleted folder \"{name}\" and reparented its children."))
    }
}

pub(super) async fn move_feed_action(
    db: Arc<Database<'static>>,
    feed_id: String,
    folder_id: Option<String>,
) -> Result<String> {
    let (feed_title, _) = crate::commands::feed_mgmt::move_to_folder_core(db, feed_id, folder_id).await?;
    Ok(format!("Moved feed \"{feed_title}\"."))
}

pub(super) async fn remove_feed_action(db: Arc<Database<'static>>, feed_id: String) -> Result<String> {
    let (title, _) = crate::commands::feed_mgmt::remove_core(db, feed_id).await?;
    Ok(format!("Removed feed \"{title}\"."))
}

/// Toggle read or starred. `new_read` / `new_star` are the desired new values (caller computes the toggle).
pub(super) async fn toggle_mark_action(db: Arc<Database<'static>>, item_id: String, new_read: Option<bool>, new_star: Option<bool>) -> Result<String> {
    let (item, _) = crate::commands::curate::mark_core(db, item_id, new_read, new_star, None).await?;
    let label = match (new_read, new_star) {
        (Some(true), _) => "Marked read:",
        (Some(false), _) => "Marked unread:",
        (_, Some(true)) => "Starred:",
        (_, Some(false)) => "Unstarred:",
        _ => "Updated:",
    };
    Ok(format!("{label} {}", item.title.unwrap_or_else(|| "(untitled)".to_string())))
}

pub(super) async fn save_note_action(db: Arc<Database<'static>>, item_id: String, note: String) -> Result<String> {
    let (item, _) = crate::commands::curate::mark_core(db, item_id, None, None, Some(note)).await?;
    Ok(format!("Saved note for {}", item.title.unwrap_or_else(|| "(untitled)".to_string())))
}

pub(super) async fn mark_all_read_action(
    db: Arc<Database<'static>>,
    scope: String,
    scope_id: Option<String>,
) -> Result<String> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let items: Vec<Item> = rw.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let folders: Vec<Folder> = rw.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();

        let target_ids: std::collections::HashSet<String> = match (scope.as_str(), &scope_id) {
            ("feed", Some(fid)) => items.iter().filter(|i| i.feed_id == *fid).map(|i| i.id.clone()).collect(),
            ("folder", Some(fid)) => {
                let desc = crate::shared::db::collect_descendant_ids(&folders, fid);
                let feeds: Vec<Feed> = rw.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
                let feed_ids: std::collections::HashSet<String> = feeds.iter()
                    .filter(|f| f.folder_id.as_ref().is_some_and(|id| id == fid || desc.contains(id)))
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
        Ok(format!("Marked {count} items as read."))
    }).await?
}

pub(super) async fn toggle_read_later_action(db: Arc<Database<'static>>, item_id: String) -> Result<String> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let item: Item = rw.get().primary(item_id.clone())?.ok_or_else(|| anyhow::anyhow!("Item not found."))?;
        let existing: Option<Mark> = rw.get().primary(item_id.clone()).ok().flatten();
        let was_later = existing.as_ref().is_some_and(|m| m.read_later);
        let mut mark = Mark::from_existing(item_id, existing.as_ref());
        mark.read_later = !was_later;
        let _: Option<Mark> = rw.upsert(mark)?;
        rw.commit()?;
        let title = item.title.unwrap_or_else(|| "(untitled)".into());
        Ok(if was_later { format!("Removed from read later: {title}") } else { format!("Added to read later: {title}") })
    }).await?
}

pub(super) async fn create_board_action(db: Arc<Database<'static>>, name: String) -> Result<String> {
    crate::commands::board::create_core(db, name.clone()).await?;
    Ok(format!("Created board \"{name}\"."))
}

pub(super) async fn add_to_board_action(db: Arc<Database<'static>>, board_id: String, item_id: String) -> Result<String> {
    crate::commands::board::add_item_core(db, board_id, item_id, None).await?;
    Ok("Added to board.".into())
}

pub(super) async fn delete_board_action(db: Arc<Database<'static>>, id: String) -> Result<String> {
    let name = crate::commands::board::delete_core(db, id).await?;
    Ok(format!("Deleted board \"{name}\"."))
}

pub(super) async fn create_watch_action(db: Arc<Database<'static>>, name: String, query: String) -> Result<String> {
    let ss = crate::commands::watch::create_core(db, name, query).await?;
    Ok(format!("Created watch \"{}\".", ss.name))
}

pub(super) async fn import_opml_action(db: Arc<Database<'static>>, path: String) -> Result<String> {
    let content = tokio::fs::read_to_string(&path).await?;
    let result = crate::commands::opml::import_core(db, content).await?;
    Ok(format!("Imported {} feeds, {} folders.", result.feeds_added, result.folders_created))
}

pub(super) async fn fetch_full_article_action(db: Arc<Database<'static>>, item_id: String) -> Result<String> {
    let db2 = Arc::clone(&db);
    let id2 = item_id.clone();
    let (url, existing_full) = spawn_blocking(move || -> Result<(Option<String>, Option<String>)> {
        let r = db2.r_transaction()?;
        let item: Item = r.get().primary(id2)?.context("item not found")?;
        Ok((item.link.clone(), item.full_content.clone()))
    }).await??;
    if let Some(content) = existing_full {
        return Ok(format!("Full article already cached ({} chars).", content.len()));
    }
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
    Ok(format!("Full article fetched ({} chars).", text.len()))
}
