use super::theme::Theme;
use super::*;

fn themed_block<'a>(title: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focus_border(focused, theme))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    entries: &[SidebarEntry],
    selected: usize,
    offset: usize,
    focus: Focus,
    theme: &Theme,
    collapsed: &HashSet<String>,
    hover: Option<usize>,
) -> usize {
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = i == selected;
            let is_hovered = hover == Some(i) && !is_selected;
            let gutter = if is_selected { "\u{2503} " } else if is_hovered { "\u{2502} " } else { "  " };

            let tree_prefix = if entry.depth > 0 {
                if entry.is_last_child { "\u{2570}\u{2500}\u{2500} " } else { "\u{251c}\u{2500}\u{2500} " }
            } else { "" };
            let indent = if entry.depth > 1 { "  ".repeat(entry.depth - 1) } else { String::new() };

            let folder_icon = match &entry.kind {
                SidebarKind::Folder(id) => {
                    if collapsed.contains(id) { "\u{25b8} " } else { "\u{25be} " }
                }
                _ => "",
            };

            let label_style = if is_selected {
                Style::default().fg(theme.accent)
            } else if is_hovered {
                Style::default().fg(theme.fg)
            } else {
                Style::default().fg(theme.fg_dim)
            };
            let gutter_style = if is_selected {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.fg_faint)
            };

            let mut spans = vec![
                Span::styled(gutter, gutter_style),
                Span::styled(indent, Style::default().fg(theme.fg_faint)),
                Span::styled(tree_prefix, Style::default().fg(theme.fg_faint)),
                Span::styled(folder_icon, Style::default().fg(theme.fg_dim)),
            ];

            if entry.has_error {
                spans.push(Span::styled("! ", Style::default().fg(theme.accent_secondary)));
            }

            spans.push(Span::styled(entry.label.clone(), label_style));

            if entry.unread > 0 {
                spans.push(Span::styled(format!(" ({})", entry.unread), Style::default().fg(theme.accent_secondary)));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ListState::default().with_offset(offset);
    if !entries.is_empty() {
        state.select(Some(selected));
    }

    let list = List::new(items)
        .block(themed_block("Feeds", focus == Focus::Sidebar, theme));

    frame.render_stateful_widget(list, area, &mut state);
    state.offset()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_items(
    frame: &mut Frame,
    area: Rect,
    items: &[VisibleItem],
    selected: usize,
    offset: usize,
    focus: Focus,
    theme: &Theme,
    visual_mode: bool,
    selected_items: &HashSet<String>,
    hover: Option<usize>,
) -> usize {
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_cursor = i == selected;
            let is_hovered = hover == Some(i) && !is_cursor;
            let is_read = entry.mark.as_ref().is_some_and(|m| m.read);
            let is_starred = entry.mark.as_ref().is_some_and(|m| m.starred);
            let is_unread = !is_read;
            let is_vis_selected = visual_mode && selected_items.contains(&entry.item.id);

            let gutter = if is_cursor || is_vis_selected {
                "\u{2503} "
            } else if is_hovered {
                "\u{2502} "
            } else {
                "  "
            };

            let title = entry.item.title.as_deref().unwrap_or("(untitled)");
            let title_style = if is_vis_selected {
                Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD)
            } else if is_cursor {
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
            } else if is_hovered && is_unread {
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
            } else if is_hovered {
                Style::default().fg(theme.fg)
            } else if is_unread {
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg_faint)
            };
            let gutter_style = if is_vis_selected {
                Style::default().fg(theme.accent_secondary)
            } else if is_cursor {
                Style::default().fg(theme.accent)
            } else {
                Style::default().fg(theme.fg_faint)
            };

            let source = entry.feed.as_ref().map(helpers::feed_label)
                .unwrap_or_else(|| "(unknown)".to_string());
            let published = entry.item.published_at.as_deref()
                .map(time_ago).unwrap_or_else(|| "-".to_string());

            let meta_style = if is_vis_selected {
                Style::default().fg(theme.accent_secondary)
            } else {
                Style::default().fg(theme.fg_dim)
            };
            let unread_style = Style::default().fg(theme.accent_secondary);
            let star_style = Style::default().fg(theme.accent);

            // Status markers go BEFORE the title so they never get truncated
            // when the items column is narrow. Keep the prefix a fixed 2-cell
            // width so titles align across rows.
            let unread_marker = if is_unread { "\u{25cf}" } else { " " };
            let star_marker = if is_starred { "\u{2605}" } else { " " };

            let title_line = Line::from(vec![
                Span::styled(gutter, gutter_style),
                Span::styled(unread_marker, unread_style),
                Span::raw(" "),
                Span::styled(star_marker, star_style),
                Span::raw(" "),
                Span::styled(title, title_style),
            ]);
            let meta_line = Line::from(Span::styled(
                format!("      {source} \u{2022} {published}"),
                meta_style,
            ));
            ListItem::new(vec![title_line, meta_line])
        })
        .collect();

    let mut state = ListState::default().with_offset(offset);
    if !items.is_empty() {
        state.select(Some(selected));
    }

    let list = List::new(list_items)
        .block(themed_block("Items", focus == Focus::Items, theme));

    frame.render_stateful_widget(list, area, &mut state);
    state.offset()
}

pub(super) fn draw_preview(
    frame: &mut Frame,
    area: Rect,
    item: Option<VisibleItem>,
    scroll: u16,
    focus: Focus,
    theme: &Theme,
) -> Vec<PreviewLink> {
    let mut links = Vec::new();
    let w = area.width.saturating_sub(4) as usize; // inner width with some padding

    let text = if let Some(entry) = item {
        let source = entry.feed.as_ref().map(helpers::feed_label)
            .unwrap_or_else(|| "(unknown feed)".to_string());
        let published = entry.item.published_at.as_deref()
            .map(time_ago).unwrap_or_else(|| "-".to_string());
        let url = entry.item.link.clone().unwrap_or_default();
        let body = helpers::preview_body(&entry.item, w);
        let raw_content = entry.item.content.as_deref().or(entry.item.summary.as_deref()).unwrap_or("");
        let wide_text = crate::shared::output::strip_html(raw_content, 100_000);
        let mut known_urls = helpers::extract_urls(&wide_text);
        if url.starts_with("http") { known_urls.push(url.clone()); }

        let url_style = Style::default().fg(theme.accent_secondary).add_modifier(Modifier::UNDERLINED);
        let mut lines: Vec<Line> = Vec::new();

        // Title
        let title_text = entry.item.title.unwrap_or_else(|| "(untitled)".to_string());
        for wl in helpers::wrap_text(&title_text, w) {
            lines.push(Line::from(Span::styled(wl, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))));
        }

        // Metadata: source + time, dot-separated
        let meta = format!("{source} \u{2022} {published}");
        lines.push(Line::from(Span::styled(meta, Style::default().fg(theme.fg_dim))));

        // URL
        if url.starts_with("http") {
            for wl in helpers::wrap_text(&url, w) {
                let li = lines.len();
                links.push(PreviewLink { line: li, col_start: 0, col_end: wl.len(), url: url.clone() });
                lines.push(Line::from(Span::styled(wl, url_style)));
            }
        }

        // Note
        if let Some(note) = entry.mark.and_then(|m| m.note) {
            if !note.is_empty() {
                for wl in helpers::wrap_text(&format!("Note: {note}"), w) {
                    lines.push(Line::from(Span::styled(wl, Style::default().fg(theme.fg_dim))));
                }
            }
        }

        // Horizontal rule
        let rule = "\u{2500}".repeat(w.min(120));
        lines.push(Line::from(Span::styled(rule, Style::default().fg(theme.fg_faint))));
        lines.push(Line::from(""));

        // Body
        stylize_body(&body, &mut lines, &mut links, &known_urls, url_style, theme);

        Text::from(lines)
    } else {
        Text::from(vec![
            Line::from(Span::styled("No item selected.", Style::default().fg(theme.fg_dim))),
            Line::from(Span::styled("Use the sidebar and item list to browse feeds.", Style::default().fg(theme.fg_dim))),
        ])
    };

    let block = themed_block("Preview", focus == Focus::Preview, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // Add 1-column horizontal padding
    let padded = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };
    let paragraph = Paragraph::new(text).scroll((scroll, 0));
    frame.render_widget(paragraph, padded);
    links
}

fn stylize_body(
    body: &str,
    lines: &mut Vec<Line<'static>>,
    links: &mut Vec<PreviewLink>,
    known_urls: &[String],
    url_style: Style,
    _theme: &Theme,
) {
    let mut continuation: Option<(String, usize)> = None;

    for raw_line in body.lines() {
        let li = lines.len();

        if let Some((ref full_url, shown)) = continuation {
            let remaining = &full_url[shown..];
            if !remaining.is_empty() && (remaining.starts_with(raw_line) || raw_line.starts_with(remaining)) {
                let match_len = raw_line.len().min(remaining.len());
                let url_part = &raw_line[..match_len];
                let after = &raw_line[match_len..];
                let mut spans: Vec<Span<'static>> = vec![Span::styled(url_part.to_string(), url_style)];
                links.push(PreviewLink { line: li, col_start: 0, col_end: match_len, url: full_url.clone() });
                if !after.is_empty() { spans.push(Span::raw(after.to_string())); }
                lines.push(Line::from(spans));
                let new_shown = shown + match_len;
                continuation = if new_shown < full_url.len() { Some((full_url.clone(), new_shown)) } else { None };
                continue;
            }
            continuation = None;
        }

        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut last_end = 0;

        for link in linkify::LinkFinder::new().links(raw_line) {
            if !matches!(link.kind(), linkify::LinkKind::Url) { continue; }
            let start = link.start();
            let end = link.end();
            let fragment = link.as_str();
            let full_url = known_urls.iter().find(|k| k.starts_with(fragment)).cloned()
                .unwrap_or_else(|| fragment.to_string());
            if start > last_end { spans.push(Span::raw(raw_line[last_end..start].to_string())); }
            spans.push(Span::styled(fragment.to_string(), url_style));
            links.push(PreviewLink { line: li, col_start: start, col_end: end, url: full_url.clone() });
            last_end = end;
            if fragment.len() < full_url.len() { continuation = Some((full_url, fragment.len())); }
        }

        if last_end == 0 {
            lines.push(Line::from(raw_line.to_string()));
        } else {
            if last_end < raw_line.len() { spans.push(Span::raw(raw_line[last_end..].to_string())); }
            lines.push(Line::from(spans));
        }
    }
}

pub(super) fn draw_help_overlay(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(70, 32, frame.area());
    frame.render_widget(Clear, area);
    let bold = Style::default().add_modifier(Modifier::BOLD).fg(theme.fg);
    let normal = Style::default().fg(theme.fg);
    let dim = Style::default().fg(theme.fg_dim);
    let text = Text::from(vec![
        Line::from(Span::styled("rssdude", bold)),
        Line::from(""),
        Line::from(Span::styled("Navigation", bold)),
        Line::from(Span::styled("  h / l             traverse panes", normal)),
        Line::from(Span::styled("  Tab / Shift-Tab   cycle panes", normal)),
        Line::from(Span::styled("  j / k             move selection", normal)),
        Line::from(Span::styled("  Enter             open item / focus preview", normal)),
        Line::from(Span::styled("  Ctrl-d / Ctrl-u   half-page scroll", normal)),
        Line::from(Span::styled("  g / G             go to top / bottom", normal)),
        Line::from(""),
        Line::from(Span::styled("Actions", bold)),
        Line::from(Span::styled("  r   sync all    s   sync feed    a   add feed", dim)),
        Line::from(Span::styled("  n   new folder  e   rename       M   move", dim)),
        Line::from(Span::styled("  x   delete      X   recursive    o   open link", dim)),
        Line::from(""),
        Line::from(Span::styled("Items", bold)),
        Line::from(Span::styled("  space  mark read + next   *   star/unstar", dim)),
        Line::from(Span::styled("  N      edit note          E   export", dim)),
        Line::from(Span::styled("  /      search             u   unread only", dim)),
        Line::from(Span::styled("  L      read later         t   filter tag", dim)),
        Line::from(Span::styled("  d      filter time        v   visual mode", dim)),
        Line::from(""),
        Line::from(Span::styled("Layout", bold)),
        Line::from(Span::styled("  1   toggle sidebar        3   toggle preview", dim)),
        Line::from(Span::styled("  [ / ]  resize sidebar     { / }  resize preview", dim)),
        Line::from(""),
        Line::from(Span::styled("Views", bold)),
        Line::from(Span::styled("  D   digest    T   trending    ?   this help", dim)),
        Line::from(""),
        Line::from(Span::styled("  space on sidebar collapses/expands folders", dim)),
        Line::from(""),
        Line::from(Span::styled("Press Esc or ? to close.", Style::default().fg(theme.fg_faint))),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(themed_block("Help", true, theme)),
        area,
    );
}

pub(super) fn draw_scrollable_overlay(frame: &mut Frame, title: &str, lines: &[String], scroll: u16, theme: &Theme) {
    let area = centered_rect(80, 24, frame.area());
    frame.render_widget(Clear, area);
    let text_lines: Vec<Line> = lines.iter().map(|l| Line::from(l.as_str())).collect();
    let paragraph = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(themed_block(title, true, theme));
    frame.render_widget(paragraph, area);
}

pub(super) fn draw_modal(frame: &mut Frame, modal: &Modal, theme: &Theme) {
    match modal {
        Modal::Input(input) => {
            let height = (input.fields.len() as u16) + 6;
            let area = centered_rect(70, height, frame.area());
            frame.render_widget(Clear, area);
            let mut lines = vec![Line::from(input.hint.clone()), Line::from("")];
            for (index, field) in input.fields.iter().enumerate() {
                let marker = if index == input.active { "\u{25b8}" } else { " " };
                let suffix = if index == input.active { "_" } else { "" };
                lines.push(Line::from(format!("{marker} {}: {}{suffix}", field.label, field.value)));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Enter submit  Tab move field  Esc cancel"));
            frame.render_widget(
                Paragraph::new(Text::from(lines))
                    .wrap(Wrap { trim: false })
                    .block(themed_block(&input.title, true, theme)),
                area,
            );
        }
        Modal::Picker(picker) => {
            let height = (picker.entries.len().min(12) as u16) + 4;
            let area = centered_rect(60, height, frame.area());
            frame.render_widget(Clear, area);
            let items: Vec<ListItem> = picker.entries.iter().enumerate().map(|(i, entry)| {
                let is_sel = i == picker.selected;
                let gutter = if is_sel { "\u{2503} " } else { "  " };
                let style = if is_sel {
                    Style::default().fg(theme.accent)
                } else {
                    Style::default().fg(theme.fg)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(gutter, if is_sel { Style::default().fg(theme.accent) } else { Style::default() }),
                    Span::styled(entry.label.clone(), style),
                ]))
            }).collect();
            let mut state = ListState::default();
            state.select(Some(picker.selected));
            let list = List::new(items)
                .block(themed_block(&picker.title, true, theme));
            frame.render_stateful_widget(list, area, &mut state);
        }
        Modal::Confirm(confirm) => {
            let area = centered_rect(60, 7, frame.area());
            frame.render_widget(Clear, area);
            let text = Text::from(vec![
                Line::from(confirm.body.clone()),
                Line::from(""),
                Line::from("Enter or y confirm  Esc or n cancel"),
            ]);
            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .block(themed_block(&confirm.title, true, theme)),
                area,
            );
        }
    }
}

pub(super) fn focus_border(active: bool, theme: &Theme) -> Style {
    if active {
        Style::default().fg(theme.border_focus)
    } else {
        Style::default().fg(theme.border)
    }
}

pub(super) fn centered_rect(width_pct: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(height),
            Constraint::Min(1),
        ])
        .split(area);
    let inner = vertical[1];
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(inner);
    horizontal[1]
}

pub(super) fn inner_rect(area: Rect) -> Rect {
    if area.width <= 2 || area.height <= 2 {
        return area;
    }
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width - 2,
        height: area.height - 2,
    }
}
