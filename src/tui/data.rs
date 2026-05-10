use super::*;

/// Pull a fresh snapshot from the backend and assemble the indexed
/// `BrowserData` the panes consume. Indexing (HashMaps + per-feed stats) lives
/// here rather than on the wire so the snapshot payload stays compact.
pub(super) async fn load_browser_data(backend: Arc<Backend>) -> Result<BrowserData> {
    let snap = backend.snapshot().await?;
    let mut feeds = snap.feeds;
    let mut folders = snap.folders;
    let mut items = snap.items;
    let marks_vec = snap.marks;

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

    let marks_slice: Vec<Mark> = marks.values().cloned().collect();
    let feed_stats = compute_feed_stats(&items, &marks_slice);
    let total_unread = feed_stats.values().map(|s| s.unread).sum();
    let total_starred = feed_stats.values().map(|s| s.starred).sum();

    Ok(BrowserData {
        feeds,
        folders,
        items,
        feed_lookup,
        marks,
        feed_stats,
        total_unread,
        total_starred,
        boards: snap.boards,
        board_items: snap.board_items,
        watches: snap.watches,
    })
}

/// Drive a sync via the backend, translating progress into `AppMessage`s the
/// event loop can consume. Always emits a final `SyncFinished` even on error.
pub(super) async fn run_sync_task(
    backend: Arc<Backend>,
    feed_id: Option<String>,
    tx: Sender<AppMessage>,
) {
    let tx2 = tx.clone();
    let started = std::sync::Mutex::new(false);
    let cb: backend::ProgressCb = Box::new(move |completed, total| {
        let mut s = started.lock().unwrap();
        if !*s {
            *s = true;
            let _ = tx2.send(AppMessage::SyncStarted {
                total,
                scope: if total == 1 { "feed".into() } else { "feeds".into() },
            });
        }
        let _ = tx2.send(AppMessage::SyncProgress { completed, total });
    });

    match backend.sync(feed_id, cb).await {
        Ok(result) => {
            let errors: Vec<String> = result
                .results
                .iter()
                .filter(|r| r.status == "error")
                .map(|r| format!("{}: {}", r.title, r.error.as_deref().unwrap_or("?")))
                .collect();
            let _ = tx.send(AppMessage::SyncFinished {
                total_new: result.new_items,
                errors,
            });
        }
        Err(err) => {
            let _ = tx.send(AppMessage::SyncFinished {
                total_new: 0,
                errors: vec![format!("{err:#}")],
            });
        }
    }
}
