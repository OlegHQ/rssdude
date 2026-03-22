use super::*;
impl App {
    pub(super) fn new(db: Arc<Database<'static>>, tx: Sender<AppMessage>, rx: Receiver<AppMessage>, theme_name: &str) -> Self {
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
            theme: theme::Theme::from_name(theme_name),
            collapsed_folders: HashSet::new(),
            visual_mode: false,
            selected_items: HashSet::new(),
            sort_order: "newest".into(),
            dedup: false,
            hover_sidebar: None,
            hover_item: None,
            hover_link: None,
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

    pub(super) fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        let Some(data) = &self.data else {
            return vec![
                SidebarEntry {
                    kind: SidebarKind::All,
                    label: "All Items".to_string(),
                    unread: 0,
                    depth: 0,
                    has_error: false, is_last_child: false,
                },
                SidebarEntry {
                    kind: SidebarKind::Starred,
                    label: "Starred".to_string(),
                    unread: 0,
                    depth: 0,
                    has_error: false, is_last_child: false,
                },
            ];
        };

        let mut entries = vec![
            SidebarEntry {
                kind: SidebarKind::All,
                label: "All Items".to_string(),
                unread: data.total_unread,
                depth: 0,
                has_error: false, is_last_child: false,
            },
            SidebarEntry {
                kind: SidebarKind::Starred,
                label: "Starred".to_string(),
                unread: data.total_starred,
                depth: 0,
                has_error: false, is_last_child: false,
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
                    &self.collapsed_folders,
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
                    has_error: false, is_last_child: false,
                });
                let unc_len = uncategorized.len();
                for (i, feed) in uncategorized.iter().enumerate() {
                    entries.push(SidebarEntry {
                        kind: SidebarKind::Feed(feed.id.clone()),
                        label: helpers::feed_label(feed),
                        unread: data
                            .feed_stats
                            .get(&feed.id)
                            .map(|stats| stats.unread)
                            .unwrap_or(0),
                        depth: 1,
                        has_error: feed.error_count > 0,
                        is_last_child: i == unc_len - 1,
                    });
                }
            }
        }

        // Read Later
        let read_later_count = data.marks.values().filter(|m| m.read_later).count();
        entries.push(SidebarEntry {
            kind: SidebarKind::ReadLater,
            label: "Read Later".into(),
            unread: read_later_count,
            depth: 0,
            has_error: false, is_last_child: false,
        });

        // Recently Read
        entries.push(SidebarEntry {
            kind: SidebarKind::RecentlyRead,
            label: "Recently Read".into(),
            unread: 0,
            depth: 0,
            has_error: false, is_last_child: false,
        });

        // Boards
        for board in &data.boards {
            let count = data.board_items.iter().filter(|bi| bi.board_id == board.id).count();
            entries.push(SidebarEntry {
                kind: SidebarKind::Board(board.id.clone()),
                label: format!("[B] {}", board.name),
                unread: count,
                depth: 0,
                has_error: false, is_last_child: false,
            });
        }

        // Watches
        for watch in &data.watches {
            entries.push(SidebarEntry {
                kind: SidebarKind::Watch(watch.id.clone()),
                label: format!("[W] {}", watch.name),
                unread: 0,
                depth: 0,
                has_error: false, is_last_child: false,
            });
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

        // Board/Watch scopes use a different base item set
        let base_items: Vec<&Item> = match &scope {
            SidebarKind::Board(board_id) => {
                let item_ids: HashSet<&str> = data.board_items.iter()
                    .filter(|bi| bi.board_id == *board_id)
                    .map(|bi| bi.item_id.as_str())
                    .collect();
                data.items.iter().filter(|i| item_ids.contains(i.id.as_str())).collect()
            }
            SidebarKind::Watch(watch_id) => {
                let query = data.watches.iter()
                    .find(|w| w.id == *watch_id)
                    .map(|w| w.query.to_lowercase())
                    .unwrap_or_default();
                data.items.iter()
                    .filter(|item| helpers::item_matches_query(item, &query))
                    .collect()
            }
            _ => data.items.iter().collect(),
        };

        let descendant_ids = match &scope {
            SidebarKind::Folder(folder_id) => {
                Some(collect_descendant_ids(&data.folders, folder_id))
            }
            _ => None,
        };
        let query = self.search_query.to_lowercase();

        let since_cutoff = self.since_filter.as_ref().and_then(|s| {
            crate::shared::output::parse_duration(s).ok().map(|dur| chrono::Utc::now().naive_utc() - dur)
        });

        let mut result: Vec<VisibleItem> = base_items
            .into_iter()
            .filter(|item| helpers::item_matches_scope(item, &scope, data, descendant_ids.as_ref()))
            .filter(|item| helpers::item_matches_query(item, &query))
            .filter(|item| {
                if !self.unread_only { return true; }
                !data.marks.get(&item.id).is_some_and(|mark| mark.read)
            })
            .filter(|item| {
                if let Some(ref tag) = self.tag_filter {
                    data.feed_lookup.get(&item.feed_id).is_some_and(|feed| {
                        feed.tags.split(',').any(|t| t.trim().eq_ignore_ascii_case(tag))
                    })
                } else { true }
            })
            .filter(|item| {
                if let Some(ref cutoff) = since_cutoff {
                    item.published_at.as_ref().and_then(|p| {
                        chrono::DateTime::parse_from_rfc3339(p).ok().map(|d| d.naive_utc())
                    }).is_some_and(|dt| dt >= *cutoff)
                } else { true }
            })
            .filter(|item| !is_muted(item, data.feed_lookup.get(&item.feed_id), &data.mute_filters))
            .map(|item| VisibleItem {
                item: item.clone(),
                feed: data.feed_lookup.get(&item.feed_id).cloned(),
                mark: data.marks.get(&item.id).cloned(),
            })
            .collect();

        // Sort: RecentlyRead sorts by read_at desc; otherwise by sort_order
        if matches!(scope, SidebarKind::RecentlyRead) {
            result.sort_by(|a, b| {
                let a_at = a.mark.as_ref().and_then(|m| m.read_at.as_deref()).unwrap_or("");
                let b_at = b.mark.as_ref().and_then(|m| m.read_at.as_deref()).unwrap_or("");
                b_at.cmp(a_at)
            });
        } else if self.sort_order == "oldest" {
            result.reverse();
        }

        // Dedup by guid
        if self.dedup {
            let mut seen = HashSet::new();
            result.retain(|vi| seen.insert(vi.item.guid.clone()));
        }

        result
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
