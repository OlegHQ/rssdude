use std::sync::Arc;
use anyhow::{Context, Result};
use chrono::Utc;
use native_db::Database;
use tokio::task::spawn_blocking;
use crate::shared::db::*;
use crate::shared::output::*;

/// Add mute filters (comma-separated patterns).
pub async fn mute_add_core(
    db: Arc<Database<'static>>,
    patterns: String,
    filter_type: String,
    until: Option<String>,
) -> Result<Vec<MuteFilterJson>> {
    let expires_at = match until {
        Some(ref s) => {
            // Try parsing as duration first, then as date
            if let Ok(dur) = parse_duration(s) {
                Some((Utc::now() + dur).to_rfc3339())
            } else if let Ok(dt) = parse_datetime(s) {
                Some(dt.and_utc().to_rfc3339())
            } else {
                anyhow::bail!("invalid --until value: {s} (expected duration like '7d' or datetime)")
            }
        }
        None => None,
    };

    let now = Utc::now().to_rfc3339();
    let parts: Vec<String> = patterns.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        anyhow::bail!("no patterns provided");
    }

    let ft = filter_type.clone();
    let filters: Vec<MuteFilter> = parts.iter().map(|p| MuteFilter {
        id: gen_id(),
        pattern: p.clone(),
        filter_type: ft.clone(),
        expires_at: expires_at.clone(),
        created_at: now.clone(),
    }).collect();

    let to_insert = filters.clone();
    spawn_blocking(move || -> Result<()> {
        let rw = db.rw_transaction()?;
        for f in to_insert {
            rw.insert(f)?;
        }
        rw.commit()?;
        Ok(())
    }).await??;

    Ok(filters.iter().map(MuteFilterJson::from).collect())
}

/// List all active (non-expired) mute filters.
pub async fn mute_list_core(db: Arc<Database<'static>>) -> Result<Vec<MuteFilterJson>> {
    spawn_blocking(move || -> Result<Vec<MuteFilterJson>> {
        let r = db.r_transaction()?;
        let now = Utc::now().to_rfc3339();
        let filters: Vec<MuteFilterJson> = r.scan().primary::<MuteFilter>()?.all()?
            .filter_map(|f| f.ok())
            .filter(|f: &MuteFilter| {
                f.expires_at.as_ref().is_none_or(|exp| exp.as_str() >= now.as_str())
            })
            .map(|f| MuteFilterJson::from(&f))
            .collect();
        Ok(filters)
    }).await?
}

/// Remove a mute filter by ID.
pub async fn mute_remove_core(db: Arc<Database<'static>>, id: String) -> Result<String> {
    spawn_blocking(move || -> Result<String> {
        let rw = db.rw_transaction()?;
        let filter: MuteFilter = rw.get().primary(id.clone())?
            .with_context(|| format!("mute filter {id} not found"))?;
        let pattern = filter.pattern.clone();
        rw.remove(filter)?;
        rw.commit()?;
        Ok(pattern)
    }).await?
}

// CLI wrappers

pub async fn mute_add(db: Arc<Database<'static>>, json: bool, patterns: String, filter_type: String, until: Option<String>) -> Result<()> {
    let filters = mute_add_core(db, patterns, filter_type, until).await?;
    if json {
        print_json(&filters);
    } else {
        for f in &filters {
            let expiry = f.expires_at.as_deref().map(time_ago).unwrap_or_else(|| "never".into());
            println!("Muted {} \"{}\" (expires: {})", f.filter_type, f.pattern, expiry);
        }
    }
    Ok(())
}

pub async fn mute_list(db: Arc<Database<'static>>, json: bool) -> Result<()> {
    let filters = mute_list_core(db).await?;
    if json {
        print_json(&filters);
    } else {
        let rows: Vec<Vec<String>> = filters.iter().map(|f| {
            vec![
                f.id.clone(),
                f.filter_type.clone(),
                f.pattern.clone(),
                f.expires_at.as_deref().map(time_ago).unwrap_or_else(|| "permanent".into()),
            ]
        }).collect();
        print_table(&["ID", "TYPE", "PATTERN", "EXPIRES"], &rows);
    }
    Ok(())
}

pub async fn mute_remove(db: Arc<Database<'static>>, json: bool, id: String) -> Result<()> {
    let pattern = mute_remove_core(db, id).await?;
    if json {
        print_json(&serde_json::json!({"removed": pattern}));
    } else {
        println!("Removed mute filter: {pattern}");
    }
    Ok(())
}
