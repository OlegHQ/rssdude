use super::*;

impl App {
    pub(super) fn move_selection_up(&mut self, step: usize) {
        match self.focus {
            Focus::Sidebar => {
                let old = self.sidebar_index;
                self.sidebar_index = self.sidebar_index.saturating_sub(step);
                if self.sidebar_index != old {
                    self.item_index = 0;
                    self.preview_scroll = 0;
                }
            }
            Focus::Items => {
                self.item_index = self.item_index.saturating_sub(step);
                self.preview_scroll = 0;
            }
            Focus::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_sub(step as u16);
            }
        }
    }

    pub(super) fn move_selection_down(&mut self, step: usize) {
        match self.focus {
            Focus::Sidebar => {
                let entries = self.sidebar_entries();
                let max = entries.len().saturating_sub(1);
                let old = self.sidebar_index;
                self.sidebar_index = (self.sidebar_index + step).min(max);
                if self.sidebar_index != old {
                    self.item_index = 0;
                    self.preview_scroll = 0;
                }
            }
            Focus::Items => {
                let items = self.visible_items();
                let max = items.len().saturating_sub(1);
                self.item_index = (self.item_index + step).min(max);
                self.preview_scroll = 0;
            }
            Focus::Preview => {
                self.preview_scroll = self.preview_scroll.saturating_add(step as u16);
            }
        }
    }

    pub(super) fn move_to_start(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                self.sidebar_index = 0;
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            Focus::Items => {
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            Focus::Preview => self.preview_scroll = 0,
        }
    }

    pub(super) fn move_to_end(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                let len = self.sidebar_entries().len();
                self.sidebar_index = len.saturating_sub(1);
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            Focus::Items => {
                let len = self.visible_items().len();
                self.item_index = len.saturating_sub(1);
                self.preview_scroll = 0;
            }
            Focus::Preview => self.preview_scroll = u16::MAX / 4,
        }
    }

    pub(super) fn cycle_focus_forward(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Items,
            Focus::Items => Focus::Preview,
            Focus::Preview => Focus::Sidebar,
        };
    }

    pub(super) fn cycle_focus_back(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Preview,
            Focus::Items => Focus::Sidebar,
            Focus::Preview => Focus::Items,
        };
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if self.show_help {
            self.show_help = false;
            return;
        }

        // Handle digest overlay
        if self.digest_content.is_some() {
            let half = self.half_page() as u16;
            match key.code {
                KeyCode::Esc | KeyCode::Char('D') => self.digest_content = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    self.digest_scroll = self.digest_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.digest_scroll = self.digest_scroll.saturating_sub(1);
                }
                KeyCode::PageDown => self.digest_scroll = self.digest_scroll.saturating_add(10),
                KeyCode::PageUp => self.digest_scroll = self.digest_scroll.saturating_sub(10),
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.digest_scroll = self.digest_scroll.saturating_add(half);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.digest_scroll = self.digest_scroll.saturating_sub(half);
                }
                _ => {}
            }
            return;
        }

        // Handle trending overlay
        if self.trending_content.is_some() {
            let half = self.half_page() as u16;
            match key.code {
                KeyCode::Esc | KeyCode::Char('T') => self.trending_content = None,
                KeyCode::Down | KeyCode::Char('j') => {
                    self.trending_scroll = self.trending_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.trending_scroll = self.trending_scroll.saturating_sub(1);
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.trending_scroll = self.trending_scroll.saturating_add(half);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.trending_scroll = self.trending_scroll.saturating_sub(half);
                }
                KeyCode::Enter => {
                    if let Some(ref lines) = self.trending_content {
                        let idx = self.trending_scroll as usize + 2;
                        if let Some(line) = lines.get(idx) {
                            if let Some(topic) = line.split_whitespace().next() {
                                self.search_query = topic.to_string();
                                self.item_index = 0;
                                self.preview_scroll = 0;
                            }
                        }
                    }
                    self.trending_content = None;
                }
                _ => {}
            }
            return;
        }

        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.cycle_focus_forward(),
            KeyCode::BackTab => self.cycle_focus_back(),
            KeyCode::Left | KeyCode::Char('h') => self.cycle_focus_back(),
            KeyCode::Right | KeyCode::Char('l') => self.cycle_focus_forward(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(1),
            KeyCode::PageUp => self.move_selection_up(10),
            KeyCode::PageDown => self.move_selection_down(10),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection_down(self.half_page())
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection_up(self.half_page())
            }
            KeyCode::Home | KeyCode::Char('g') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_to_start()
            }
            KeyCode::End | KeyCode::Char('G') => self.move_to_end(),
            KeyCode::Enter => match self.focus {
                Focus::Sidebar => {
                    self.focus = Focus::Items;
                    self.item_index = 0;
                    self.preview_scroll = 0;
                }
                Focus::Items => self.open_current_item(),
                Focus::Preview => self.focus = Focus::Items,
            },
            KeyCode::Char(' ') => self.toggle_current_read(),
            KeyCode::Char('*') => self.toggle_current_star(),
            KeyCode::Char('N') => self.open_note_modal(),
            KeyCode::Char('o') => self.open_current_link(),
            KeyCode::Char('u') => {
                self.unread_only = !self.unread_only;
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            KeyCode::Char('/') => self.open_search_modal(),
            KeyCode::Char('r') => self.start_sync(None),
            KeyCode::Char('s') => self.start_current_sync(),
            KeyCode::Char('a') => self.open_add_feed_modal(),
            KeyCode::Char('n') => self.open_new_folder_modal(),
            KeyCode::Char('e') => self.open_rename_folder_modal(),
            KeyCode::Char('x') => self.open_delete_modal(false),
            KeyCode::Char('X') => self.open_delete_modal(true),
            KeyCode::Char('M') => self.open_move_picker(),
            KeyCode::Char('t') => self.open_tag_picker(),
            KeyCode::Char('d') => self.open_time_picker(),
            KeyCode::Char('E') => self.open_export_picker(),
            KeyCode::Char('D') => self.open_digest_overlay(),
            KeyCode::Char('T') => self.open_trending_overlay(),
            KeyCode::Char('?') => self.show_help = true,
            _ => {}
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.show_help || self.modal.is_some() {
            return;
        }

        let Some(layout) = self.layout else {
            return;
        };

        let x = mouse.column;
        let y = mouse.row;

        if helpers::rect_contains(layout.sidebar, x, y) {
            self.focus = Focus::Sidebar;
            self.last_item_click = None;
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(line) = helpers::relative_line(layout.sidebar_inner, y) {
                        let entries = self.sidebar_entries();
                        let index = self.sidebar_offset + line as usize;
                        if index < entries.len() {
                            self.sidebar_index = index;
                            self.item_index = 0;
                            self.preview_scroll = 0;
                        }
                    }
                }
                MouseEventKind::ScrollUp => self.move_selection_up(3),
                MouseEventKind::ScrollDown => self.move_selection_down(3),
                _ => {}
            }
            return;
        }

        if helpers::rect_contains(layout.items, x, y) {
            self.focus = Focus::Items;
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(line) = helpers::relative_line(layout.items_inner, y) {
                        let items = self.visible_items();
                        let index = self.items_offset + (line as usize / 2);
                        if index < items.len() {
                            self.item_index = index;
                            self.preview_scroll = 0;
                            let now = Instant::now();
                            if helpers::is_double_click(self.last_item_click.as_ref(), index, now) {
                                self.last_item_click = None;
                                self.open_current_link();
                            } else {
                                self.last_item_click = Some(LastItemClick {
                                    item_index: index,
                                    at: now,
                                });
                            }
                        }
                    }
                }
                MouseEventKind::ScrollUp => self.move_selection_up(3),
                MouseEventKind::ScrollDown => self.move_selection_down(3),
                _ => {}
            }
            return;
        }

        if helpers::rect_contains(layout.preview, x, y) {
            self.focus = Focus::Preview;
            self.last_item_click = None;
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(3);
                }
                MouseEventKind::ScrollDown => {
                    self.preview_scroll = self.preview_scroll.saturating_add(3);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(url) = self.url_at_position(layout, x, y) {
                        match Command::new("open").arg(&url).spawn() {
                            Ok(_) => self.set_flash(format!("Opened {url}"), false),
                            Err(e) => self.set_flash(format!("Failed: {e}"), true),
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Find a URL at the given screen position in the preview pane.
    fn url_at_position(&self, layout: UiLayout, x: u16, y: u16) -> Option<String> {
        let inner = layout.preview_inner;
        if y < inner.y || y >= inner.y + inner.height { return None; }
        let visible_line = (y - inner.y) as usize;
        let text_line = visible_line + self.preview_scroll as usize;
        let col = (x.saturating_sub(inner.x)) as usize;
        self.preview_links.iter()
            .find(|link| link.line == text_line && col >= link.col_start && col < link.col_end)
            .map(|link| link.url.clone())
    }

    pub(super) fn handle_modal_key(&mut self, key: KeyEvent) {
        if let Some(modal) = &mut self.modal {
            match modal {
                Modal::Input(input) => match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Enter => {
                        let modal = self.modal.take();
                        if let Some(Modal::Input(input)) = modal {
                            self.submit_input_modal(input);
                        }
                    }
                    KeyCode::Tab | KeyCode::Down => {
                        input.active = (input.active + 1) % input.fields.len();
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        if input.active == 0 {
                            input.active = input.fields.len().saturating_sub(1);
                        } else {
                            input.active -= 1;
                        }
                    }
                    KeyCode::Backspace => {
                        input.fields[input.active].value.pop();
                    }
                    KeyCode::Char(ch)
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        input.fields[input.active].value.push(ch);
                    }
                    _ => {}
                },
                Modal::Picker(picker) => match key.code {
                    KeyCode::Esc => self.modal = None,
                    KeyCode::Enter => {
                        let modal = self.modal.take();
                        if let Some(Modal::Picker(picker)) = modal {
                            self.submit_picker_modal(picker);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        picker.selected = picker.selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let max = picker.entries.len().saturating_sub(1);
                        picker.selected = (picker.selected + 1).min(max);
                    }
                    _ => {}
                },
                Modal::Confirm(_confirm) => match key.code {
                    KeyCode::Esc | KeyCode::Char('n') => self.modal = None,
                    KeyCode::Enter | KeyCode::Char('y') => {
                        let modal = self.modal.take();
                        if let Some(Modal::Confirm(confirm)) = modal {
                            self.submit_confirm_modal(confirm);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
}
