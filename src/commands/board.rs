use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::output::*;

pub async fn create_core(db: Arc<Database<'static>>, name: String) -> Result<Board> {
    let board = Board {
        id: gen_id(),
        name,
        created_at: Utc::now().to_rfc3339(),
    };
    let b = board.clone();
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        rw.insert(b)?;
        rw.commit()?;
        Ok(())
    }).await??;
    Ok(board)
}

pub async fn delete_core(db: Arc<Database<'static>>, id: String) -> Result<String> {
    spawn_blocking(move || -> Result<String> {
        let rw = db.rw_transaction()?;
        let board: Board = rw.get().primary(id.clone())?
            .with_context(|| format!("board {id} not found"))?;
        let name = board.name.clone();
        // Remove all board items
        let board_items: Vec<BoardItem> = rw.scan().secondary::<BoardItem>(BoardItemKey::board_id)?
            .all()?.filter_map(|r| r.ok()).filter(|bi: &BoardItem| bi.board_id == id).collect();
        for bi in board_items { rw.remove(bi)?; }
        rw.remove(board)?;
        rw.commit()?;
        Ok(name)
    }).await?
}

pub async fn list_core(db: Arc<Database<'static>>) -> Result<Vec<BoardJson>> {
    spawn_blocking(move || -> Result<Vec<BoardJson>> {
        let r = db.r_transaction()?;
        let boards: Vec<Board> = r.scan().primary()?.all()?.filter_map(|b| b.ok()).collect();
        let mut result = Vec::new();
        for board in &boards {
            let count = r.scan().secondary::<BoardItem>(BoardItemKey::board_id)?
                .all()?.filter_map(|r| r.ok()).filter(|bi: &BoardItem| bi.board_id == board.id).count();
            result.push(BoardJson {
                id: board.id.clone(),
                name: board.name.clone(),
                created_at: board.created_at.clone(),
                item_count: count,
            });
        }
        Ok(result)
    }).await?
}

pub async fn show_core(db: Arc<Database<'static>>, id: String) -> Result<(Board, Vec<ItemJson>)> {
    spawn_blocking(move || -> Result<(Board, Vec<ItemJson>)> {
        let r = db.r_transaction()?;
        let board: Board = r.get().primary(id.clone())?
            .with_context(|| format!("board {id} not found"))?;
        let board_items: Vec<BoardItem> = r.scan().secondary::<BoardItem>(BoardItemKey::board_id)?
            .all()?.filter_map(|r| r.ok()).filter(|bi: &BoardItem| bi.board_id == id).collect();
        let mut items = Vec::new();
        for bi in &board_items {
            if let Ok(Some(item)) = r.get().primary::<Item>(bi.item_id.clone()) {
                let feed: Option<Feed> = r.get().primary(item.feed_id.clone()).ok().flatten();
                let mark: Option<Mark> = r.get().primary(item.id.clone()).ok().flatten();
                items.push(ItemJson::from_parts(&item, feed.as_ref(), mark.as_ref()));
            }
        }
        Ok((board, items))
    }).await?
}

pub async fn add_item_core(db: Arc<Database<'static>>, board_id: String, item_id: String, note: Option<String>) -> Result<BoardItem> {
    let bi = BoardItem {
        id: format!("{}:{}", board_id, item_id),
        board_id,
        item_id,
        added_at: Utc::now().to_rfc3339(),
        note,
    };
    let bi2 = bi.clone();
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        // Verify board exists
        let _: Board = rw.get().primary(bi2.board_id.clone())?
            .with_context(|| format!("board {} not found", bi2.board_id))?;
        let _: Option<BoardItem> = rw.upsert(bi2)?;
        rw.commit()?;
        Ok(())
    }).await??;
    Ok(bi)
}

pub async fn remove_item_core(db: Arc<Database<'static>>, board_id: String, item_id: String) -> Result<()> {
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        let pk = format!("{board_id}:{item_id}");
        let bi: BoardItem = rw.get().primary(pk.clone())?
            .with_context(|| "item not in board")?;
        rw.remove(bi)?;
        rw.commit()?;
        Ok(())
    }).await?
}

// CLI wrappers

pub async fn create(db: Arc<Database<'static>>, json: bool, name: String) -> Result<()> {
    let board = create_core(db, name).await?;
    if json { print_json(&BoardJson { id: board.id.clone(), name: board.name.clone(), created_at: board.created_at.clone(), item_count: 0 }); }
    else { println!("Created board: {} (id: {})", board.name, board.id); }
    Ok(())
}

pub async fn delete(db: Arc<Database<'static>>, json: bool, id: String) -> Result<()> {
    let name = delete_core(db, id).await?;
    if json { print_json(&serde_json::json!({"deleted": name})); }
    else { println!("Deleted board: {name}"); }
    Ok(())
}

pub async fn list(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let boards = list_core(db).await?;
    if json { print_json(&boards); }
    else {
        let rows: Vec<Vec<String>> = boards.iter().map(|b| vec![
            b.id.clone(), b.name.clone(), b.item_count.to_string(), b.created_at.clone(),
        ]).collect();
        print_table(&["ID", "NAME", "ITEMS", "CREATED"], &rows);
    }
    Ok(())
}

pub async fn show(db: Arc<Database<'static>>, json: bool, id: String) -> Result<()> {
    let (board, items) = show_core(db, id).await?;
    if json { print_json(&serde_json::json!({"board": board.name, "items": items})); }
    else {
        println!("Board: {} ({} items)", board.name, items.len());
        let rows: Vec<Vec<String>> = items.iter().map(|ij| vec![
            ij.id.clone(),
            ij.source.clone().unwrap_or("-".into()),
            ij.title.clone().unwrap_or("-".into()),
        ]).collect();
        print_table(&["ID", "SOURCE", "TITLE"], &rows);
    }
    Ok(())
}

pub async fn add_item(db: Arc<Database<'static>>, json: bool, board_id: String, item_id: String, note: Option<String>) -> Result<()> {
    let bi = add_item_core(db, board_id, item_id, note).await?;
    if json { print_json(&serde_json::json!({"board_id": bi.board_id, "item_id": bi.item_id})); }
    else { println!("Added item to board."); }
    Ok(())
}

pub async fn remove_item(db: Arc<Database<'static>>, json: bool, board_id: String, item_id: String) -> Result<()> {
    remove_item_core(db, board_id, item_id).await?;
    if json { print_json(&serde_json::json!({"removed": true})); }
    else { println!("Removed item from board."); }
    Ok(())
}
