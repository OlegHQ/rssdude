use super::*;

impl App {
    pub(super) fn open_search_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Search items".to_string(),
            hint: "Comma-separated keywords match any term. Enter applies.".to_string(),
            fields: vec![InputField {
                label: "Query".to_string(),
                value: self.search_query.clone(),
            }],
            active: 0,
            purpose: InputPurpose::Search,
        }));
    }

    pub(super) fn open_add_feed_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Add feed".to_string(),
            hint: "Tags are comma-separated. Feed goes into the selected folder context."
                .to_string(),
            fields: vec![
                InputField {
                    label: "URL".to_string(),
                    value: String::new(),
                },
                InputField {
                    label: "Tags".to_string(),
                    value: String::new(),
                },
            ],
            active: 0,
            purpose: InputPurpose::AddFeed {
                folder_id: self.selected_folder_context(),
            },
        }));
    }

    pub(super) fn open_new_folder_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Create folder".to_string(),
            hint: "The current folder is used as the parent when applicable.".to_string(),
            fields: vec![InputField {
                label: "Name".to_string(),
                value: String::new(),
            }],
            active: 0,
            purpose: InputPurpose::NewFolder {
                parent_id: self.selected_folder_context(),
            },
        }));
    }

    pub(super) fn open_rename_folder_modal(&mut self) {
        let SidebarKind::Folder(folder_id) = self.selected_scope() else {
            self.set_flash("Select a folder to rename.".to_string(), true);
            return;
        };

        let current_name = self
            .data
            .as_ref()
            .and_then(|data| data.folders.iter().find(|folder| folder.id == folder_id))
            .map(|folder| folder.name.clone())
            .unwrap_or_default();

        self.modal = Some(Modal::Input(InputModal {
            title: "Rename folder".to_string(),
            hint: "Enter saves the new name.".to_string(),
            fields: vec![InputField {
                label: "Name".to_string(),
                value: current_name,
            }],
            active: 0,
            purpose: InputPurpose::RenameFolder { folder_id },
        }));
    }

    pub(super) fn open_note_modal(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".to_string(), true);
            return;
        };

        self.modal = Some(Modal::Input(InputModal {
            title: "Edit note".to_string(),
            hint: "An empty note clears the note.".to_string(),
            fields: vec![InputField {
                label: "Note".to_string(),
                value: item.mark.and_then(|mark| mark.note).unwrap_or_default(),
            }],
            active: 0,
            purpose: InputPurpose::EditNote {
                item_id: item.item.id,
            },
        }));
    }

    pub(super) fn open_delete_modal(&mut self, recursive: bool) {
        match self.selected_scope() {
            SidebarKind::Feed(feed_id) => {
                let title = self
                    .data
                    .as_ref()
                    .and_then(|data| data.feed_lookup.get(&feed_id))
                    .map(helpers::feed_label)
                    .unwrap_or_else(|| "feed".to_string());
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: "Remove feed".to_string(),
                    body: format!("Remove \"{title}\" and all of its cached items?"),
                    action: ConfirmAction::DeleteFeed { feed_id },
                }));
            }
            SidebarKind::Folder(folder_id) => {
                let name = self
                    .data
                    .as_ref()
                    .and_then(|data| data.folders.iter().find(|folder| folder.id == folder_id))
                    .map(|folder| folder.name.clone())
                    .unwrap_or_else(|| "folder".to_string());
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: if recursive {
                        "Delete folder recursively".to_string()
                    } else {
                        "Delete folder".to_string()
                    },
                    body: if recursive {
                        format!("Delete \"{name}\" and everything under it?")
                    } else {
                        format!("Delete \"{name}\" and reparent its children and feeds?")
                    },
                    action: ConfirmAction::DeleteFolder {
                        folder_id,
                        recursive,
                    },
                }));
            }
            SidebarKind::Board(board_id) => {
                let name = self.data.as_ref()
                    .and_then(|d| d.boards.iter().find(|b| b.id == board_id))
                    .map(|b| b.name.clone())
                    .unwrap_or_else(|| "board".into());
                self.modal = Some(Modal::Confirm(ConfirmModal {
                    title: "Delete board".into(),
                    body: format!("Delete board \"{name}\"?"),
                    action: ConfirmAction::DeleteBoard { board_id },
                }));
            }
            _ => self.set_flash("Select a feed, folder, or board to delete.".to_string(), true),
        }
    }

    pub(super) fn open_move_picker(&mut self) {
        let Some(data) = &self.data else {
            return;
        };

        match self.selected_scope() {
            SidebarKind::Feed(feed_id) => {
                let mut entries = vec![PickerEntry {
                    label: "(root)".to_string(),
                    folder_id: None,
                }];
                for folder in &data.folders {
                    entries.push(PickerEntry {
                        label: folder.name.clone(),
                        folder_id: Some(folder.id.clone()),
                    });
                }
                self.modal = Some(Modal::Picker(PickerModal {
                    title: "Move feed".to_string(),
                    entries,
                    selected: 0,
                    purpose: PickerPurpose::MoveFeed { feed_id },
                }));
            }
            SidebarKind::Folder(folder_id) => {
                let blocked = collect_descendant_ids(&data.folders, &folder_id);
                let mut entries = vec![PickerEntry {
                    label: "(root)".to_string(),
                    folder_id: None,
                }];
                for folder in &data.folders {
                    if blocked.contains(&folder.id) {
                        continue;
                    }
                    entries.push(PickerEntry {
                        label: folder.name.clone(),
                        folder_id: Some(folder.id.clone()),
                    });
                }
                self.modal = Some(Modal::Picker(PickerModal {
                    title: "Move folder".to_string(),
                    entries,
                    selected: 0,
                    purpose: PickerPurpose::MoveFolder { folder_id },
                }));
            }
            _ => self.set_flash("Select a feed or folder to move.".to_string(), true),
        }
    }

    pub(super) fn open_tag_picker(&mut self) {
        let Some(data) = &self.data else { return; };
        let mut tags: Vec<String> = data.feeds.iter()
            .flat_map(|f| f.tags.split(',').map(|t| t.trim().to_string()))
            .filter(|t| !t.is_empty())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        tags.sort_by_key(|a| a.to_lowercase());

        let mut entries = vec![PickerEntry { label: "(clear filter)".to_string(), folder_id: None }];
        for tag in tags {
            entries.push(PickerEntry { label: tag, folder_id: None });
        }
        self.modal = Some(Modal::Picker(PickerModal {
            title: "Filter by tag".to_string(),
            entries,
            selected: 0,
            purpose: PickerPurpose::FilterByTag,
        }));
    }

    pub(super) fn open_time_picker(&mut self) {
        let entries = vec![
            PickerEntry { label: "All time".to_string(), folder_id: None },
            PickerEntry { label: "Last 1h".to_string(), folder_id: None },
            PickerEntry { label: "Last 24h".to_string(), folder_id: None },
            PickerEntry { label: "Last 7d".to_string(), folder_id: None },
            PickerEntry { label: "Last 30d".to_string(), folder_id: None },
        ];
        self.modal = Some(Modal::Picker(PickerModal {
            title: "Filter by time".to_string(),
            entries,
            selected: 0,
            purpose: PickerPurpose::FilterByTime,
        }));
    }

    pub(super) fn open_mark_all_read_confirm(&mut self) {
        let scope = self.selected_scope();
        let (label, scope_str, scope_id) = match &scope {
            SidebarKind::All => ("all items".to_string(), "all".to_string(), None),
            SidebarKind::Feed(id) => {
                let name = self.data.as_ref()
                    .and_then(|d| d.feed_lookup.get(id))
                    .map(helpers::feed_label)
                    .unwrap_or_else(|| "feed".into());
                (name, "feed".to_string(), Some(id.clone()))
            }
            SidebarKind::Folder(id) => {
                let name = self.data.as_ref()
                    .and_then(|d| d.folders.iter().find(|f| f.id == *id))
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "folder".into());
                (name, "folder".to_string(), Some(id.clone()))
            }
            _ => {
                self.set_flash("Mark all read works on All, Feed, or Folder.".into(), true);
                return;
            }
        };
        self.modal = Some(Modal::Confirm(ConfirmModal {
            title: "Mark all read".into(),
            body: format!("Mark all items in \"{label}\" as read?"),
            action: ConfirmAction::MarkAllRead { scope: scope_str, scope_id },
        }));
    }

    pub(super) fn open_export_picker(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".to_string(), true);
            return;
        };
        let entries = vec![
            PickerEntry { label: "Markdown".to_string(), folder_id: None },
            PickerEntry { label: "JSON".to_string(), folder_id: None },
            PickerEntry { label: "Text".to_string(), folder_id: None },
        ];
        self.modal = Some(Modal::Picker(PickerModal {
            title: "Export item".to_string(),
            entries,
            selected: 0,
            purpose: PickerPurpose::ExportItem { item_id: item.item.id.clone() },
        }));
    }

    pub(super) fn open_create_board_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Create board".into(),
            hint: "Name for the new board".into(),
            fields: vec![InputField { label: "Name".into(), value: String::new() }],
            active: 0,
            purpose: InputPurpose::CreateBoard,
        }));
    }

    pub(super) fn open_import_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Import OPML".into(),
            hint: "Path to .opml file".into(),
            fields: vec![InputField { label: "File".into(), value: String::new() }],
            active: 0,
            purpose: InputPurpose::ImportOpml,
        }));
    }

    pub(super) fn open_create_watch_modal(&mut self) {
        self.modal = Some(Modal::Input(InputModal {
            title: "Create watch".into(),
            hint: "Saved search name and query".into(),
            fields: vec![
                InputField { label: "Name".into(), value: String::new() },
                InputField { label: "Query".into(), value: String::new() },
            ],
            active: 0,
            purpose: InputPurpose::CreateWatch,
        }));
    }

    pub(super) fn open_board_picker_for_current(&mut self) {
        let Some(item) = self.current_item() else {
            self.set_flash("Select an item first.".into(), true);
            return;
        };
        let Some(ref data) = self.data else { return; };
        let entries: Vec<PickerEntry> = data.boards.iter().map(|b| PickerEntry {
            label: b.name.clone(),
            folder_id: Some(b.id.clone()),
        }).collect();
        if entries.is_empty() {
            self.set_flash("No boards. Press B to create one.".into(), true);
            return;
        }
        self.modal = Some(Modal::Picker(PickerModal {
            title: "Add to board".into(),
            entries,
            selected: 0,
            purpose: PickerPurpose::AddToBoard { item_id: item.item.id },
        }));
    }
}
