use std::sync::Arc;
use anyhow::Result;
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;

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
