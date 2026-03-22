use super::*;

pub(super) fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    entries: &[SidebarEntry],
    selected: usize,
    offset: usize,
    focus: Focus,
) -> usize {
    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let prefix = "  ".repeat(entry.depth);
            let line = Line::from(vec![
                Span::raw(prefix),
                Span::raw(entry.label.clone()),
                Span::raw(" "),
                Span::styled(
                    format!("({})", entry.unread),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default().with_offset(offset);
    if !entries.is_empty() {
        state.select(Some(selected));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title("Folders and feeds")
                .borders(Borders::ALL)
                .border_style(focus_border(focus == Focus::Sidebar)),
        )
        .highlight_style(selected_style())
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state);
    state.offset()
}

pub(super) fn draw_items(
    frame: &mut Frame,
    area: Rect,
    items: &[VisibleItem],
    selected: usize,
    offset: usize,
    focus: Focus,
) -> usize {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|entry| {
            let title = entry
                .item
                .title
                .as_deref()
                .unwrap_or("(untitled)")
                .to_string();
            let source = entry
                .feed
                .as_ref()
                .map(helpers::feed_label)
                .unwrap_or_else(|| "(unknown feed)".to_string());
            let published = entry
                .item
                .published_at
                .as_deref()
                .map(time_ago)
                .unwrap_or_else(|| "-".to_string());
            let status = format!(
                "{}{}",
                if entry.mark.as_ref().is_some_and(|mark| mark.read) {
                    "r "
                } else {
                    "u "
                },
                if entry.mark.as_ref().is_some_and(|mark| mark.starred) {
                    "*"
                } else {
                    "-"
                }
            );

            let subtitle = format!("{source}  {published}  {status}");
            ListItem::new(vec![
                Line::from(Span::raw(title)),
                Line::from(Span::styled(subtitle, Style::default().fg(Color::DarkGray))),
            ])
        })
        .collect();

    let mut state = ListState::default().with_offset(offset);
    if !items.is_empty() {
        state.select(Some(selected));
    }

    let title = if items.is_empty() {
        "Items (empty)".to_string()
    } else {
        format!("Items ({})", items.len())
    };
    let list = List::new(list_items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(focus_border(focus == Focus::Items)),
        )
        .highlight_style(selected_style())
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut state);
    state.offset()
}

pub(super) fn draw_preview(
    frame: &mut Frame,
    area: Rect,
    item: Option<VisibleItem>,
    scroll: u16,
    focus: Focus,
) -> Vec<PreviewLink> {
    let mut links = Vec::new();
    let w = area.width.saturating_sub(2) as usize; // inner width (minus borders)

    let (title, text) = if let Some(entry) = item {
        let source = entry.feed.as_ref().map(helpers::feed_label)
            .unwrap_or_else(|| "(unknown feed)".to_string());
        let published = entry.item.published_at.as_deref()
            .map(|p| format!("{} ({})", time_ago(p), p))
            .unwrap_or_else(|| "-".to_string());
        let url = entry.item.link.clone().unwrap_or_else(|| "-".to_string());
        let body = helpers::preview_body(&entry.item, w);
        // Extract full URLs from unwrapped text so wrapped fragments resolve correctly
        let raw_content = entry.item.content.as_deref().or(entry.item.summary.as_deref()).unwrap_or("");
        let wide_text = helpers::strip_html(raw_content, 100_000);
        let mut known_urls = helpers::extract_urls(&wide_text);
        if url.starts_with("http") { known_urls.push(url.clone()); }

        let url_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED);
        let mut lines: Vec<Line> = Vec::new();

        // Title (bold, wrapped)
        let title_text = entry.item.title.unwrap_or_else(|| "(untitled)".to_string());
        for wl in helpers::wrap_text(&title_text, w) {
            lines.push(Line::from(Span::styled(wl, Style::default().add_modifier(Modifier::BOLD))));
        }

        // Metadata (wrapped)
        for wl in helpers::wrap_text(&format!("Source: {source}"), w) { lines.push(Line::from(wl)); }
        for wl in helpers::wrap_text(&format!("Published: {published}"), w) { lines.push(Line::from(wl)); }

        // URL header — wrap and style every segment as clickable
        if url.starts_with("http") {
            let full = format!("URL: {url}");
            for wl in helpers::wrap_text(&full, w) {
                let li = lines.len();
                if let Some(pos) = wl.find("http") {
                    let mut spans = Vec::new();
                    if pos > 0 { spans.push(Span::raw(wl[..pos].to_string())); }
                    spans.push(Span::styled(wl[pos..].to_string(), url_style));
                    links.push(PreviewLink { line: li, col_start: pos, col_end: wl.len(), url: url.clone() });
                    lines.push(Line::from(spans));
                } else {
                    // Continuation of the URL — whole line is clickable
                    links.push(PreviewLink { line: li, col_start: 0, col_end: wl.len(), url: url.clone() });
                    lines.push(Line::from(Span::styled(wl, url_style)));
                }
            }
        } else {
            lines.push(Line::from(format!("URL: {url}")));
        }

        if let Some(note) = entry.mark.and_then(|mark| mark.note) {
            if !note.is_empty() {
                for wl in helpers::wrap_text(&format!("Note: {note}"), w) { lines.push(Line::from(wl)); }
            }
        }

        lines.push(Line::from(""));

        // Body — already wrapped by html2text to width w.
        // Track URL continuations across wrapped line breaks.
        stylize_body(&body, &mut lines, &mut links, &known_urls, url_style);

        ("Preview".to_string(), Text::from(lines))
    } else {
        (
            "Preview".to_string(),
            Text::from(vec![
                Line::from("No item selected."),
                Line::from("Use the sidebar and item list to browse feeds."),
            ]),
        )
    };

    let paragraph = Paragraph::new(text)
        .scroll((scroll, 0))
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(focus_border(focus == Focus::Preview)),
        );
    frame.render_widget(paragraph, area);
    links
}

/// Process body text lines, styling URLs and tracking continuations across wrapped lines.
fn stylize_body(
    body: &str,
    lines: &mut Vec<Line<'static>>,
    links: &mut Vec<PreviewLink>,
    known_urls: &[String],
    url_style: Style,
) {
    // (full_url, chars_of_url_already_rendered)
    let mut continuation: Option<(String, usize)> = None;

    for raw_line in body.lines() {
        let li = lines.len();

        // Check if this line continues a URL from the previous line
        if let Some((ref full_url, shown)) = continuation {
            let remaining = &full_url[shown..];
            if !remaining.is_empty() && (remaining.starts_with(raw_line) || raw_line.starts_with(remaining)) {
                let match_len = raw_line.len().min(remaining.len());
                let url_part = &raw_line[..match_len];
                let after = &raw_line[match_len..];

                let mut spans: Vec<Span<'static>> = Vec::new();
                spans.push(Span::styled(url_part.to_string(), url_style));
                links.push(PreviewLink { line: li, col_start: 0, col_end: match_len, url: full_url.clone() });
                if !after.is_empty() {
                    spans.push(Span::raw(after.to_string()));
                }
                lines.push(Line::from(spans));

                let new_shown = shown + match_len;
                continuation = if new_shown < full_url.len() {
                    Some((full_url.clone(), new_shown))
                } else {
                    None
                };
                continue;
            }
            continuation = None;
        }

        // Normal line: find URLs starting with http
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut last_end = 0;

        for (start, _) in raw_line.match_indices("http") {
            let rest = &raw_line[start..];
            let url_len = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '"' || c == '\'' || c == ')' || c == ']')
                .unwrap_or(rest.len());
            let fragment = &raw_line[start..start + url_len];

            if !fragment.starts_with("http://") && !fragment.starts_with("https://") {
                continue;
            }

            // Resolve to full URL
            let full_url = known_urls.iter()
                .find(|known| known.starts_with(fragment))
                .cloned()
                .unwrap_or_else(|| fragment.to_string());

            if start > last_end {
                spans.push(Span::raw(raw_line[last_end..start].to_string()));
            }
            spans.push(Span::styled(fragment.to_string(), url_style));
            links.push(PreviewLink { line: li, col_start: start, col_end: start + url_len, url: full_url.clone() });
            last_end = start + url_len;

            // If this URL was truncated by wrapping, track continuation
            if fragment.len() < full_url.len() {
                continuation = Some((full_url, fragment.len()));
            }
        }

        if last_end == 0 {
            lines.push(Line::from(raw_line.to_string()));
        } else {
            if last_end < raw_line.len() {
                spans.push(Span::raw(raw_line[last_end..].to_string()));
            }
            lines.push(Line::from(spans));
        }
    }
}

pub(super) fn draw_help_overlay(frame: &mut Frame) {
    let area = centered_rect(70, 24, frame.area());
    frame.render_widget(Clear, area);
    let text = Text::from(vec![
        Line::from(Span::styled(
            "rssdude TUI",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Navigation"),
        Line::from("  Tab / Shift-Tab  cycle panes"),
        Line::from("  j / k             move selection"),
        Line::from("  Enter             open item / focus preview"),
        Line::from("  PageUp/PageDown   move faster"),
        Line::from("  Ctrl-d / Ctrl-u   half-page scroll"),
        Line::from(""),
        Line::from("Actions"),
        Line::from("  r                 sync all feeds"),
        Line::from("  s                 sync selected feed / current item feed"),
        Line::from("  a                 add feed into current folder context"),
        Line::from("  n                 create folder"),
        Line::from("  e                 rename selected folder"),
        Line::from("  M                 move selected feed or folder"),
        Line::from("  x                 delete selected feed or folder"),
        Line::from("  X                 recursive folder delete"),
        Line::from(""),
        Line::from("Item tools"),
        Line::from("  space             toggle read"),
        Line::from("  *                 toggle star / unstar"),
        Line::from("  N                 edit note"),
        Line::from("  E                 export item (md/json/txt)"),
        Line::from("  /                 search (comma-separated keywords)"),
        Line::from("  u                 toggle unread-only"),
        Line::from("  t                 filter by tag"),
        Line::from("  d                 filter by time range"),
        Line::from("  o                 open item link"),
        Line::from(""),
        Line::from("Views"),
        Line::from("  D                 digest (recent items by feed)"),
        Line::from("  T                 trending keywords"),
        Line::from(""),
        Line::from("Press Esc or ? to close help."),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::default().title("Help").borders(Borders::ALL)),
        area,
    );
}

pub(super) fn draw_scrollable_overlay(frame: &mut Frame, title: &str, lines: &[String], scroll: u16) {
    let area = centered_rect(80, 24, frame.area());
    frame.render_widget(Clear, area);
    let text_lines: Vec<Line> = lines.iter().map(|l| Line::from(l.as_str())).collect();
    let paragraph = Paragraph::new(Text::from(text_lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

pub(super) fn draw_modal(frame: &mut Frame, modal: &Modal) {
    match modal {
        Modal::Input(input) => {
            let height = (input.fields.len() as u16) + 6;
            let area = centered_rect(70, height, frame.area());
            frame.render_widget(Clear, area);

            let mut lines = vec![Line::from(input.hint.clone()), Line::from("")];
            for (index, field) in input.fields.iter().enumerate() {
                let marker = if index == input.active { ">" } else { " " };
                let suffix = if index == input.active { "_" } else { "" };
                lines.push(Line::from(format!(
                    "{marker} {}: {}{suffix}",
                    field.label, field.value
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Enter submit  Tab move field  Esc cancel"));

            frame.render_widget(
                Paragraph::new(Text::from(lines))
                    .wrap(Wrap { trim: false })
                    .block(
                        Block::default()
                            .title(input.title.clone())
                            .borders(Borders::ALL),
                    ),
                area,
            );
        }
        Modal::Picker(picker) => {
            let height = (picker.entries.len().min(12) as u16) + 4;
            let area = centered_rect(60, height, frame.area());
            frame.render_widget(Clear, area);

            let items: Vec<ListItem> = picker
                .entries
                .iter()
                .map(|entry| ListItem::new(entry.label.clone()))
                .collect();
            let mut state = ListState::default();
            state.select(Some(picker.selected));
            let list = List::new(items)
                .block(
                    Block::default()
                        .title(picker.title.clone())
                        .borders(Borders::ALL),
                )
                .highlight_style(selected_style())
                .highlight_symbol("> ");
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
                Paragraph::new(text).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .title(confirm.title.clone())
                        .borders(Borders::ALL),
                ),
                area,
            );
        }
    }
}

pub(super) fn selected_style() -> Style {
    Style::default()
        .bg(Color::Blue)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub(super) fn focus_border(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
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
