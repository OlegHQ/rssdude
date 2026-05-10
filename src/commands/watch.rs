use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;

pub async fn list_core(db: Arc<Database<'static>>) -> Result<Vec<SavedSearch>> {
    spawn_blocking(move || -> Result<Vec<SavedSearch>> {
        let r = db.r_transaction()?;
        Ok(r.scan().primary()?.all()?.filter_map(|s| s.ok()).collect())
    }).await?
}

pub async fn create_core(db: Arc<Database<'static>>, name: String, query: String) -> Result<SavedSearch> {
    let ss = SavedSearch {
        id: gen_id(),
        name,
        query,
        created_at: Utc::now().to_rfc3339(),
    };
    let ss2 = ss.clone();
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        rw.insert(ss2)?;
        rw.commit()?;
        Ok(())
    }).await??;
    Ok(ss)
}

pub async fn delete_core(db: Arc<Database<'static>>, id: String) -> Result<String> {
    spawn_blocking(move || -> Result<String> {
        let rw = db.rw_transaction()?;
        let ss: SavedSearch = rw.get().primary(id.clone())?
            .with_context(|| format!("watch {id} not found"))?;
        let name = ss.name.clone();
        rw.remove(ss)?;
        rw.commit()?;
        Ok(name)
    }).await?
}
