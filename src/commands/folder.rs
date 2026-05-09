use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;

use crate::shared::db::*;
use crate::shared::output::*;

// ---------------------------------------------------------------------------
// Core functions (return data, no printing)
// ---------------------------------------------------------------------------

pub async fn create_core(db: Arc<Database<'static>>, name: String, parent_id: Option<String>) -> Result<Folder> {
    let folder = Folder { id: gen_id(), name, parent_id: parent_id.clone(), created_at: Utc::now().to_rfc3339() };
    let fc = folder.clone();
    let db2 = Arc::clone(&db);
    spawn_blocking(move || {
        if let Some(ref pid) = parent_id {
            let r = db2.r_transaction()?;
            if r.get().primary::<Folder>(pid.clone())?.is_none() { bail!("Parent folder not found: {pid}"); }
        }
        let rw = db2.rw_transaction()?;
        rw.insert(fc)?;
        rw.commit()?;
        Ok::<_, anyhow::Error>(())
    }).await??;
    Ok(folder)
}

pub async fn rename_core(db: Arc<Database<'static>>, id: String, new_name: String) -> Result<Folder> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let folder: Folder = rw.get().primary(id.clone())?.ok_or_else(|| anyhow::anyhow!("Folder not found: {id}"))?;
        let mut updated = folder.clone();
        updated.name = new_name;
        rw.update(folder, updated.clone())?;
        rw.commit()?;
        Ok(updated)
    }).await?
}

pub async fn move_folder_core(db: Arc<Database<'static>>, id: String, parent_id: Option<String>) -> Result<Folder> {
    spawn_blocking(move || {
        let rw = db.rw_transaction()?;
        let folder: Folder = rw.get().primary(id.clone())?.ok_or_else(|| anyhow::anyhow!("Folder not found: {id}"))?;

        if let Some(ref pid) = parent_id {
            let _: Folder = rw.get().primary(pid.clone())?.ok_or_else(|| anyhow::anyhow!("Parent folder not found: {pid}"))?;
            // Cycle detection
            let all_folders: Vec<Folder> = rw.scan().primary::<Folder>()?.all()?.filter_map(|x| x.ok()).collect();
            let folder_map: HashMap<String, &Folder> = all_folders.iter().map(|f| (f.id.clone(), f)).collect();
            let mut current = Some(pid.clone());
            while let Some(ref cid) = current {
                if cid == &id { bail!("Cannot move folder: would create a cycle"); }
                current = folder_map.get(cid).and_then(|f| f.parent_id.clone());
            }
        }

        let mut updated = folder.clone();
        updated.parent_id = parent_id;
        rw.update(folder, updated.clone())?;
        rw.commit()?;
        Ok(updated)
    }).await?
}

pub async fn delete_core(db: Arc<Database<'static>>, id: String, recursive: bool) -> Result<serde_json::Value> {
    spawn_blocking(move || -> Result<serde_json::Value> {
        let rw = db.rw_transaction()?;
        let folder: Folder = rw.get().primary(id.clone())?.ok_or_else(|| anyhow::anyhow!("Folder not found: {id}"))?;
        if recursive {
            do_recursive_delete(rw, folder)
        } else {
            do_reparent_delete(rw, folder)
        }
    }).await?
}

fn do_recursive_delete(rw: native_db::transaction::RwTransaction<'_>, folder: Folder) -> Result<serde_json::Value> {
    let folder_name = folder.name.clone();
    let all_folders: Vec<Folder> = rw.scan().primary::<Folder>()?.all()?.filter_map(|x| x.ok()).collect();
    let folder_ids = collect_descendant_ids(&all_folders, &folder.id);
    let all_feeds: Vec<Feed> = rw.scan().primary::<Feed>()?.all()?.filter_map(|x| x.ok()).collect();
    let mut feeds_deleted = 0;
    let mut items_deleted = 0;
    for feed in all_feeds {
        if !feed.folder_id.as_ref().is_some_and(|fid| folder_ids.contains(fid.as_str())) { continue; }
        let items: Vec<Item> = rw.scan().secondary::<Item>(ItemKey::feed_id)?
            .all()?.filter_map(|i| i.ok()).filter(|i: &Item| i.feed_id == feed.id).collect();
        for item in &items {
            if let Ok(Some(mark)) = rw.get().primary::<Mark>(item.id.clone()) { let _ = rw.remove(mark); }
            rw.remove(item.clone())?;
        }
        items_deleted += items.len();
        rw.remove(feed)?;
        feeds_deleted += 1;
    }
    for fid in &folder_ids {
        if let Ok(Some(f)) = rw.get().primary::<Folder>(fid.clone()) { let _ = rw.remove(f); }
    }
    rw.commit()?;
    Ok(serde_json::json!({
        "folder": folder_name, "recursive": true,
        "folders_deleted": folder_ids.len(), "feeds_deleted": feeds_deleted, "items_deleted": items_deleted,
    }))
}

fn do_reparent_delete(rw: native_db::transaction::RwTransaction<'_>, folder: Folder) -> Result<serde_json::Value> {
    let folder_name = folder.name.clone();
    let folder_parent = folder.parent_id.clone();
    let all_folders: Vec<Folder> = rw.scan().primary::<Folder>()?.all()?.filter_map(|x| x.ok()).collect();
    let mut reparented_folders = 0;
    let mut reparented_feeds = 0;
    for child in all_folders {
        if child.parent_id.as_deref() == Some(folder.id.as_str()) {
            let mut updated = child.clone();
            updated.parent_id = folder_parent.clone();
            rw.update(child, updated)?;
            reparented_folders += 1;
        }
    }
    let all_feeds: Vec<Feed> = rw.scan().primary::<Feed>()?.all()?.filter_map(|x| x.ok()).collect();
    for feed in all_feeds {
        if feed.folder_id.as_deref() == Some(folder.id.as_str()) {
            let mut updated = feed.clone();
            updated.folder_id = folder_parent.clone();
            rw.update(feed, updated)?;
            reparented_feeds += 1;
        }
    }
    rw.remove(folder)?;
    rw.commit()?;
    Ok(serde_json::json!({
        "folder": folder_name, "recursive": false,
        "reparented_folders": reparented_folders, "reparented_feeds": reparented_feeds,
    }))
}

/// Core: build the folder/feed tree and return the same JSON shape consumed by
/// the CLI text renderer, the `--json` mode, and the HTTP client. Uncategorized
/// feeds appear as a synthetic trailing node with `id == ""`.
pub async fn list_core(db: Arc<Database<'static>>) -> Result<serde_json::Value> {
    let (root_nodes, uncategorized) = spawn_blocking(move || -> Result<TreeRoots> {
        let r = db.r_transaction()?;
        let folders: Vec<Folder> = r.scan().primary::<Folder>()?.all()?.filter_map(|x| x.ok()).collect();
        let feeds: Vec<Feed> = r.scan().primary::<Feed>()?.all()?.filter_map(|x| x.ok()).collect();
        let items: Vec<Item> = r.scan().primary::<Item>()?.all()?.filter_map(|x| x.ok()).collect();
        let marks: Vec<Mark> = r.scan().primary::<Mark>()?.all()?.filter_map(|x| x.ok()).collect();

        let stats_map = compute_feed_stats(&items, &marks);
        let feed_unread: HashMap<String, usize> = feeds.iter()
            .map(|f| (f.id.clone(), stats_map.get(&f.id).map(|s| s.unread).unwrap_or(0)))
            .collect();

        let mut feeds_by_folder: HashMap<Option<String>, Vec<(Feed, usize)>> = HashMap::new();
        for feed in feeds {
            let unread = feed_unread.get(&feed.id).copied().unwrap_or(0);
            feeds_by_folder.entry(feed.folder_id.clone()).or_default().push((feed, unread));
        }
        let mut folders_by_parent: HashMap<Option<String>, Vec<Folder>> = HashMap::new();
        for folder in folders { folders_by_parent.entry(folder.parent_id.clone()).or_default().push(folder); }
        for group in folders_by_parent.values_mut() { group.sort_by_cached_key(|f| f.name.to_lowercase()); }
        for group in feeds_by_folder.values_mut() {
            group.sort_by_key(|a| a.0.display_title().to_lowercase());
        }
        let root_nodes = build_tree(None, &mut folders_by_parent, &mut feeds_by_folder);
        let uncategorized = feeds_by_folder.remove(&None).unwrap_or_default();
        Ok((root_nodes, uncategorized))
    }).await??;

    let mut json_nodes: Vec<serde_json::Value> = root_nodes.into_iter().map(node_to_json).collect();
    if !uncategorized.is_empty() {
        let uncat_unread: usize = uncategorized.iter().map(|(_, u)| *u).sum();
        json_nodes.push(serde_json::json!({
            "id": "", "name": "Uncategorized", "parent_id": null, "unread": uncat_unread,
            "feeds": uncategorized.iter().map(|(f, u)| serde_json::json!({"id": f.id, "title": f.display_title(), "url": f.url, "unread": u})).collect::<Vec<_>>(),
            "children": [],
        }));
    }
    Ok(serde_json::Value::Array(json_nodes))
}

/// Render the tree returned by `list_core` (or fetched from the server) as
/// indented text. Empty input prints the standard "no folders" message.
pub fn render_tree_json(value: &serde_json::Value) {
    let nodes = value.as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    if nodes.is_empty() { println!("No folders or feeds."); return; }
    for node in nodes { render_json_node(node, "", ""); }
}

fn render_json_node(node: &serde_json::Value, header_prefix: &str, child_prefix: &str) {
    let name = node["name"].as_str().unwrap_or("?");
    let unread = node["unread"].as_u64().unwrap_or(0);
    let feeds = node["feeds"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    let children = node["children"].as_array().map(|v| v.as_slice()).unwrap_or(&[]);
    let is_uncategorized = node["id"].as_str() == Some("");
    if is_uncategorized {
        println!("{name}");
        for (i, f) in feeds.iter().enumerate() {
            let connector = if i == feeds.len() - 1 { "└── " } else { "├── " };
            let title = f["title"].as_str().or(f["url"].as_str()).unwrap_or("?");
            let u = f["unread"].as_u64().unwrap_or(0);
            println!("{connector}{title:<40} {u} unread");
        }
        return;
    }
    println!("{header_prefix}{name} ({unread} unread)");
    let total = children.len() + feeds.len();
    let mut idx = 0;
    for child in children {
        idx += 1;
        let is_last = idx == total;
        let (connector, next) = if is_last { ("└── ", format!("{child_prefix}    ")) }
        else { ("├── ", format!("{child_prefix}│   ")) };
        render_json_node(child, &format!("{child_prefix}{connector}"), &next);
    }
    for f in feeds {
        idx += 1;
        let connector = if idx == total { "└── " } else { "├── " };
        let title = f["title"].as_str().or(f["url"].as_str()).unwrap_or("?");
        let u = f["unread"].as_u64().unwrap_or(0);
        println!("{child_prefix}{connector}{title:<40} {u} unread");
    }
}

// ---------------------------------------------------------------------------
// CLI wrappers
// ---------------------------------------------------------------------------

pub async fn create(db: Arc<Database<'static>>, json: bool, name: String, parent: Option<String>) -> Result<()> {
    let folder = create_core(db, name, parent).await?;
    if json { print_json(&FolderJson::from(&folder)); } else { println!("Created folder: {} (id: {})", folder.name, folder.id); }
    Ok(())
}

pub async fn list(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let tree = list_core(db).await?;
    if json { print_json(&tree); } else { render_tree_json(&tree); }
    Ok(())
}

pub async fn rename(db: Arc<Database<'static>>, json: bool, id: String, new_name: String) -> Result<()> {
    let updated = rename_core(db, id, new_name).await?;
    if json { print_json(&FolderJson::from(&updated)); } else { println!("Renamed folder to: {}", updated.name); }
    Ok(())
}

pub async fn move_folder(db: Arc<Database<'static>>, json: bool, id: String, parent_id: String) -> Result<()> {
    let updated = move_folder_core(db, id, Some(parent_id)).await?;
    if json { print_json(&FolderJson::from(&updated)); }
    else { println!("Moved folder: {} -> parent {}", updated.name, updated.parent_id.as_deref().unwrap_or("(none)")); }
    Ok(())
}

pub async fn delete(db: Arc<Database<'static>>, json: bool, id: String, recursive: bool) -> Result<()> {
    let summary = delete_core(db, id, recursive).await?;
    if json { print_json(&summary); } else {
        let name = summary["folder"].as_str().unwrap_or("?");
        if summary["recursive"].as_bool().unwrap_or(false) {
            println!("Deleted folder: {} ({} subfolders, {} feeds, {} items removed)",
                name, summary["folders_deleted"].as_u64().unwrap_or(1) - 1,
                summary["feeds_deleted"], summary["items_deleted"]);
        } else {
            println!("Deleted folder: {} ({} subfolders reparented, {} feeds reparented)",
                name, summary["reparented_folders"], summary["reparented_feeds"]);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tree helpers
// ---------------------------------------------------------------------------

type TreeRoots = (Vec<TreeNode>, Vec<(Feed, usize)>);

struct TreeNode {
    folder: Folder,
    feeds: Vec<(Feed, usize)>,
    children: Vec<TreeNode>,
    total_unread: usize,
}

fn build_tree(parent_id: Option<String>, folders_by_parent: &mut HashMap<Option<String>, Vec<Folder>>,
              feeds_by_folder: &mut HashMap<Option<String>, Vec<(Feed, usize)>>) -> Vec<TreeNode> {
    let folders = folders_by_parent.remove(&parent_id).unwrap_or_default();
    folders.into_iter().map(|folder| {
        let fid = folder.id.clone();
        let feeds = feeds_by_folder.remove(&Some(fid.clone())).unwrap_or_default();
        let children = build_tree(Some(fid), folders_by_parent, feeds_by_folder);
        let feed_unread: usize = feeds.iter().map(|(_, u)| *u).sum();
        let child_unread: usize = children.iter().map(|c| c.total_unread).sum();
        TreeNode { folder, feeds, children, total_unread: feed_unread + child_unread }
    }).collect()
}

fn node_to_json(node: TreeNode) -> serde_json::Value {
    serde_json::json!({
        "id": node.folder.id, "name": node.folder.name, "parent_id": node.folder.parent_id,
        "unread": node.total_unread,
        "feeds": node.feeds.iter().map(|(f, u)| serde_json::json!({"id": f.id, "title": f.title, "url": f.url, "unread": u})).collect::<Vec<_>>(),
        "children": node.children.into_iter().map(node_to_json).collect::<Vec<_>>(),
    })
}

