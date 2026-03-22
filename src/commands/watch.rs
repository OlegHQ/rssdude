use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::output::*;

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

pub async fn list_core(db: Arc<Database<'static>>) -> Result<Vec<SavedSearchJson>> {
    spawn_blocking(move || -> Result<Vec<SavedSearchJson>> {
        let r = db.r_transaction()?;
        let searches: Vec<SavedSearchJson> = r.scan().primary::<SavedSearch>()?.all()?
            .filter_map(|s| s.ok())
            .map(|s| SavedSearchJson::from(&s))
            .collect();
        Ok(searches)
    }).await?
}

/// Show a saved search with matching items.
pub async fn show_core(db: Arc<Database<'static>>, id: String, limit: usize) -> Result<(SavedSearch, Vec<ItemJson>)> {
    let db2 = Arc::clone(&db);
    let id2 = id.clone();
    let ss = spawn_blocking(move || -> Result<SavedSearch> {
        let r = db2.r_transaction()?;
        r.get().primary(id2.clone())?.with_context(|| format!("watch {id2} not found"))
    }).await??;

    let items = crate::commands::read::search_core(db, ss.query.clone(), limit).await?;

    Ok((ss, items))
}

// CLI wrappers

pub async fn create(db: Arc<Database<'static>>, json: bool, name: String, query: String) -> Result<()> {
    let ss = create_core(db, name, query).await?;
    if json { print_json(&SavedSearchJson::from(&ss)); }
    else { println!("Created watch: \"{}\" (query: \"{}\", id: {})", ss.name, ss.query, ss.id); }
    Ok(())
}

pub async fn delete(db: Arc<Database<'static>>, json: bool, id: String) -> Result<()> {
    let name = delete_core(db, id).await?;
    if json { print_json(&serde_json::json!({"deleted": name})); }
    else { println!("Deleted watch: {name}"); }
    Ok(())
}

pub async fn list(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let searches = list_core(db).await?;
    if json {
        print_json(&searches);
    } else {
        let rows: Vec<Vec<String>> = searches.iter().map(|s| vec![
            s.id.clone(), s.name.clone(), s.query.clone(), s.created_at.clone(),
        ]).collect();
        print_table(&["ID", "NAME", "QUERY", "CREATED"], &rows);
    }
    Ok(())
}

pub async fn show(db: Arc<Database<'static>>, json: bool, id: String, limit: usize) -> Result<()> {
    let (ss, items) = show_core(db, id, limit).await?;
    if json { print_json(&serde_json::json!({"watch": SavedSearchJson::from(&ss), "items": items})); }
    else {
        println!("Watch: \"{}\" (query: \"{}\")", ss.name, ss.query);
        println!("{} matching items:", items.len());
        let rows: Vec<Vec<String>> = items.iter().map(|ij| vec![
            ij.id.clone(),
            ij.source.clone().unwrap_or("-".into()),
            ij.title.clone().unwrap_or("-".into()),
            ij.published_at.as_deref().map(time_ago).unwrap_or("-".into()),
        ]).collect();
        print_table(&["ID", "SOURCE", "TITLE", "PUBLISHED"], &rows);
    }
    Ok(())
}
