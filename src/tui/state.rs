use super::*;
impl App {
    pub(super) fn new(db: Arc<Database<'static>>, tx: Sender<AppMessage>, rx: Receiver<AppMessage>) -> Self {
        Self {
            db,
            tx,
            rx,
            data: None,
            focus: Focus::Sidebar,
            modal: None,
            show_help: false,
            should_quit: false,
            unread_only: false,
            search_query: String::new(),
            tag_filter: None,
            since_filter: None,
            digest_content: None,
            digest_scroll: 0,
            trending_content: None,
            trending_scroll: 0,
            sidebar_index: 0,
            sidebar_offset: 0,
            item_index: 0,
            items_offset: 0,
            preview_scroll: 0,
            syncing: None,
            refreshing: false,
            pending_refresh: false,
            flash: None,
            last_auto_sync: Instant::now() - AUTO_SYNC_INTERVAL,
            last_item_click: None,
            layout: None,
            preview_links: Vec::new(),
        }
    }

    /// Half the visible height of the focused pane (for Ctrl+D / Ctrl+U).
    pub(super) fn half_page(&self) -> usize {
        let h = self.layout.map(|l| match self.focus {
            Focus::Sidebar => l.sidebar_inner.height,
            Focus::Items => l.items_inner.height / 2, // items use 2 lines per entry
            Focus::Preview => l.preview.height,
        }).unwrap_or(10);
        (h as usize / 2).max(1)
    }

    /// Spawn an async action and send the result as ActionFinished.
    pub(super) fn spawn_action<F>(&self, refresh: bool, fut: F)
    where
        F: std::future::Future<Output = Result<String>> + Send + 'static,
    {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fut.await.map_err(|err| format!("{err:#}"));
            let _ = tx.send(AppMessage::ActionFinished { result, refresh });
        });
    }

    pub(super) fn request_refresh(&mut self) {
        if self.refreshing {
            self.pending_refresh = true;
            return;
        }

        self.refreshing = true;
        let db = Arc::clone(&self.db);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = data::load_browser_data(db)
                .await
                .map_err(|err| format!("{err:#}"));
            let _ = tx.send(AppMessage::DataLoaded(result));
        });
    }

    pub(super) fn maybe_auto_sync(&mut self) {
        if self.syncing.is_none() && self.last_auto_sync.elapsed() >= AUTO_SYNC_INTERVAL {
            self.start_sync(None);
        }
    }

    pub(super) fn start_sync(&mut self, feed_id: Option<String>) {
        if self.syncing.is_some() {
            return;
        }

        self.last_auto_sync = Instant::now();
        let db = Arc::clone(&self.db);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            data::run_sync_task(db, feed_id, tx).await;
        });
    }

    pub(super) fn handle_messages(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AppMessage::DataLoaded(result) => {
                    self.refreshing = false;
                    match result {
                        Ok(data) => {
                            self.data = Some(data);
                            self.clamp_selection();
                        }
                        Err(err) => self.set_flash(err, true),
                    }
                    if self.pending_refresh {
                        self.pending_refresh = false;
                        self.request_refresh();
                    }
                }
                AppMessage::ActionFinished { result, refresh } => match result {
                    Ok(message) => {
                        self.set_flash(message, false);
                        if refresh {
                            self.request_refresh();
                        }
                    }
                    Err(err) => self.set_flash(err, true),
                },
                AppMessage::SyncStarted { total, scope } => {
                    self.syncing = Some(SyncState {
                        total,
                        completed: 0,
                        current: String::new(),
                        total_new: 0,
                        scope,
                    });
                }
                AppMessage::SyncProgress {
                    completed,
                    total,
                    title,
                    total_new,
                } => {
                    self.syncing = Some(SyncState {
                        total,
                        completed,
                        current: title,
                        total_new,
                        scope: self
                            .syncing
                            .as_ref()
                            .map(|state| state.scope.clone())
                            .unwrap_or_else(|| "feeds".to_string()),
                    });
                }
                AppMessage::SyncFinished { total_new, errors } => {
                    self.syncing = None;
                    if errors.is_empty() {
                        self.set_flash(format!("Sync finished: {total_new} new items."), false);
                    } else {
                        let details = errors.join(" | ");
                        self.set_flash(
                            format!("Sync: +{total_new} items, {} errors: {details}", errors.len()),
                            true,
                        );
                    }
                    self.request_refresh();
                }
                AppMessage::DigestReady(lines) => {
                    self.digest_content = Some(lines);
                    self.digest_scroll = 0;
                }
                AppMessage::TrendingReady(lines) => {
                    self.trending_content = Some(lines);
                    self.trending_scroll = 0;
                }
            }
        }
    }

    pub(super) fn set_flash(&mut self, text: String, is_error: bool) {
        self.flash = Some(FlashMessage {
            text,
            is_error,
            at: Instant::now(),
        });
    }

    pub(super) fn status_text(&self) -> (String, bool) {
        if let Some(sync) = &self.syncing {
            return (
                format!(
                    "syncing {} {}/{}  {}  +{}",
                    sync.scope, sync.completed, sync.total, sync.current, sync.total_new
                ),
                false,
            );
        }

        if self.refreshing {
            return ("refreshing data".to_string(), false);
        }

        if let Some(flash) = &self.flash {
            if flash.at.elapsed() <= FLASH_TTL {
                return (flash.text.clone(), flash.is_error);
            }
        }

        let Some(data) = &self.data else {
            return ("loading data".to_string(), false);
        };

        (
            format!(
                "{} feeds  {} unread  {} starred",
                data.feeds.len(),
                data.total_unread,
                data.total_starred
            ),
            false,
        )
    }

    pub(super) fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        let Some(data) = &self.data else {
            return vec![
                SidebarEntry {
                    kind: SidebarKind::All,
                    label: "All Items".to_string(),
                    unread: 0,
                    depth: 0,
                },
                SidebarEntry {
                    kind: SidebarKind::Starred,
                    label: "Starred".to_string(),
                    unread: 0,
                    depth: 0,
                },
            ];
        };

        let mut entries = vec![
            SidebarEntry {
                kind: SidebarKind::All,
                label: "All Items".to_string(),
                unread: data.total_unread,
                depth: 0,
            },
            SidebarEntry {
                kind: SidebarKind::Starred,
                label: "Starred".to_string(),
                unread: data.total_starred,
                depth: 0,
            },
        ];

        let mut feeds_by_folder: HashMap<Option<String>, Vec<&Feed>> = HashMap::new();
        for feed in &data.feeds {
            feeds_by_folder
                .entry(feed.folder_id.clone())
                .or_default()
                .push(feed);
        }
        for feeds in feeds_by_folder.values_mut() {
            feeds.sort_by_key(|feed| helpers::feed_sort_key(feed));
        }

        let mut folders_by_parent: HashMap<Option<String>, Vec<&Folder>> = HashMap::new();
        for folder in &data.folders {
            folders_by_parent
                .entry(folder.parent_id.clone())
                .or_default()
                .push(folder);
        }
        for folders in folders_by_parent.values_mut() {
            folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }

        if let Some(root_folders) = folders_by_parent.get(&None) {
            for folder in root_folders {
                helpers::push_folder_entries(
                    folder,
                    0,
                    &mut entries,
                    &feeds_by_folder,
                    &folders_by_parent,
                    &data.feed_stats,
                );
            }
        }

        if let Some(uncategorized) = feeds_by_folder.get(&None) {
            if !uncategorized.is_empty() {
                let unread: usize = uncategorized
                    .iter()
                    .map(|feed| {
                        data.feed_stats
                            .get(&feed.id)
                            .map(|stats| stats.unread)
                            .unwrap_or(0)
                    })
                    .sum();
                entries.push(SidebarEntry {
                    kind: SidebarKind::Uncategorized,
                    label: "Uncategorized".to_string(),
                    unread,
                    depth: 0,
                });
                for feed in uncategorized {
                    entries.push(SidebarEntry {
                        kind: SidebarKind::Feed(feed.id.clone()),
                        label: helpers::feed_label(feed),
                        unread: data
                            .feed_stats
                            .get(&feed.id)
                            .map(|stats| stats.unread)
                            .unwrap_or(0),
                        depth: 1,
                    });
                }
            }
        }

        entries
    }

    pub(super) fn selected_scope(&self) -> SidebarKind {
        self.sidebar_entries()
            .get(self.sidebar_index)
            .map(|entry| entry.kind.clone())
            .unwrap_or(SidebarKind::All)
    }

    pub(super) fn visible_items(&self) -> Vec<VisibleItem> {
        let Some(data) = &self.data else {
            return Vec::new();
        };

        let scope = self.selected_scope();
        let descendant_ids = match &scope {
            SidebarKind::Folder(folder_id) => {
                Some(collect_descendant_ids(&data.folders, folder_id))
            }
            _ => None,
        };
        let query = self.search_query.to_lowercase();

        let since_cutoff = self.since_filter.as_ref().and_then(|s| {
            crate::output::parse_duration(s).ok().map(|dur| chrono::Utc::now().naive_utc() - dur)
        });

        data.items
            .iter()
            .filter(|item| helpers::item_matches_scope(item, &scope, data, descendant_ids.as_ref()))
            .filter(|item| helpers::item_matches_query(item, &query))
            .filter(|item| {
                if !self.unread_only {
                    return true;
                }
                !data.marks.get(&item.id).is_some_and(|mark| mark.read)
            })
            .filter(|item| {
                if let Some(ref tag) = self.tag_filter {
                    data.feed_lookup.get(&item.feed_id).is_some_and(|feed| {
                        feed.tags.split(',').any(|t| t.trim().eq_ignore_ascii_case(tag))
                    })
                } else {
                    true
                }
            })
            .filter(|item| {
                if let Some(ref cutoff) = since_cutoff {
                    item.published_at.as_ref().and_then(|p| {
                        chrono::DateTime::parse_from_rfc3339(p).ok().map(|d| d.naive_utc())
                    }).is_some_and(|dt| dt >= *cutoff)
                } else {
                    true
                }
            })
            .map(|item| VisibleItem {
                item: item.clone(),
                feed: data.feed_lookup.get(&item.feed_id).cloned(),
                mark: data.marks.get(&item.id).cloned(),
            })
            .collect()
    }

    pub(super) fn current_item(&self) -> Option<VisibleItem> {
        self.visible_items().get(self.item_index).cloned()
    }

    pub(super) fn clamp_selection(&mut self) {
        let sidebar_len = self.sidebar_entries().len();
        self.sidebar_index = helpers::clamp_index(self.sidebar_index, sidebar_len);
        let items_len = self.visible_items().len();
        self.item_index = helpers::clamp_index(self.item_index, items_len);
    }

    pub(super) fn selected_folder_context(&self) -> Option<String> {
        match self.selected_scope() {
            SidebarKind::Folder(id) => Some(id),
            SidebarKind::Feed(id) => self.data.as_ref()
                .and_then(|data| data.feed_lookup.get(&id))
                .and_then(|feed| feed.folder_id.clone()),
            _ => None,
        }
    }

    pub(super) fn selected_feed_context(&self) -> Option<String> {
        let scope = self.selected_scope();
        let current_item = self.current_item();
        helpers::sync_target_feed_id(
            &scope,
            current_item.as_ref().map(|item| item.item.feed_id.as_str()),
        )
    }

    pub(super) fn start_current_sync(&mut self) {
        match self.selected_feed_context() {
            Some(feed_id) => self.start_sync(Some(feed_id)),
            None => self.set_flash(
                "Select a feed or an item to sync a specific feed.".to_string(),
                true,
            ),
        }
    }
}
