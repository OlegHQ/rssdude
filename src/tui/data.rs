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

        feeds.sort_by_key(helpers::feed_sort_key);
        folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
    let db_for_list = Arc::clone(&db);
    let feeds: Vec<Feed> = spawn_blocking(move || {
        let r = db_for_list.r_transaction()?;
        let all: Vec<Feed> = r
            .scan()
            .primary()?
            .all()?
            .filter_map(|row| row.ok())
            .collect();
        Ok::<_, anyhow::Error>(all)
    })
    .await??;

    let feeds: Vec<Feed> = if let Some(id) = feed_id {
        let selected: Vec<Feed> = feeds.into_iter().filter(|feed| feed.id == id).collect();
        if selected.is_empty() {
            bail!("Feed not found.");
        }
        selected
    } else {
        feeds
    };

    let _ = tx.send(AppMessage::SyncStarted {
        total: feeds.len(),
        scope: if feeds.len() == 1 {
            "feed".to_string()
        } else {
            "feeds".to_string()
        },
    });

    let mut total_new = 0usize;
    let mut errors = Vec::new();

    for (index, feed_record) in feeds.iter().enumerate() {
        let title = feed_record
            .title
            .clone()
            .unwrap_or_else(|| feed_record.url.clone());

        let result = match feed::fetch_feed_conditional(
            &feed_record.url,
            feed_record.etag.as_deref(),
            feed_record.last_modified.as_deref(),
        )
        .await
        {
            Ok(result) => result,
            Err(err) => {
                errors.push(format!("{title}: {err:#}"));
                let _ = tx.send(AppMessage::SyncProgress {
                    completed: index + 1,
                    total: feeds.len(),
                    title,
                    total_new,
                });
                continue;
            }
        };

        let Some(fetch_result) = result else {
            let _ = tx.send(AppMessage::SyncProgress {
                completed: index + 1,
                total: feeds.len(),
                title,
                total_new,
            });
            continue;
        };

        let now = Utc::now().to_rfc3339();
        let items = feed::entries_to_items(&fetch_result.feed.entries, &feed_record.id, &now);
        let db_for_write = Arc::clone(&db);
        let feed_id = feed_record.id.clone();
        let etag = fetch_result.etag.clone();
        let last_modified = fetch_result.last_modified.clone();
        let new_count = spawn_blocking(move || {
            let rw = db_for_write.rw_transaction()?;
            let mut inserted = 0usize;
            for item in items {
                let existing: Option<Item> = rw.get().secondary(ItemKey::guid, item.guid.clone()).ok().flatten();
                if existing.is_none() {
                    rw.insert(item)?;
                    inserted += 1;
                }
            }
            if let Some(mut updated_feed) = rw.get().primary::<Feed>(feed_id)? {
                let old = updated_feed.clone();
                updated_feed.last_synced = Some(now);
                updated_feed.etag = etag;
                updated_feed.last_modified = last_modified;
                rw.update(old, updated_feed)?;
            }
            rw.commit()?;
            Ok::<_, anyhow::Error>(inserted)
        })
        .await??;

        total_new += new_count;
        let _ = tx.send(AppMessage::SyncProgress {
            completed: index + 1,
            total: feeds.len(),
            title,
            total_new,
        });
    }

    let _ = tx.send(AppMessage::SyncFinished { total_new, errors });
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
    if let Some(pid) = parent_id {
        let folder = crate::commands::folder::move_folder_core(db, folder_id, pid).await?;
        Ok(format!("Moved folder \"{}\".", folder.name))
    } else {
        // Move to root (no parent) — not supported by move_folder_core which requires a parent
        let db_for_write = Arc::clone(&db);
        let name = spawn_blocking(move || {
            let rw = db_for_write.rw_transaction()?;
            let folder: Folder = rw
                .get()
                .primary(folder_id.clone())?
                .ok_or_else(|| anyhow::anyhow!("Folder not found."))?;
            let name = folder.name.clone();
            let mut updated = folder.clone();
            updated.parent_id = None;
            rw.update(folder, updated)?;
            rw.commit()?;
            Ok::<_, anyhow::Error>(name)
        })
        .await??;
        Ok(format!("Moved folder \"{name}\"."))
    }
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
    if let Some(fid) = folder_id {
        let (feed_title, _) = crate::commands::feed_mgmt::move_to_folder_core(db, feed_id, fid).await?;
        Ok(format!("Moved feed \"{feed_title}\"."))
    } else {
        // Move to root (uncategorized)
        let db_for_write = Arc::clone(&db);
        let name = spawn_blocking(move || {
            let rw = db_for_write.rw_transaction()?;
            let feed: Feed = rw.get().primary(feed_id.clone())?
                .ok_or_else(|| anyhow::anyhow!("Feed not found."))?;
            let name = helpers::feed_label(&feed);
            let mut updated = feed.clone();
            updated.folder_id = None;
            rw.update(feed, updated)?;
            rw.commit()?;
            Ok::<_, anyhow::Error>(name)
        }).await??;
        Ok(format!("Moved feed \"{name}\"."))
    }
}

pub(super) async fn remove_feed_action(db: Arc<Database<'static>>, feed_id: String) -> Result<String> {
    let (title, _) = crate::commands::feed_mgmt::remove_core(db, feed_id).await?;
    Ok(format!("Removed feed \"{title}\"."))
}

pub(super) enum ToggleField { Read, Star }

pub(super) async fn toggle_mark_action(db: Arc<Database<'static>>, item_id: String, field: ToggleField) -> Result<String> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let item: Item = rw.get().primary(item_id.clone())?
            .ok_or_else(|| anyhow::anyhow!("Item not found."))?;
        let existing: Option<Mark> = rw.get().primary(item_id.clone()).ok().flatten();

        let old_read = existing.as_ref().is_some_and(|m| m.read);
        let old_star = existing.as_ref().is_some_and(|m| m.starred);
        let (new_read, new_star, label) = match field {
            ToggleField::Read => (!old_read, old_star, if !old_read { "Marked read:" } else { "Marked unread:" }),
            ToggleField::Star => (old_read, !old_star, if !old_star { "Starred:" } else { "Unstarred:" }),
        };

        let mark = Mark {
            item_id, read: new_read, starred: new_star,
            note: existing.and_then(|m| m.note.clone()),
            marked_at: Utc::now().to_rfc3339(),
        };
        let _: Option<Mark> = rw.upsert(mark)?;
        rw.commit()?;
        Ok(format!("{label} {}", item.title.unwrap_or_else(|| "(untitled)".to_string())))
    }).await?
}

pub(super) async fn save_note_action(
    db: Arc<Database<'static>>,
    item_id: String,
    note: String,
) -> Result<String> {
    let db_for_write = Arc::clone(&db);
    let message = spawn_blocking(move || {
        let rw = db_for_write.rw_transaction()?;
        let item: Item = rw
            .get()
            .primary(item_id.clone())?
            .ok_or_else(|| anyhow::anyhow!("Item not found."))?;
        let existing: Option<Mark> = rw.get().primary(item_id.clone()).ok().flatten();

        let mark = Mark {
            item_id,
            read: existing.as_ref().is_some_and(|mark| mark.read),
            starred: existing.as_ref().is_some_and(|mark| mark.starred),
            note: if note.is_empty() { None } else { Some(note) },
            marked_at: Utc::now().to_rfc3339(),
        };
        let _: Option<Mark> = rw.upsert(mark)?;
        rw.commit()?;
        Ok::<_, anyhow::Error>(format!(
            "Saved note for {}",
            item.title.unwrap_or_else(|| "(untitled)".to_string())
        ))
    })
    .await??;

    Ok(message)
}
