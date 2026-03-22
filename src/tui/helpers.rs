use super::*;

pub(super) fn folder_unread(
    folder_id: &str,
    feeds_by_folder: &HashMap<Option<String>, Vec<&Feed>>,
    folders_by_parent: &HashMap<Option<String>, Vec<&Folder>>,
    feed_stats: &HashMap<String, FeedStats>,
) -> usize {
    let direct: usize = feeds_by_folder
        .get(&Some(folder_id.to_string()))
        .map(|feeds| {
            feeds
                .iter()
                .map(|feed| {
                    feed_stats
                        .get(&feed.id)
                        .map(|stats| stats.unread)
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0);
    let children: usize = folders_by_parent
        .get(&Some(folder_id.to_string()))
        .map(|folders| {
            folders
                .iter()
                .map(|folder| {
                    folder_unread(
                        &folder.id,
                        feeds_by_folder,
                        folders_by_parent,
                        feed_stats,
                    )
                })
                .sum()
        })
        .unwrap_or(0);
    direct + children
}

pub(super) fn push_folder_entries(
    folder: &Folder,
    depth: usize,
    entries: &mut Vec<SidebarEntry>,
    feeds_by_folder: &HashMap<Option<String>, Vec<&Feed>>,
    folders_by_parent: &HashMap<Option<String>, Vec<&Folder>>,
    feed_stats: &HashMap<String, FeedStats>,
    collapsed: &HashSet<String>,
) {
    entries.push(SidebarEntry {
        kind: SidebarKind::Folder(folder.id.clone()),
        label: folder.name.clone(),
        unread: folder_unread(&folder.id, feeds_by_folder, folders_by_parent, feed_stats),
        depth,
        has_error: false,
        is_last_child: false,
    });

    if collapsed.contains(&folder.id) {
        return;
    }

    let child_feeds = feeds_by_folder.get(&Some(folder.id.clone()));
    let child_folders = folders_by_parent.get(&Some(folder.id.clone()));
    let total_children = child_feeds.map(|f| f.len()).unwrap_or(0)
        + child_folders.map(|f| f.len()).unwrap_or(0);
    let mut child_idx = 0;

    if let Some(feeds) = child_feeds {
        for feed in feeds {
            child_idx += 1;
            entries.push(SidebarEntry {
                kind: SidebarKind::Feed(feed.id.clone()),
                label: feed_label(feed),
                unread: feed_stats
                    .get(&feed.id)
                    .map(|stats| stats.unread)
                    .unwrap_or(0),
                depth: depth + 1,
                has_error: feed.error_count > 0,
                is_last_child: child_idx == total_children,
            });
        }
    }

    if let Some(children) = child_folders {
        for child in children {
            child_idx += 1;
            // Mark the folder entry as last child if it's the last
            push_folder_entries(
                child,
                depth + 1,
                entries,
                feeds_by_folder,
                folders_by_parent,
                feed_stats,
                collapsed,
            );
            // Set is_last_child on the folder entry we just pushed
            if child_idx == total_children {
                if let Some(entry) = entries.iter_mut().rev()
                    .find(|e| matches!(&e.kind, SidebarKind::Folder(id) if *id == child.id))
                {
                    entry.is_last_child = true;
                }
            }
        }
    }
}

pub(super) fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}

pub(super) fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

pub(super) fn relative_line(rect: Rect, y: u16) -> Option<u16> {
    if y < rect.y || y >= rect.y + rect.height {
        return None;
    }
    Some(y.saturating_sub(rect.y))
}

pub(super) fn is_double_click(last_click: Option<&LastItemClick>, item_index: usize, now: Instant) -> bool {
    last_click.is_some_and(|click| {
        click.item_index == item_index && now.duration_since(click.at) <= DOUBLE_CLICK_TTL
    })
}

pub(super) fn sync_target_feed_id(scope: &SidebarKind, current_item_feed_id: Option<&str>) -> Option<String> {
    match scope {
        SidebarKind::Feed(id) => Some(id.clone()),
        _ => current_item_feed_id.map(str::to_owned),
    }
}

pub(super) fn feed_label(feed: &Feed) -> String {
    feed.title.clone().unwrap_or_else(|| feed.url.clone())
}

pub(super) fn feed_sort_key(feed: &Feed) -> String {
    feed_label(feed).to_lowercase()
}

pub(super) fn item_time_key(item: &Item) -> &str {
    item.published_at
        .as_deref()
        .unwrap_or(item.fetched_at.as_str())
}

pub(super) fn item_matches_scope(
    item: &Item,
    scope: &SidebarKind,
    data: &BrowserData,
    descendants: Option<&HashSet<String>>,
) -> bool {
    let mark = data.marks.get(&item.id);
    let feed = data.feed_lookup.get(&item.feed_id);

    match scope {
        SidebarKind::All => true,
        SidebarKind::Starred => mark.is_some_and(|mark| mark.starred),
        SidebarKind::Uncategorized => feed.and_then(|feed| feed.folder_id.as_ref()).is_none(),
        SidebarKind::Feed(feed_id) => item.feed_id == *feed_id,
        SidebarKind::Folder(_) => feed
            .and_then(|feed| feed.folder_id.as_ref())
            .is_some_and(|folder_id| descendants.is_some_and(|set| set.contains(folder_id))),
        SidebarKind::ReadLater => mark.is_some_and(|m| m.read_later),
        SidebarKind::RecentlyRead => mark.is_some_and(|m| m.read),
        SidebarKind::Board(_) | SidebarKind::Watch(_) => true, // handled in visible_items
    }
}

pub(super) fn item_matches_query(item: &Item, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    // Support comma-separated keywords: match if ANY keyword found
    let keywords: Vec<&str> = query.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if keywords.is_empty() {
        return true;
    }

    let title = item.title.as_ref().map(|t| t.to_lowercase());
    let content = item.content.as_ref().map(|c| c.to_lowercase());
    let summary = item.summary.as_ref().map(|s| s.to_lowercase());

    keywords.iter().any(|kw| {
        title.as_ref().is_some_and(|t| t.contains(kw))
            || content.as_ref().is_some_and(|c| c.contains(kw))
            || summary.as_ref().is_some_and(|s| s.contains(kw))
    })
}

pub(super) fn preview_body(item: &Item, width: usize) -> String {
    let raw = item
        .content
        .as_deref()
        .or(item.summary.as_deref())
        .unwrap_or("(no content)");
    strip_html(raw, width)
}

pub(super) fn strip_html(input: &str, width: usize) -> String {
    html2text::from_read(input.as_bytes(), width.max(20))
        .trim()
        .to_string()
}

/// Extract all complete URLs from text (use on unwrapped text to get full URLs).
pub(super) fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for (start, _) in text.match_indices("http") {
        let rest = &text[start..];
        let url_len = rest.find(|c: char| c.is_whitespace() || c == '>' || c == '"' || c == '\'' || c == ')' || c == ']')
            .unwrap_or(rest.len());
        let url = &text[start..start + url_len];
        if url.starts_with("http://") || url.starts_with("https://") {
            urls.push(url.to_string());
        }
    }
    urls
}

/// Word-wrap a string to fit within `width` columns.
pub(super) fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.lines() {
        if line.len() <= width {
            out.push(line.to_string());
        } else {
            let mut remaining = line;
            while remaining.len() > width {
                let break_at = remaining[..width].rfind(|c: char| c.is_whitespace() || c == '/' || c == '-')
                    .map(|i| i + 1)
                    .unwrap_or(width);
                out.push(remaining[..break_at].to_string());
                remaining = &remaining[break_at..];
            }
            if !remaining.is_empty() {
                out.push(remaining.to_string());
            }
        }
    }
    out
}

pub(super) async fn compute_digest(db: Arc<Database<'static>>, since: &str) -> Result<Vec<String>> {
    let dur = crate::shared::output::parse_duration(since)?;
    let cutoff = Utc::now().naive_utc() - dur;

    spawn_blocking(move || {
        let r = db.r_transaction()?;
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let all_feeds: Vec<Feed> = r.scan().primary()?.all()?.filter_map(|f| f.ok()).collect();
        let feed_map: HashMap<String, &Feed> = all_feeds.iter().map(|f| (f.id.clone(), f)).collect();

        let mut grouped: HashMap<String, Vec<&Item>> = HashMap::new();
        for item in &all_items {
            if let Some(ref pub_at) = item.published_at {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(pub_at).map(|d| d.naive_utc()) {
                    if dt >= cutoff {
                        grouped.entry(item.feed_id.clone()).or_default().push(item);
                    }
                }
            }
        }

        let mut lines = Vec::new();
        let total: usize = grouped.values().map(|v| v.len()).sum();
        lines.push(format!("Digest: {} new items since {}", total, cutoff.format("%Y-%m-%d %H:%M")));
        lines.push(String::new());

        let mut feed_ids: Vec<&String> = grouped.keys().collect();
        feed_ids.sort();
        for fid in feed_ids {
            let items = &grouped[fid];
            let feed_title = feed_map.get(fid).and_then(|f| f.title.as_deref()).unwrap_or(fid);
            lines.push(format!("{} ({} new)", feed_title, items.len()));
            for item in items.iter().take(5) {
                let title = item.title.as_deref().unwrap_or("(untitled)");
                lines.push(format!("  - {}", title));
            }
            if items.len() > 5 {
                lines.push(format!("  ...and {} more", items.len() - 5));
            }
            lines.push(String::new());
        }
        Ok::<_, anyhow::Error>(lines)
    }).await?
}

pub(super) async fn compute_trending(db: Arc<Database<'static>>) -> Result<Vec<String>> {
    let cutoff = Utc::now().naive_utc() - chrono::Duration::hours(48);
    let sw = stop_words::get(stop_words::LANGUAGE::English);
    let stopwords: HashSet<String> = sw.into_iter().collect();

    spawn_blocking(move || {
        let r = db.r_transaction()?;
        let all_items: Vec<Item> = r.scan().primary()?.all()?.filter_map(|i| i.ok()).collect();
        let mut word_stats: HashMap<String, (usize, HashSet<String>)> = HashMap::new();

        for item in &all_items {
            let in_window = item.published_at.as_ref()
                .and_then(|p| chrono::DateTime::parse_from_rfc3339(p).ok().map(|d| d.naive_utc()))
                .is_some_and(|dt| dt >= cutoff);
            if !in_window { continue; }
            if let Some(ref title) = item.title {
                for word in title.split_whitespace() {
                    let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
                    if w.len() < 2 || stopwords.contains(w.as_str()) { continue; }
                    let entry = word_stats.entry(w).or_insert_with(|| (0, HashSet::new()));
                    entry.0 += 1;
                    entry.1.insert(item.feed_id.clone());
                }
            }
        }

        let mut topics: Vec<(String, usize, usize)> = word_stats.into_iter()
            .filter(|(_, (_, feeds))| feeds.len() >= 2)
            .map(|(word, (count, feeds))| (word, count, feeds.len()))
            .collect();
        topics.sort_by(|a, b| b.1.cmp(&a.1));

        let mut lines = Vec::new();
        lines.push(format!("{:<20} {:>8} {:>6}", "TOPIC", "MENTIONS", "FEEDS"));
        lines.push(String::new());
        for (topic, mentions, feeds) in topics.iter().take(30) {
            lines.push(format!("{:<20} {:>8} {:>6}", topic, mentions, feeds));
        }
        if topics.is_empty() {
            lines.push("No trending topics found (need items from 2+ feeds in last 48h).".to_string());
        }
        Ok::<_, anyhow::Error>(lines)
    }).await?
}

pub(super) async fn export_item_to_file(
    db: Arc<Database<'static>>,
    item_id: String,
    ext: String,
    format_name: String,
) -> Result<String> {

    let (item, feed, mark) = spawn_blocking({
        let db = Arc::clone(&db);
        let id = item_id.clone();
        move || {
            let r = db.r_transaction()?;
            let item: Item = r.get().primary(id.clone())?
                .ok_or_else(|| anyhow::anyhow!("item {id} not found"))?;
            let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
            let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
            Ok::<_, anyhow::Error>((item, feed, mark))
        }
    }).await??;

    let title = item.title.as_deref().unwrap_or("(untitled)");
    let source = feed.as_ref().and_then(|f| f.title.as_deref()).unwrap_or("-");
    let published = item.published_at.as_deref().unwrap_or("-");
    let url = item.link.as_deref().unwrap_or("-");
    let note = mark.as_ref().and_then(|m| m.note.as_deref()).unwrap_or("");
    let content = item.content.as_deref().or(item.summary.as_deref()).unwrap_or("");

    let output = match ext.as_str() {
        "md" => {
            let mut s = format!("# {title}\n\n**Source:** {source}\n**Published:** {published}\n**URL:** {url}\n");
            if !note.is_empty() { s.push_str(&format!("**Note:** {note}\n")); }
            s.push_str(&format!("\n---\n\n{content}"));
            s
        }
        "json" => {
            let json = crate::shared::db::ItemJson::from_parts(&item, feed.as_ref(), mark.as_ref());
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
        }
        _ => {
            let mut s = format!("{title}\nSource: {source}\nPublished: {published}\nURL: {url}\n");
            if !note.is_empty() { s.push_str(&format!("Note: {note}\n")); }
            s.push_str(&format!("\n{content}"));
            s
        }
    };

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&home).join(".rssdude").join("export");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.{}", item_id, ext));
    std::fs::write(&path, &output)?;

    Ok(format!("Exported ({format_name}) to {}", path.display()))
}
