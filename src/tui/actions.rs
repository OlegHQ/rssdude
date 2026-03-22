use super::*;

impl App {
    pub(super) fn open_digest_overlay(&mut self) {
        let db = Arc::clone(&self.db);
        let tx = self.tx.clone();
        let since = self.since_filter.clone().unwrap_or_else(|| "24h".to_string());
        tokio::spawn(async move {
            let lines = helpers::compute_digest(db, &since).await.unwrap_or_else(|e| vec![format!("Error: {e:#}")]);
            let _ = tx.send(AppMessage::DigestReady(lines));
        });
    }

    pub(super) fn open_trending_overlay(&mut self) {
        let db = Arc::clone(&self.db);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let lines = helpers::compute_trending(db).await.unwrap_or_else(|e| vec![format!("Error: {e:#}")]);
            let _ = tx.send(AppMessage::TrendingReady(lines));
        });
    }

    pub(super) fn open_current_item(&mut self) {
        if self.current_item().is_some() {
            self.focus = Focus::Preview;
            self.preview_scroll = 0;
            self.toggle_current_read();
        }
    }

    pub(super) fn toggle_current_read(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".to_string(), true);
            return;
        };
        let db = Arc::clone(&self.db);
        self.spawn_action(true, data::toggle_mark_action(db, item.item.id, data::ToggleField::Read));
    }

    pub(super) fn toggle_current_star(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".to_string(), true);
            return;
        };
        let db = Arc::clone(&self.db);
        self.spawn_action(true, data::toggle_mark_action(db, item.item.id, data::ToggleField::Star));
    }

    pub(super) fn open_current_link(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".to_string(), true);
            return;
        };
        let Some(url) = item.item.link.clone() else {
            self.set_flash("This item has no link.".to_string(), true);
            return;
        };
        match Command::new("open").arg(&url).spawn() {
            Ok(_) => {
                self.set_flash(format!("Opened {url}"), false);
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::toggle_mark_action(db, item.item.id, data::ToggleField::Read));
            }
            Err(err) => self.set_flash(format!("Failed to open link: {err}"), true),
        }
    }

    pub(super) fn toggle_current_read_later(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".into(), true);
            return;
        };
        let db = Arc::clone(&self.db);
        self.spawn_action(true, data::toggle_read_later_action(db, item.item.id));
    }

    pub(super) fn fetch_full_article(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".into(), true);
            return;
        };
        let db = Arc::clone(&self.db);
        self.spawn_action(true, data::fetch_full_article_action(db, item.item.id));
    }

    pub(super) fn bulk_mark_read(&mut self) {
        let ids: Vec<String> = self.selected_items.drain().collect();
        let db = Arc::clone(&self.db);
        let count = ids.len();
        self.spawn_action(true, async move {
            for id in ids {
                let _ = data::toggle_mark_action(Arc::clone(&db), id, data::ToggleField::Read).await;
            }
            Ok(format!("Marked {count} items as read."))
        });
        self.visual_mode = false;
    }

    pub(super) fn bulk_star(&mut self) {
        let ids: Vec<String> = self.selected_items.drain().collect();
        let db = Arc::clone(&self.db);
        let count = ids.len();
        self.spawn_action(true, async move {
            for id in ids {
                let _ = data::toggle_mark_action(Arc::clone(&db), id, data::ToggleField::Star).await;
            }
            Ok(format!("Starred {count} items."))
        });
        self.visual_mode = false;
    }

    pub(super) fn submit_input_modal(&mut self, modal: InputModal) {
        let values: Vec<String> = modal.fields.into_iter().map(|f| f.value.trim().to_string()).collect();

        match modal.purpose {
            InputPurpose::Search => {
                self.search_query = values.first().cloned().unwrap_or_default();
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            InputPurpose::AddFeed { folder_id } => {
                let url = values.first().cloned().unwrap_or_default();
                if url.is_empty() {
                    self.set_flash("Feed URL cannot be empty.".to_string(), true);
                    return;
                }
                let tags: Vec<String> = values.get(1)
                    .map(|v| v.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
                    .unwrap_or_default();
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::add_feed_action(db, url, tags, folder_id));
            }
            InputPurpose::NewFolder { parent_id } => {
                let name = values.first().cloned().unwrap_or_default();
                if name.is_empty() {
                    self.set_flash("Folder name cannot be empty.".to_string(), true);
                    return;
                }
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::create_folder_action(db, name, parent_id));
            }
            InputPurpose::RenameFolder { folder_id } => {
                let name = values.first().cloned().unwrap_or_default();
                if name.is_empty() {
                    self.set_flash("Folder name cannot be empty.".to_string(), true);
                    return;
                }
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::rename_folder_action(db, folder_id, name));
            }
            InputPurpose::EditNote { item_id } => {
                let note = values.first().cloned().unwrap_or_default();
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::save_note_action(db, item_id, note));
            }
            InputPurpose::CreateBoard => {
                let name = values.first().cloned().unwrap_or_default();
                if name.is_empty() { self.set_flash("Board name cannot be empty.".into(), true); return; }
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::create_board_action(db, name));
            }
            InputPurpose::ImportOpml => {
                let path = values.first().cloned().unwrap_or_default();
                if path.is_empty() { self.set_flash("File path cannot be empty.".into(), true); return; }
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::import_opml_action(db, path));
            }
            InputPurpose::CreateWatch => {
                let name = values.first().cloned().unwrap_or_default();
                let query = values.get(1).cloned().unwrap_or_default();
                if name.is_empty() || query.is_empty() { self.set_flash("Name and query required.".into(), true); return; }
                let db = Arc::clone(&self.db);
                self.spawn_action(true, data::create_watch_action(db, name, query));
            }
        }
    }

    pub(super) fn submit_picker_modal(&mut self, modal: PickerModal) {
        let Some(choice) = modal.entries.get(modal.selected) else { return; };

        match modal.purpose {
            PickerPurpose::MoveFeed { feed_id } => {
                let db = Arc::clone(&self.db);
                let folder_id = choice.folder_id.clone();
                self.spawn_action(true, data::move_feed_action(db, feed_id, folder_id));
            }
            PickerPurpose::MoveFolder { folder_id } => {
                let db = Arc::clone(&self.db);
                let parent_id = choice.folder_id.clone();
                self.spawn_action(true, data::move_folder_action(db, folder_id, parent_id));
            }
            PickerPurpose::FilterByTag => {
                if choice.label == "(clear filter)" {
                    self.tag_filter = None;
                    self.set_flash("Tag filter cleared.".to_string(), false);
                } else {
                    self.tag_filter = Some(choice.label.clone());
                    self.set_flash(format!("Filtering by tag: {}", choice.label), false);
                }
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            PickerPurpose::FilterByTime => {
                let duration = match choice.label.as_str() {
                    "Last 1h" => Some("1h".to_string()),
                    "Last 24h" => Some("24h".to_string()),
                    "Last 7d" => Some("7d".to_string()),
                    "Last 30d" => Some("30d".to_string()),
                    _ => None,
                };
                if let Some(ref d) = duration {
                    self.set_flash(format!("Filtering: last {d}"), false);
                } else {
                    self.set_flash("Time filter cleared.".to_string(), false);
                }
                self.since_filter = duration;
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            PickerPurpose::ExportItem { item_id } => {
                let (ext, format_name) = match choice.label.as_str() {
                    "Markdown" => ("md", "Markdown"),
                    "JSON" => ("json", "JSON"),
                    _ => ("txt", "Text"),
                };
                let db = Arc::clone(&self.db);
                let ext = ext.to_string();
                let format_name = format_name.to_string();
                self.spawn_action(false, helpers::export_item_to_file(db, item_id, ext, format_name));
            }
            PickerPurpose::AddToBoard { item_id } => {
                if let Some(board_id) = choice.folder_id.clone() {
                    let db = Arc::clone(&self.db);
                    self.spawn_action(true, data::add_to_board_action(db, board_id, item_id));
                }
            }
        }
    }

    pub(super) fn submit_confirm_modal(&mut self, modal: ConfirmModal) {
        let db = Arc::clone(&self.db);
        match modal.action {
            ConfirmAction::DeleteFeed { feed_id } => {
                self.spawn_action(true, data::remove_feed_action(db, feed_id));
            }
            ConfirmAction::DeleteFolder { folder_id, recursive } => {
                self.spawn_action(true, data::delete_folder_action(db, folder_id, recursive));
            }
            ConfirmAction::MarkAllRead { scope, scope_id } => {
                self.spawn_action(true, data::mark_all_read_action(db, scope, scope_id));
            }
            ConfirmAction::DeleteBoard { board_id } => {
                self.spawn_action(true, data::delete_board_action(db, board_id));
            }
        }
    }
}
