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
            Focus::Items | Focus::Preview => {
                self.item_index = self.item_index.saturating_sub(step);
                self.preview_scroll = 0;
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
            Focus::Items | Focus::Preview => {
                let items = self.visible_items();
                let max = items.len().saturating_sub(1);
                self.item_index = (self.item_index + step).min(max);
                self.preview_scroll = 0;
            }
        }
    }

    pub(super) fn scroll_preview_up(&mut self, step: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(step);
    }

    pub(super) fn scroll_preview_down(&mut self, step: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(step);
    }

    pub(super) fn advance_to_next_unread(&mut self) {
        let items = self.visible_items();
        let start = self.item_index + 1;
        if let Some((offset, _)) = items[start..].iter().enumerate()
            .find(|(_, item)| !item.mark.as_ref().is_some_and(|m| m.read))
        {
            self.item_index = start + offset;
        } else {
            self.item_index = (self.item_index + 1).min(items.len().saturating_sub(1));
        }
        self.preview_scroll = 0;
    }

    pub(super) fn move_to_start(&mut self) {
        match self.focus {
            Focus::Sidebar => {
                self.sidebar_index = 0;
                self.item_index = 0;
                self.preview_scroll = 0;
            }
            Focus::Items | Focus::Preview => {
                self.item_index = 0;
                self.preview_scroll = 0;
            }
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
            Focus::Items | Focus::Preview => {
                let len = self.visible_items().len();
                self.item_index = len.saturating_sub(1);
                self.preview_scroll = 0;
            }
        }
    }

    pub(super) fn cycle_focus_forward(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Items,
            Focus::Items => if self.show_preview { Focus::Preview } else if self.show_sidebar { Focus::Sidebar } else { Focus::Items },
            Focus::Preview => if self.show_sidebar { Focus::Sidebar } else { Focus::Items },
        };
    }

    pub(super) fn cycle_focus_back(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => if self.show_preview { Focus::Preview } else { Focus::Items },
            Focus::Items => if self.show_sidebar { Focus::Sidebar } else if self.show_preview { Focus::Preview } else { Focus::Items },
            Focus::Preview => Focus::Items,
        };
    }

    /// Handle scroll keys for a scrollable overlay. Returns true if the key was consumed.
    fn handle_scrollable_overlay_key(key: &KeyEvent, scroll: &mut u16, half: u16) -> bool {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
            KeyCode::PageDown => *scroll = scroll.saturating_add(10),
            KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *scroll = scroll.saturating_add(half);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                *scroll = scroll.saturating_sub(half);
            }
            _ => return false,
        }
        true
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
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('D')) {
                self.digest_content = None;
            } else {
                Self::handle_scrollable_overlay_key(&key, &mut self.digest_scroll, half);
            }
            return;
        }

        // Handle trending overlay
        if self.trending_content.is_some() {
            let half = self.half_page() as u16;
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('T')) {
                self.trending_content = None;
            } else if key.code == KeyCode::Enter {
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
            } else {
                Self::handle_scrollable_overlay_key(&key, &mut self.trending_scroll, half);
            }
            return;
        }

        self.hover_sidebar = None;
        self.hover_item = None;
        self.hover_link = None;

        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        self.handle_main_keys(key);
    }

    fn handle_main_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc if self.visual_mode => {
                self.visual_mode = false;
                self.selected_items.clear();
            }
            KeyCode::Char('q') => self.should_quit = true,
            // Panel collapse: 1=sidebar, 3=preview
            KeyCode::Char('1') => {
                self.show_sidebar = !self.show_sidebar;
                if !self.show_sidebar && self.focus == Focus::Sidebar { self.focus = Focus::Items; }
            }
            KeyCode::Char('3') => {
                self.show_preview = !self.show_preview;
                if !self.show_preview && self.focus == Focus::Preview { self.focus = Focus::Items; }
            }
            // Panel resize: [/] shrink/grow sidebar, {/} shrink/grow preview
            KeyCode::Char('[') if self.sidebar_pct > 10 => self.sidebar_pct -= 5,
            KeyCode::Char(']') if self.sidebar_pct < 50 => self.sidebar_pct += 5,
            KeyCode::Char('{') if self.preview_pct > 15 => self.preview_pct -= 5,
            KeyCode::Char('}') if self.preview_pct < 70 => self.preview_pct += 5,
            KeyCode::Tab => self.cycle_focus_forward(),
            KeyCode::BackTab => self.cycle_focus_back(),
            KeyCode::Left | KeyCode::Char('h') => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Sidebar,
                    Focus::Items => if self.show_sidebar { Focus::Sidebar } else { Focus::Items },
                    Focus::Preview => Focus::Items,
                };
            }
            KeyCode::Right => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Items,
                    Focus::Items => if self.show_preview { Focus::Preview } else { Focus::Items },
                    Focus::Preview => Focus::Preview,
                };
            }
            KeyCode::Char('l') => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Items,
                    Focus::Items => if self.show_preview { Focus::Preview } else { Focus::Items },
                    Focus::Preview => Focus::Preview,
                };
            }
            KeyCode::Char('L') => self.toggle_current_read_later(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(1),
            KeyCode::PageUp => {
                if self.focus == Focus::Preview {
                    self.scroll_preview_up(self.half_page() as u16 * 2);
                } else {
                    self.move_selection_up(10);
                }
            }
            KeyCode::PageDown => {
                if self.focus == Focus::Preview {
                    self.scroll_preview_down(self.half_page() as u16 * 2);
                } else {
                    self.move_selection_down(10);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Focus::Preview {
                    self.scroll_preview_down(self.half_page() as u16);
                } else {
                    self.move_selection_down(self.half_page());
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Focus::Preview {
                    self.scroll_preview_up(self.half_page() as u16);
                } else {
                    self.move_selection_up(self.half_page());
                }
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
            KeyCode::Char(' ') => {
                if self.visual_mode {
                    // Toggle selection on current item, don't advance
                    if let Some(item) = self.current_item() {
                        if !self.selected_items.remove(&item.item.id) {
                            self.selected_items.insert(item.item.id.clone());
                        }
                    }
                } else if self.focus == Focus::Sidebar {
                    if let Some(SidebarKind::Folder(id)) = self.sidebar_entries().get(self.sidebar_index).map(|e| e.kind.clone()) {
                        if !self.collapsed_folders.remove(&id) {
                            self.collapsed_folders.insert(id);
                        }
                    }
                } else if let Some(item) = self.current_item() {
                    let was_read = item.mark.as_ref().is_some_and(|m| m.read);
                    let backend = Arc::clone(&self.backend);
                    self.spawn_action(true, async move {
                        backend.toggle_mark(item.item.id, Some(!was_read), None).await
                    });
                    self.advance_to_next_unread();
                }
            }
            KeyCode::Char('v') => {
                self.visual_mode = !self.visual_mode;
                if !self.visual_mode { self.selected_items.clear(); }
            }
            KeyCode::Char('V') => {
                self.visual_mode = true;
                let items = self.visible_items();
                self.selected_items = items.iter().map(|i| i.item.id.clone()).collect();
            }
            KeyCode::Char('*') => {
                if self.visual_mode && !self.selected_items.is_empty() {
                    self.bulk_star();
                } else {
                    self.toggle_current_star();
                }
            }
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
            KeyCode::Char('R') => {
                if self.visual_mode && !self.selected_items.is_empty() {
                    self.bulk_mark_read();
                } else {
                    self.open_mark_all_read_confirm();
                }
            }
            KeyCode::Char('S') => {
                self.sort_order = if self.sort_order == "newest" { "oldest".into() } else { "newest".into() };
                self.set_flash(format!("Sort: {}", self.sort_order), false);
            }
            KeyCode::Char('=') => {
                self.dedup = !self.dedup;
                self.set_flash(format!("Dedup: {}", if self.dedup { "on" } else { "off" }), false);
            }
            KeyCode::Char('F') if self.focus == Focus::Preview => self.fetch_full_article(),
            KeyCode::Char('I') => self.open_import_modal(),
            KeyCode::Char('b') => self.open_board_picker_for_current(),
            KeyCode::Char('B') => self.open_create_board_modal(),
            KeyCode::Char('W') => self.open_create_watch_modal(),
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
        let body_width = layout.sidebar.width + layout.items.width + layout.preview.width;
        let body_x = layout.items.x.min(layout.sidebar.x.min(layout.preview.x));

        // Drag release
        if matches!(mouse.kind, MouseEventKind::Up(_)) {
            self.dragging = None;
            return;
        }

        // Active drag — update percentages
        if matches!(mouse.kind, MouseEventKind::Drag(_)) {
            if let Some(target) = self.dragging {
                let rel = x.saturating_sub(body_x);
                let pct = ((rel as u32 * 100) / body_width.max(1) as u32) as u16;
                match target {
                    DragTarget::SidebarBorder => self.sidebar_pct = pct.clamp(10, 50),
                    DragTarget::PreviewBorder => {
                        self.preview_pct = (100u16.saturating_sub(pct)).clamp(15, 70);
                    }
                }
                return;
            }
        }

        // Check if clicking on a panel border to start dragging
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let sidebar_edge = layout.sidebar.x + layout.sidebar.width;
            let preview_edge = layout.preview.x;
            if self.show_sidebar && (x == sidebar_edge || x + 1 == sidebar_edge) {
                self.dragging = Some(DragTarget::SidebarBorder);
                return;
            }
            if self.show_preview && (x == preview_edge || x == preview_edge.saturating_sub(1)) {
                self.dragging = Some(DragTarget::PreviewBorder);
                return;
            }
        }

        // Hover tracking — update on every mouse move, no focus change
        if matches!(mouse.kind, MouseEventKind::Moved) {
            self.hover_sidebar = None;
            self.hover_item = None;
            self.hover_link = None;
            if self.show_sidebar && helpers::rect_contains(layout.sidebar, x, y) {
                if let Some(line) = helpers::relative_line(layout.sidebar_inner, y) {
                    let idx = self.sidebar_offset + line as usize;
                    if idx < self.sidebar_entries().len() { self.hover_sidebar = Some(idx); }
                }
            } else if helpers::rect_contains(layout.items, x, y) {
                if let Some(line) = helpers::relative_line(layout.items_inner, y) {
                    let idx = self.items_offset + (line as usize / 2);
                    let items = self.visible_items();
                    if idx < items.len() { self.hover_item = Some(idx); }
                }
            } else if self.show_preview && helpers::rect_contains(layout.preview, x, y) {
                self.hover_link = self.url_at_position(layout, x, y);
            }
            return;
        }

        // Click/scroll — only these change focus
        if self.show_sidebar && helpers::rect_contains(layout.sidebar, x, y) {
            self.handle_sidebar_mouse(layout, &mouse, y);
        } else if helpers::rect_contains(layout.items, x, y) {
            self.handle_items_mouse(layout, &mouse, y);
        } else if self.show_preview && helpers::rect_contains(layout.preview, x, y) {
            self.handle_preview_mouse(layout, &mouse, x, y);
        }
    }

    fn handle_sidebar_mouse(&mut self, layout: UiLayout, mouse: &MouseEvent, y: u16) {
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
    }

    fn handle_items_mouse(&mut self, layout: UiLayout, mouse: &MouseEvent, y: u16) {
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
                            self.last_item_click = Some(LastItemClick { item_index: index, at: now });
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => self.move_selection_up(3),
            MouseEventKind::ScrollDown => self.move_selection_down(3),
            _ => {}
        }
    }

    fn handle_preview_mouse(&mut self, layout: UiLayout, mouse: &MouseEvent, x: u16, y: u16) {
        self.focus = Focus::Preview;
        self.last_item_click = None;
        match mouse.kind {
            MouseEventKind::ScrollUp => self.preview_scroll = self.preview_scroll.saturating_sub(3),
            MouseEventKind::ScrollDown => self.preview_scroll = self.preview_scroll.saturating_add(3),
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
