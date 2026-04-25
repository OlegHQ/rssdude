use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;

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
        let board_items: Vec<BoardItem> = rw.scan().secondary::<BoardItem>(BoardItemKey::board_id)?
            .all()?.filter_map(|r| r.ok()).filter(|bi: &BoardItem| bi.board_id == id).collect();
        for bi in board_items { rw.remove(bi)?; }
        rw.remove(board)?;
        rw.commit()?;
        Ok(name)
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
        let _: Board = rw.get().primary(bi2.board_id.clone())?
            .with_context(|| format!("board {} not found", bi2.board_id))?;
        let _: Option<BoardItem> = rw.upsert(bi2)?;
        rw.commit()?;
        Ok(())
    }).await??;
    Ok(bi)
}
