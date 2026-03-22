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
            _ => self.set_flash("Select a feed or folder to delete.".to_string(), true),
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
}
