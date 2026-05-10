use std::sync::Arc;

use anyhow::Result;
use native_db::Database;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

use crate::shared::db::*;

/// Full TUI snapshot — every collection the TUI needs to render its panes.
/// Sent over the wire as one JSON blob so a remote TUI does a single round-trip
/// per refresh instead of N requests.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Snapshot {
    pub feeds: Vec<Feed>,
    pub folders: Vec<Folder>,
    pub items: Vec<Item>,
    pub marks: Vec<Mark>,
    pub boards: Vec<Board>,
    pub board_items: Vec<BoardItem>,
    pub watches: Vec<SavedSearch>,
}

pub async fn snapshot_core(db: Arc<Database<'static>>) -> Result<Snapshot> {
    spawn_blocking(move || {
        let r = db.r_transaction()?;
        Ok::<_, anyhow::Error>(Snapshot {
            feeds: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            folders: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            items: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            marks: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            boards: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            board_items: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
            watches: r.scan().primary()?.all()?.filter_map(|x| x.ok()).collect(),
        })
    })
    .await?
}
