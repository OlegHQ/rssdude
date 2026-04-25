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
    crate::shared::output::strip_html(raw, width)
}

/// Extract all complete URLs from text.
pub(super) fn extract_urls(text: &str) -> Vec<String> {
    linkify::LinkFinder::new()
        .links(text)
        .filter(|l| matches!(l.kind(), linkify::LinkKind::Url))
        .map(|l| l.as_str().to_string())
        .collect()
}

/// Word-wrap a string to fit within `width` columns.
pub(super) fn wrap_text(s: &str, width: usize) -> Vec<String> {
    textwrap::wrap(s, width).into_iter().map(|cow| cow.into_owned()).collect()
}

pub(super) async fn compute_digest(db: Arc<Database<'static>>, since: &str) -> Result<Vec<String>> {
    let digest = crate::commands::discover::digest_core(db, since.to_string(), None).await?;
    let mut lines = Vec::new();
    lines.push(format!("Digest: {} new items since {}", digest.total, digest.since));
    lines.push(String::new());
    for feed in &digest.feeds {
        lines.push(format!("{} ({} new)", feed.feed_title, feed.items.len()));
        for item in feed.items.iter().take(5) {
            lines.push(format!("  - {}", item.title));
        }
        if feed.items.len() > 5 {
            lines.push(format!("  ...and {} more", feed.items.len() - 5));
        }
        lines.push(String::new());
    }
    Ok(lines)
}

pub(super) async fn compute_trending(db: Arc<Database<'static>>) -> Result<Vec<String>> {
    let topics = crate::commands::discover::trending_core(db).await?;
    let mut lines = Vec::new();
    lines.push(format!("{:<20} {:>8} {:>6}", "TOPIC", "MENTIONS", "FEEDS"));
    lines.push(String::new());
    for t in topics.iter().take(30) {
        lines.push(format!("{:<20} {:>8} {:>6}", t.topic, t.mentions, t.feeds));
    }
    if topics.is_empty() {
        lines.push("No trending topics found (need items from 2+ feeds in last 48h).".to_string());
    }
    Ok(lines)
}

pub(super) async fn export_item_to_file(
    db: Arc<Database<'static>>,
    item_id: String,
    ext: String,
    format_name: String,
) -> Result<String> {
    let item_json = crate::commands::curate::export_core(db, item_id.clone()).await?;
    let output = crate::shared::output::format_export(&item_json, &ext)?;

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&home).join(".rssdude").join("export");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.{}", item_id, ext));
    std::fs::write(&path, &output)?;

    Ok(format!("Exported ({format_name}) to {}", path.display()))
}
