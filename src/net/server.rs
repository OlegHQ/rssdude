use std::sync::Arc;

use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::Router;
use native_db::Database;
use serde::Deserialize;

use crate::commands;
use crate::commands::read::ItemsQuery;
use crate::shared::db::*;

// ---------------------------------------------------------------------------
// Server state & error handling
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    db: Arc<Database<'static>>,
    token: Option<String>,
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let msg = format!("{:#}", self.0);
        let lower = msg.to_lowercase();
        let status = if lower.contains("not found") {
            StatusCode::NOT_FOUND
        } else if lower.contains("already exists") {
            StatusCode::CONFLICT
        } else if lower.contains("unauthorized") {
            StatusCode::UNAUTHORIZED
        } else if lower.contains("invalid")
            || lower.contains("unsupported")
            || lower.contains("would create a cycle")
        {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        (status, Json(serde_json::json!({"error": msg}))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self { ApiError(err.into()) }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

async fn check_auth(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    if let Some(ref token) = state.token {
        let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer "));
        match auth {
            Some(t) if t == token => Ok(()),
            _ => Err(ApiError(anyhow::anyhow!("unauthorized"))),
        }
    } else { Ok(()) }
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

pub fn router(db: Arc<Database<'static>>, token: Option<String>) -> Router {
    let state = AppState { db, token };
    Router::new()
        .route("/api/feeds", get(list_feeds).post(add_feed))
        .route("/api/feeds/{id}", delete(remove_feed))
        .route("/api/feeds/{id}/move", put(move_feed))
        .route("/api/sync", post(sync_feeds))
        .route("/api/status", get(get_status))
        .route("/api/items", get(list_items))
        .route("/api/items/{id}", get(read_item))
        .route("/api/items/{id}/mark", put(mark_item))
        .route("/api/search", get(search_items))
        .route("/api/starred", get(list_starred))
        .route("/api/export/{id}", get(export_item))
        .route("/api/digest", get(get_digest))
        .route("/api/trending", get(get_trending))
        .route("/api/match", get(match_keywords))
        .route("/api/stats", get(get_stats))
        .route("/api/folders", get(list_folders).post(create_folder))
        .route("/api/folders/{id}", delete(delete_folder))
        .route("/api/folders/{id}/rename", put(rename_folder))
        .route("/api/folders/{id}/move", put(move_folder))
        .route("/api/opml", get(get_opml).post(post_opml))
        .route("/api/snapshot", get(get_snapshot))
        .route("/api/boards", get(list_boards).post(create_board))
        .route("/api/boards/{id}", delete(delete_board))
        .route("/api/boards/{id}/items", post(add_board_item))
        .route("/api/watches", get(list_watches).post(create_watch))
        .route("/api/watches/{id}", delete(delete_watch))
        .route("/api/items/{id}/read_later", put(toggle_read_later))
        .route("/api/items/{id}/full", post(fetch_full))
        .route("/api/mark_all_read", post(mark_all_read))
        .with_state(state)
}

pub async fn serve(db: Arc<Database<'static>>, bind: String, token: Option<String>) -> anyhow::Result<()> {
    let app = router(db, token);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    eprintln!("rssdude server listening on {bind}");
    let _guard = crate::shared::runtime::write(&bind)?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

// ---------------------------------------------------------------------------
// Thin handlers — delegate to commands::*_core functions
// ---------------------------------------------------------------------------

// --- Feeds ---

#[derive(Deserialize)]
struct AddFeedReq { url: String, #[serde(default)] tags: Vec<String>, folder: Option<String> }

async fn add_feed(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<AddFeedReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (feed, count) = commands::feed_mgmt::add_core(s.db, req.url, req.tags, req.folder).await?;
    Ok(Json(serde_json::json!({"feed": FeedJson::from(&feed), "items_synced": count})))
}

#[derive(Deserialize)]
struct ListFeedsQuery { tag: Option<String> }

async fn list_feeds(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<ListFeedsQuery>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let feeds = commands::feed_mgmt::list_core(s.db, q.tag).await?;
    Ok(Json(serde_json::json!(feeds)))
}

async fn remove_feed(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (title, count) = commands::feed_mgmt::remove_core(s.db, id).await?;
    Ok(Json(serde_json::json!({"title": title, "items_deleted": count})))
}

#[derive(Deserialize)]
struct MoveFeedReq { folder_id: Option<String> }

async fn move_feed(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Json(req): Json<MoveFeedReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (ft, fn_) = commands::feed_mgmt::move_to_folder_core(s.db, id, req.folder_id).await?;
    Ok(Json(serde_json::json!({"feed_title": ft, "folder_name": fn_})))
}

// --- Sync ---

#[derive(Deserialize)]
struct SyncReq { feed: Option<String> }

async fn sync_feeds(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<SyncReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::sync::sync_core(s.db, req.feed, None).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn get_status(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::sync::status_core(s.db).await?;
    Ok(Json(serde_json::json!(result)))
}

// --- Items ---

#[derive(Deserialize)]
struct ItemsQueryParams {
    #[serde(default = "default_limit")] limit: usize,
    since: Option<String>, tag: Option<String>,
    #[serde(default)] unread: bool,
    feed: Option<String>, folder: Option<String>,
}
fn default_limit() -> usize { 20 }

async fn list_items(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<ItemsQueryParams>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let results = commands::read::items_core(s.db, ItemsQuery {
        limit: q.limit, since: q.since, tag: q.tag, unread: q.unread,
        feed_id: q.feed, folder_id: q.folder,
    }).await?;
    Ok(Json(serde_json::json!(results)))
}

async fn read_item(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::read::read_item_core(s.db, id).await?;
    Ok(Json(serde_json::json!(result)))
}

#[derive(Deserialize)]
struct SearchQueryParams { q: String, #[serde(default = "default_search_limit")] limit: usize }
fn default_search_limit() -> usize { 10 }

async fn search_items(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<SearchQueryParams>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let results = commands::read::search_core(s.db, q.q, q.limit).await?;
    Ok(Json(serde_json::json!(results)))
}

// --- Curation ---

#[derive(Deserialize)]
struct MarkReq { #[serde(default)] read: bool, #[serde(default)] star: bool, note: Option<String> }

async fn mark_item(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Json(req): Json<MarkReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (item, mark) = commands::curate::mark_core(s.db, id, req.read.then_some(true), req.star.then_some(true), req.note).await?;
    Ok(Json(serde_json::json!({"item_title": item.title, "mark": mark})))
}

#[derive(Deserialize)]
struct OptionalLimitQuery { limit: Option<usize> }

async fn list_starred(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<OptionalLimitQuery>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let results = commands::curate::starred_core(s.db, q.limit).await?;
    Ok(Json(serde_json::json!(results)))
}

async fn export_item(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::curate::export_core(s.db, id).await?;
    Ok(Json(serde_json::json!(result)))
}

// --- Discovery ---

#[derive(Deserialize)]
struct DigestQueryParams { #[serde(default = "default_since")] since: String, tag: Option<String> }
fn default_since() -> String { "24h".to_string() }

async fn get_digest(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<DigestQueryParams>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::discover::digest_core(s.db, q.since, q.tag).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn get_trending(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::discover::trending_core(s.db).await?;
    Ok(Json(serde_json::json!(result)))
}

#[derive(Deserialize)]
struct MatchQueryParams { keywords: String, #[serde(default = "default_limit")] limit: usize, #[serde(default = "default_match_since")] since: String }
fn default_match_since() -> String { "7d".to_string() }

async fn match_keywords(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<MatchQueryParams>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let results = commands::discover::match_keywords_core(s.db, q.keywords, q.limit, q.since).await?;
    Ok(Json(serde_json::json!(results)))
}

// --- Stats ---

#[derive(Deserialize)]
struct StatsQueryParams {
    #[serde(default = "default_stats_since")]
    since: String,
    dead: Option<f32>,
}
fn default_stats_since() -> String { "30d".to_string() }

async fn get_stats(State(s): State<AppState>, headers: axum::http::HeaderMap, Query(q): Query<StatsQueryParams>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::stats::stats_core(s.db, &q.since, q.dead).await?;
    Ok(Json(serde_json::json!(result)))
}

// --- Folders ---

#[derive(Deserialize)]
struct CreateFolderReq { name: String, parent_id: Option<String> }

async fn create_folder(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<CreateFolderReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let folder = commands::folder::create_core(s.db, req.name, req.parent_id).await?;
    Ok(Json(serde_json::json!(FolderJson::from(&folder))))
}

async fn list_folders(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::folder::list_core(s.db).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct RenameFolderReq { name: String }

async fn rename_folder(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Json(req): Json<RenameFolderReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let folder = commands::folder::rename_core(s.db, id, req.name).await?;
    Ok(Json(serde_json::json!(FolderJson::from(&folder))))
}

#[derive(Deserialize)]
struct MoveFolderReq { parent_id: String }

async fn move_folder(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Json(req): Json<MoveFolderReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let folder = commands::folder::move_folder_core(s.db, id, Some(req.parent_id)).await?;
    Ok(Json(serde_json::json!(FolderJson::from(&folder))))
}

#[derive(Deserialize)]
struct DeleteFolderQuery { #[serde(default)] recursive: bool }

async fn delete_folder(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>, Query(q): Query<DeleteFolderQuery>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::folder::delete_core(s.db, id, q.recursive).await?;
    Ok(Json(result))
}

// --- OPML ---

#[derive(Deserialize)]
struct OpmlImportReq { content: String }

async fn post_opml(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<OpmlImportReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let result = commands::opml::import_core(s.db, req.content).await?;
    Ok(Json(serde_json::json!(result)))
}

async fn get_opml(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let xml = commands::opml::export_core(s.db).await?;
    Ok(Json(serde_json::json!({"opml": xml})))
}

// --- Snapshot (TUI bulk fetch) ---

async fn get_snapshot(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<commands::snapshot::Snapshot> {
    check_auth(&s, &headers).await?;
    Ok(Json(commands::snapshot::snapshot_core(s.db).await?))
}

// --- Boards ---

#[derive(Deserialize)]
struct CreateBoardReq { name: String }

async fn list_boards(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (boards, items) = commands::board::list_core(s.db).await?;
    Ok(Json(serde_json::json!({"boards": boards, "items": items})))
}

async fn create_board(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<CreateBoardReq>) -> ApiResult<Board> {
    check_auth(&s, &headers).await?;
    Ok(Json(commands::board::create_core(s.db, req.name).await?))
}

async fn delete_board(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let name = commands::board::delete_core(s.db, id).await?;
    Ok(Json(serde_json::json!({"name": name})))
}

#[derive(Deserialize)]
struct AddBoardItemReq { item_id: String, note: Option<String> }

async fn add_board_item(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(board_id): axum::extract::Path<String>, Json(req): Json<AddBoardItemReq>) -> ApiResult<BoardItem> {
    check_auth(&s, &headers).await?;
    Ok(Json(commands::board::add_item_core(s.db, board_id, req.item_id, req.note).await?))
}

// --- Watches ---

#[derive(Deserialize)]
struct CreateWatchReq { name: String, query: String }

async fn list_watches(State(s): State<AppState>, headers: axum::http::HeaderMap) -> ApiResult<Vec<SavedSearch>> {
    check_auth(&s, &headers).await?;
    Ok(Json(commands::watch::list_core(s.db).await?))
}

async fn create_watch(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<CreateWatchReq>) -> ApiResult<SavedSearch> {
    check_auth(&s, &headers).await?;
    Ok(Json(commands::watch::create_core(s.db, req.name, req.query).await?))
}

async fn delete_watch(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let name = commands::watch::delete_core(s.db, id).await?;
    Ok(Json(serde_json::json!({"name": name})))
}

// --- Per-item TUI ops ---

async fn toggle_read_later(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let (item, now_later) = commands::curate::toggle_read_later_core(s.db, id).await?;
    Ok(Json(serde_json::json!({"item_title": item.title, "read_later": now_later})))
}

async fn fetch_full(State(s): State<AppState>, headers: axum::http::HeaderMap, axum::extract::Path(id): axum::extract::Path<String>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let text = commands::curate::fetch_full_article_core(s.db, id).await?;
    Ok(Json(serde_json::json!({"length": text.len()})))
}

#[derive(Deserialize)]
struct MarkAllReq { scope: String, scope_id: Option<String> }

async fn mark_all_read(State(s): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<MarkAllReq>) -> ApiResult<serde_json::Value> {
    check_auth(&s, &headers).await?;
    let count = commands::curate::mark_all_read_core(s.db, req.scope, req.scope_id).await?;
    Ok(Json(serde_json::json!({"count": count})))
}
