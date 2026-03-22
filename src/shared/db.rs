use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use native_db::*;
use native_model::native_model;
use native_model::Model;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 1, version = 1)]
#[native_db]
pub struct FeedV1 {
    #[primary_key]
    pub id: String,
    #[secondary_key(unique)]
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: String,
    pub added_at: String,
    pub last_synced: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 1, version = 2, from = FeedV1)]
#[native_db]
pub struct Feed {
    #[primary_key]
    pub id: String,
    #[secondary_key(unique)]
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: String,
    pub added_at: String,
    pub last_synced: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    #[secondary_key(optional)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub error_count: u32,
    #[serde(default)]
    pub last_success_at: Option<String>,
    #[serde(default)]
    pub custom_title: Option<String>,
}

impl Feed {
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref()
            .or(self.title.as_deref())
            .unwrap_or(&self.url)
    }
}

impl From<FeedV1> for Feed {
    fn from(old: FeedV1) -> Self {
        Self {
            id: old.id,
            url: old.url,
            title: old.title,
            description: old.description,
            tags: old.tags,
            added_at: old.added_at,
            last_synced: old.last_synced,
            etag: old.etag,
            last_modified: old.last_modified,
            folder_id: None,
            last_error: None,
            error_count: 0,
            last_success_at: None,
            custom_title: None,
        }
    }
}

impl From<Feed> for FeedV1 {
    fn from(new: Feed) -> Self {
        Self {
            id: new.id,
            url: new.url,
            title: new.title,
            description: new.description,
            tags: new.tags,
            added_at: new.added_at,
            last_synced: new.last_synced,
            etag: new.etag,
            last_modified: new.last_modified,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 2, version = 1)]
#[native_db]
pub struct Item {
    #[primary_key]
    pub id: String,
    #[secondary_key]
    pub feed_id: String,
    #[secondary_key(unique)]
    pub guid: String,
    pub title: Option<String>,
    pub link: Option<String>,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<String>,
    pub fetched_at: String,
    #[serde(default)]
    pub full_content: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 3, version = 1)]
#[native_db]
pub struct MarkV1 {
    #[primary_key]
    pub item_id: String,
    pub read: bool,
    pub starred: bool,
    pub note: Option<String>,
    pub marked_at: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 3, version = 2, from = MarkV1)]
#[native_db]
pub struct Mark {
    #[primary_key]
    pub item_id: String,
    pub read: bool,
    pub starred: bool,
    pub note: Option<String>,
    pub marked_at: String,
    pub read_at: Option<String>,
    pub opened_at: Option<String>,
    #[serde(default)]
    pub read_later: bool,
}

impl From<MarkV1> for Mark {
    fn from(old: MarkV1) -> Self {
        Mark {
            item_id: old.item_id,
            read: old.read,
            starred: old.starred,
            note: old.note,
            marked_at: old.marked_at,
            read_at: None,
            opened_at: None,
            read_later: false,
        }
    }
}

impl From<Mark> for MarkV1 {
    fn from(new: Mark) -> Self {
        MarkV1 {
            item_id: new.item_id,
            read: new.read,
            starred: new.starred,
            note: new.note,
            marked_at: new.marked_at,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 4, version = 1)]
#[native_db]
pub struct Folder {
    #[primary_key]
    pub id: String,
    pub name: String,
    #[secondary_key(optional)]
    pub parent_id: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 5, version = 1)]
#[native_db]
pub struct Board {
    #[primary_key]
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 6, version = 1)]
#[native_db]
pub struct BoardItem {
    #[primary_key]
    pub id: String,
    #[secondary_key]
    pub board_id: String,
    pub item_id: String,
    pub added_at: String,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 7, version = 1)]
#[native_db]
pub struct SavedSearch {
    #[primary_key]
    pub id: String,
    pub name: String,
    pub query: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[native_model(id = 8, version = 1)]
#[native_db]
pub struct MuteFilter {
    #[primary_key]
    pub id: String,
    pub pattern: String,
    pub filter_type: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Static model registry
// ---------------------------------------------------------------------------

pub static MODELS: Lazy<Models> = Lazy::new(|| {
    let mut models = Models::new();
    models.define::<FeedV1>().expect("FeedV1 model");
    models.define::<Feed>().expect("Feed model");
    models.define::<Item>().expect("Item model");
    models.define::<MarkV1>().expect("MarkV1 model");
    models.define::<Mark>().expect("Mark model");
    models.define::<Folder>().expect("Folder model");
    models.define::<Board>().expect("Board model");
    models.define::<BoardItem>().expect("BoardItem model");
    models.define::<SavedSearch>().expect("SavedSearch model");
    models.define::<MuteFilter>().expect("MuteFilter model");
    models
});

// ---------------------------------------------------------------------------
// Database helpers
// ---------------------------------------------------------------------------

pub async fn open_db(path: &str) -> Result<Arc<Database<'static>>> {
    let path = path.to_owned();
    spawn_blocking(move || {
        if let Some(parent) = Path::new(&path).parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        let db = Builder::new()
            .create(&MODELS, &path)
            .with_context(|| format!("failed to open database at {path}"))?;
        Ok(Arc::new(db))
    })
    .await?
}

pub fn db_path() -> String {
    if let Ok(path) = std::env::var("RSSDUDE_DB_PATH") {
        return path;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{home}/.rssdude/rssdude.redb")
}

pub fn gen_id() -> String {
    nanoid::nanoid!(8, &nanoid::alphabet::SAFE)
}

/// BFS to collect a folder and all its descendants. Shared helper.
pub fn collect_descendant_ids(all_folders: &[Folder], root_id: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    result.insert(root_id.to_string());
    let mut queue = vec![root_id.to_string()];
    while let Some(parent) = queue.pop() {
        for f in all_folders {
            if f.parent_id.as_deref() == Some(&parent) && result.insert(f.id.clone()) {
                queue.push(f.id.clone());
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Shared feed stats computation (used by sync, folder, tui)
// ---------------------------------------------------------------------------

#[derive(Clone, Default, Debug, Serialize)]
pub struct FeedStatsEntry {
    pub items: usize,
    pub unread: usize,
    pub starred: usize,
}

/// Compute per-feed stats (items, unread, starred) from items and marks.
pub fn compute_feed_stats(items: &[Item], marks: &[Mark]) -> HashMap<String, FeedStatsEntry> {
    let mark_map: HashMap<&str, &Mark> = marks.iter().map(|m| (m.item_id.as_str(), m)).collect();
    let mut stats: HashMap<String, FeedStatsEntry> = HashMap::new();
    for item in items {
        let entry = stats.entry(item.feed_id.clone()).or_default();
        entry.items += 1;
        match mark_map.get(item.id.as_str()) {
            Some(mark) => {
                if !mark.read { entry.unread += 1; }
                if mark.starred { entry.starred += 1; }
            }
            None => entry.unread += 1,
        }
    }
    stats
}

// ---------------------------------------------------------------------------
// JSON output structs
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FeedJson {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub added_at: String,
    pub last_synced: Option<String>,
    pub folder_id: Option<String>,
}

impl From<&Feed> for FeedJson {
    fn from(f: &Feed) -> Self {
        Self {
            id: f.id.clone(),
            url: f.url.clone(),
            title: f.title.clone(),
            description: f.description.clone(),
            tags: if f.tags.is_empty() {
                vec![]
            } else {
                f.tags.split(',').map(|s| s.trim().to_string()).collect()
            },
            added_at: f.added_at.clone(),
            last_synced: f.last_synced.clone(),
            folder_id: f.folder_id.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FolderJson {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

impl From<&Folder> for FolderJson {
    fn from(f: &Folder) -> Self {
        Self {
            id: f.id.clone(),
            name: f.name.clone(),
            parent_id: f.parent_id.clone(),
            created_at: f.created_at.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ItemJson {
    pub id: String,
    pub feed_id: String,
    pub source: Option<String>,
    pub guid: String,
    pub title: Option<String>,
    pub link: Option<String>,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<String>,
    pub fetched_at: String,
    pub read: bool,
    pub starred: bool,
    pub note: Option<String>,
    pub read_at: Option<String>,
    pub opened_at: Option<String>,
}

impl ItemJson {
    pub fn from_parts(item: &Item, feed: Option<&Feed>, mark: Option<&Mark>) -> Self {
        Self {
            id: item.id.clone(),
            feed_id: item.feed_id.clone(),
            source: feed.and_then(|f| f.title.clone()),
            guid: item.guid.clone(),
            title: item.title.clone(),
            link: item.link.clone(),
            content: item.content.clone(),
            summary: item.summary.clone(),
            published_at: item.published_at.clone(),
            fetched_at: item.fetched_at.clone(),
            read: mark.is_some_and(|m| m.read),
            starred: mark.is_some_and(|m| m.starred),
            note: mark.and_then(|m| m.note.clone()),
            read_at: mark.and_then(|m| m.read_at.clone()),
            opened_at: mark.and_then(|m| m.opened_at.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BoardJson {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub item_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SavedSearchJson {
    pub id: String,
    pub name: String,
    pub query: String,
    pub created_at: String,
}

impl From<&SavedSearch> for SavedSearchJson {
    fn from(s: &SavedSearch) -> Self {
        Self { id: s.id.clone(), name: s.name.clone(), query: s.query.clone(), created_at: s.created_at.clone() }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MuteFilterJson {
    pub id: String,
    pub pattern: String,
    pub filter_type: String,
    pub expires_at: Option<String>,
    pub created_at: String,
}

impl From<&MuteFilter> for MuteFilterJson {
    fn from(f: &MuteFilter) -> Self {
        Self {
            id: f.id.clone(), pattern: f.pattern.clone(), filter_type: f.filter_type.clone(),
            expires_at: f.expires_at.clone(), created_at: f.created_at.clone(),
        }
    }
}

/// Check if an item is muted by any active filter.
pub fn is_muted(item: &Item, feed: Option<&Feed>, filters: &[MuteFilter]) -> bool {
    let now = chrono::Utc::now().to_rfc3339();
    for f in filters {
        if f.expires_at.as_ref().is_some_and(|exp| exp.as_str() < now.as_str()) { continue; }
        let pattern = f.pattern.to_lowercase();
        match f.filter_type.as_str() {
            "title" => {
                if item.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&pattern)) { return true; }
            }
            "feed" => {
                if feed.is_some_and(|f| f.display_title().to_lowercase().contains(&pattern) || f.url.to_lowercase().contains(&pattern)) { return true; }
            }
            _ => {
                // "keyword" or default: match title or content
                if item.title.as_ref().is_some_and(|t| t.to_lowercase().contains(&pattern)) { return true; }
                if item.summary.as_ref().is_some_and(|s| s.to_lowercase().contains(&pattern)) { return true; }
            }
        }
    }
    false
}
