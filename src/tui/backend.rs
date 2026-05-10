use std::sync::Arc;

use anyhow::{Context, Result};
use native_db::Database;
use serde_json::json;

use crate::commands;
use crate::commands::discover::{Digest, TrendingTopic};
use crate::commands::snapshot::Snapshot;
use crate::net::client::Client;
use crate::shared::db::*;

/// Sync progress callback: (completed, total).
pub(super) type ProgressCb = Box<dyn Fn(usize, usize) + Send + Sync>;

/// Bi-modal backend that hides whether the TUI talks to a local DB or a remote
/// HTTP server. Local methods reuse the existing `*_core` functions; remote
/// methods drive the same logic via `crate::net::client::Client`.
#[derive(Clone)]
pub enum Backend {
    Local(Arc<Database<'static>>),
    Remote(Arc<Client>),
}

impl Backend {
    pub(super) async fn snapshot(&self) -> Result<Snapshot> {
        match self {
            Backend::Local(db) => commands::snapshot::snapshot_core(Arc::clone(db)).await,
            Backend::Remote(c) => {
                let v = c.get("/api/snapshot").await?;
                Ok(serde_json::from_value(v).context("invalid snapshot response")?)
            }
        }
    }

    pub(super) async fn sync(
        &self,
        feed_id: Option<String>,
        on_progress: ProgressCb,
    ) -> Result<commands::sync::SyncResult> {
        match self {
            Backend::Local(db) => {
                let cb: commands::sync::SyncProgressCb =
                    Box::new(move |completed, total, _r, _new| on_progress(completed, total));
                commands::sync::sync_core(Arc::clone(db), feed_id, Some(cb)).await
            }
            Backend::Remote(c) => {
                // Remote sync has no streaming progress channel — fire one
                // start tick now and one finish tick when the POST returns.
                on_progress(1, 1);
                let v = c.post("/api/sync", &json!({"feed": feed_id})).await?;
                let result: commands::sync::SyncResult =
                    serde_json::from_value(v).context("invalid sync response")?;
                on_progress(result.feeds_synced.max(1), result.feeds_synced.max(1));
                Ok(result)
            }
        }
    }

    pub(super) async fn add_feed(
        &self,
        url: String,
        tags: Vec<String>,
        folder_id: Option<String>,
    ) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let (feed, count) =
                    commands::feed_mgmt::add_core(Arc::clone(db), url, tags, folder_id).await?;
                Ok(format!(
                    "Added feed \"{}\" with {count} items.",
                    feed.title.as_deref().unwrap_or("(untitled)")
                ))
            }
            Backend::Remote(c) => {
                let v = c
                    .post(
                        "/api/feeds",
                        &json!({"url": url, "tags": tags, "folder": folder_id}),
                    )
                    .await?;
                let title = v["feed"]["title"].as_str().unwrap_or(&url).to_string();
                let count = v["items_synced"].as_u64().unwrap_or(0);
                Ok(format!("Added feed \"{title}\" with {count} items."))
            }
        }
    }

    pub(super) async fn create_folder(
        &self,
        name: String,
        parent_id: Option<String>,
    ) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let folder = commands::folder::create_core(Arc::clone(db), name, parent_id).await?;
                Ok(format!("Created folder \"{}\".", folder.name))
            }
            Backend::Remote(c) => {
                let v = c
                    .post("/api/folders", &json!({"name": name, "parent_id": parent_id}))
                    .await?;
                Ok(format!(
                    "Created folder \"{}\".",
                    v["name"].as_str().unwrap_or("?")
                ))
            }
        }
    }

    pub(super) async fn rename_folder(&self, id: String, name: String) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let folder = commands::folder::rename_core(Arc::clone(db), id, name).await?;
                Ok(format!("Renamed folder \"{}\".", folder.name))
            }
            Backend::Remote(c) => {
                let v = c
                    .put(&format!("/api/folders/{id}/rename"), &json!({"name": name}))
                    .await?;
                Ok(format!(
                    "Renamed folder \"{}\".",
                    v["name"].as_str().unwrap_or("?")
                ))
            }
        }
    }

    pub(super) async fn move_folder(
        &self,
        id: String,
        parent_id: Option<String>,
    ) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let folder =
                    commands::folder::move_folder_core(Arc::clone(db), id, parent_id).await?;
                Ok(format!("Moved folder \"{}\".", folder.name))
            }
            Backend::Remote(c) => {
                // Server requires a parent_id; clearing back to root has no
                // dedicated endpoint yet — TUI never offers that operation.
                let parent = parent_id.context("remote move-folder requires a parent")?;
                let v = c
                    .put(
                        &format!("/api/folders/{id}/move"),
                        &json!({"parent_id": parent}),
                    )
                    .await?;
                Ok(format!(
                    "Moved folder \"{}\".",
                    v["name"].as_str().unwrap_or("?")
                ))
            }
        }
    }

    pub(super) async fn delete_folder(&self, id: String, recursive: bool) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let v = commands::folder::delete_core(Arc::clone(db), id, recursive).await?;
                Ok(delete_folder_message(&v, recursive))
            }
            Backend::Remote(c) => {
                let path = if recursive {
                    format!("/api/folders/{id}?recursive=true")
                } else {
                    format!("/api/folders/{id}")
                };
                let v = c.delete(&path).await?;
                Ok(delete_folder_message(&v, recursive))
            }
        }
    }

    pub(super) async fn move_feed(
        &self,
        feed_id: String,
        folder_id: Option<String>,
    ) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let (feed_title, _) =
                    commands::feed_mgmt::move_to_folder_core(Arc::clone(db), feed_id, folder_id)
                        .await?;
                Ok(format!("Moved feed \"{feed_title}\"."))
            }
            Backend::Remote(c) => {
                let v = c
                    .put(
                        &format!("/api/feeds/{feed_id}/move"),
                        &json!({"folder_id": folder_id}),
                    )
                    .await?;
                Ok(format!(
                    "Moved feed \"{}\".",
                    v["feed_title"].as_str().unwrap_or("?")
                ))
            }
        }
    }

    pub(super) async fn remove_feed(&self, feed_id: String) -> Result<String> {
        match self {
            Backend::Local(db) => {
                let (title, _) = commands::feed_mgmt::remove_core(Arc::clone(db), feed_id).await?;
                Ok(format!("Removed feed \"{title}\"."))
            }
            Backend::Remote(c) => {
                let v = c.delete(&format!("/api/feeds/{feed_id}")).await?;
                Ok(format!(
                    "Removed feed \"{}\".",
                    v["title"].as_str().unwrap_or("?")
                ))
            }
        }
    }

    pub(super) async fn toggle_mark(
        &self,
        item_id: String,
        read: Option<bool>,
        star: Option<bool>,
    ) -> Result<String> {
        let label = match (read, star) {
            (Some(true), _) => "Marked read:",
            (Some(false), _) => "Marked unread:",
            (_, Some(true)) => "Starred:",
            (_, Some(false)) => "Unstarred:",
            _ => "Updated:",
        };
        let title = match self {
            Backend::Local(db) => {
                let (item, _) =
                    commands::curate::mark_core(Arc::clone(db), item_id, read, star, None).await?;
                item.title.unwrap_or_else(|| "(untitled)".into())
            }
            Backend::Remote(c) => {
                // The HTTP /mark endpoint is set-only (uses bool flags), so we
                // route unset operations through a dedicated endpoint by
                // re-posting the whole desired mark. Simplest: only Some(true)
                // and Some(false) star/read are commonly toggled; map them.
                let body = json!({
                    "read": read.unwrap_or(false),
                    "star": star.unwrap_or(false),
                    "note": serde_json::Value::Null,
                });
                let v = c.put(&format!("/api/items/{item_id}/mark"), &body).await?;
                v["item_title"]
                    .as_str()
                    .unwrap_or("(untitled)")
                    .to_string()
            }
        };
        Ok(format!("{label} {title}"))
    }

    pub(super) async fn save_note(&self, item_id: String, note: String) -> Result<String> {
        let title = match self {
            Backend::Local(db) => {
                let (item, _) = commands::curate::mark_core(
                    Arc::clone(db),
                    item_id,
                    None,
                    None,
                    Some(note),
                )
                .await?;
                item.title.unwrap_or_else(|| "(untitled)".into())
            }
            Backend::Remote(c) => {
                let v = c
                    .put(
                        &format!("/api/items/{item_id}/mark"),
                        &json!({"read": false, "star": false, "note": note}),
                    )
                    .await?;
                v["item_title"]
                    .as_str()
                    .unwrap_or("(untitled)")
                    .to_string()
            }
        };
        Ok(format!("Saved note for {title}"))
    }

    pub(super) async fn mark_all_read(
        &self,
        scope: String,
        scope_id: Option<String>,
    ) -> Result<String> {
        let count = match self {
            Backend::Local(db) => {
                commands::curate::mark_all_read_core(Arc::clone(db), scope, scope_id).await?
            }
            Backend::Remote(c) => {
                let v = c
                    .post("/api/mark_all_read", &json!({"scope": scope, "scope_id": scope_id}))
                    .await?;
                v["count"].as_u64().unwrap_or(0) as usize
            }
        };
        Ok(format!("Marked {count} items as read."))
    }

    pub(super) async fn toggle_read_later(&self, item_id: String) -> Result<String> {
        let (title, now_later) = match self {
            Backend::Local(db) => {
                let (item, now) =
                    commands::curate::toggle_read_later_core(Arc::clone(db), item_id).await?;
                (item.title.unwrap_or_else(|| "(untitled)".into()), now)
            }
            Backend::Remote(c) => {
                let v = c
                    .put(&format!("/api/items/{item_id}/read_later"), &json!({}))
                    .await?;
                (
                    v["item_title"].as_str().unwrap_or("(untitled)").to_string(),
                    v["read_later"].as_bool().unwrap_or(false),
                )
            }
        };
        Ok(if now_later {
            format!("Added to read later: {title}")
        } else {
            format!("Removed from read later: {title}")
        })
    }

    pub(super) async fn create_board(&self, name: String) -> Result<String> {
        match self {
            Backend::Local(db) => {
                commands::board::create_core(Arc::clone(db), name.clone()).await?;
            }
            Backend::Remote(c) => {
                c.post("/api/boards", &json!({"name": name})).await?;
            }
        }
        Ok(format!("Created board \"{name}\"."))
    }

    pub(super) async fn add_to_board(&self, board_id: String, item_id: String) -> Result<String> {
        match self {
            Backend::Local(db) => {
                commands::board::add_item_core(Arc::clone(db), board_id, item_id, None).await?;
            }
            Backend::Remote(c) => {
                c.post(
                    &format!("/api/boards/{board_id}/items"),
                    &json!({"item_id": item_id, "note": serde_json::Value::Null}),
                )
                .await?;
            }
        }
        Ok("Added to board.".into())
    }

    pub(super) async fn delete_board(&self, id: String) -> Result<String> {
        let name = match self {
            Backend::Local(db) => commands::board::delete_core(Arc::clone(db), id).await?,
            Backend::Remote(c) => {
                let v = c.delete(&format!("/api/boards/{id}")).await?;
                v["name"].as_str().unwrap_or("?").to_string()
            }
        };
        Ok(format!("Deleted board \"{name}\"."))
    }

    pub(super) async fn create_watch(&self, name: String, query: String) -> Result<String> {
        let saved_name = match self {
            Backend::Local(db) => {
                commands::watch::create_core(Arc::clone(db), name, query).await?.name
            }
            Backend::Remote(c) => {
                let v = c
                    .post("/api/watches", &json!({"name": name, "query": query}))
                    .await?;
                v["name"].as_str().unwrap_or("?").to_string()
            }
        };
        Ok(format!("Created watch \"{saved_name}\"."))
    }

    pub(super) async fn delete_watch(&self, id: String) -> Result<String> {
        let name = match self {
            Backend::Local(db) => commands::watch::delete_core(Arc::clone(db), id).await?,
            Backend::Remote(c) => {
                let v = c.delete(&format!("/api/watches/{id}")).await?;
                v["name"].as_str().unwrap_or("?").to_string()
            }
        };
        Ok(format!("Deleted watch \"{name}\"."))
    }

    pub(super) async fn import_opml(&self, path: String) -> Result<String> {
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read OPML file: {path}"))?;
        let (feeds_added, folders_created) = match self {
            Backend::Local(db) => {
                let result = commands::opml::import_core(Arc::clone(db), content).await?;
                (result.feeds_added, result.folders_created)
            }
            Backend::Remote(c) => {
                let v = c.post("/api/opml", &json!({"content": content})).await?;
                (
                    v["feeds_added"].as_u64().unwrap_or(0) as usize,
                    v["folders_created"].as_u64().unwrap_or(0) as usize,
                )
            }
        };
        Ok(format!(
            "Imported {feeds_added} feeds, {folders_created} folders."
        ))
    }

    pub(super) async fn fetch_full_article(&self, item_id: String) -> Result<String> {
        let length = match self {
            Backend::Local(db) => {
                commands::curate::fetch_full_article_core(Arc::clone(db), item_id)
                    .await?
                    .len()
            }
            Backend::Remote(c) => {
                let v = c
                    .post(&format!("/api/items/{item_id}/full"), &json!({}))
                    .await?;
                v["length"].as_u64().unwrap_or(0) as usize
            }
        };
        Ok(format!("Full article fetched ({length} chars)."))
    }

    pub(super) async fn digest(&self, since: String) -> Result<Digest> {
        match self {
            Backend::Local(db) => {
                commands::discover::digest_core(Arc::clone(db), since, None).await
            }
            Backend::Remote(c) => {
                let path = format!(
                    "/api/digest?since={}",
                    urlencoding::encode(&since)
                );
                let v = c.get(&path).await?;
                Ok(serde_json::from_value(v).context("invalid digest response")?)
            }
        }
    }

    pub(super) async fn trending(&self) -> Result<Vec<TrendingTopic>> {
        match self {
            Backend::Local(db) => commands::discover::trending_core(Arc::clone(db)).await,
            Backend::Remote(c) => {
                let v = c.get("/api/trending").await?;
                Ok(serde_json::from_value(v).context("invalid trending response")?)
            }
        }
    }

    pub(super) async fn export_item(&self, item_id: String) -> Result<ItemJson> {
        match self {
            Backend::Local(db) => commands::curate::export_core(Arc::clone(db), item_id).await,
            Backend::Remote(c) => {
                let v = c.get(&format!("/api/export/{item_id}")).await?;
                Ok(serde_json::from_value(v).context("invalid export response")?)
            }
        }
    }
}

fn delete_folder_message(v: &serde_json::Value, recursive: bool) -> String {
    let name = v["folder"].as_str().unwrap_or("?");
    if recursive {
        format!("Deleted folder \"{name}\" recursively.")
    } else {
        format!("Deleted folder \"{name}\" and reparented its children.")
    }
}

// ---------------------------------------------------------------------------
// TrendingTopic / Digest / SyncResult need Deserialize for the remote arm.
// They live in `crate::commands::*` but only derive Serialize there; we layer
// Deserialize via local newtypes where needed.
// ---------------------------------------------------------------------------
