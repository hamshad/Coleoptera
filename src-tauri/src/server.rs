use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt as _;

use crate::crawler::{crawl, CrawlRequest};
use crate::state::AppState;

type SharedState = Arc<AppState>;

/// Build the Axum router. The Tauri webview loads from this same origin.
pub fn router(state: SharedState) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    axum::Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/health", get(health))
        .route("/crawl/stream", post(crawl_stream))
        .route("/crawl/cancel", post(cancel_crawl))
        .layer(cors)
        .with_state(state)
}

/// Serve the bundled single-file UI. The HTML is embedded into the binary at
/// compile time (see `INDEX_HTML`), so the app is fully self-contained — no
/// external asset files needed at runtime.
const INDEX_HTML: &str = include_str!("../../dist/index.html");

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML.to_string())
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

fn new_session_id() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

async fn crawl_stream(State(state): State<SharedState>, Json(req): Json<CrawlRequest>) -> Response {
    if req.url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "No URL provided" })),
        )
            .into_response();
    }

    let session_id = new_session_id();
    let cancel_tx = state.register(session_id.clone());
    let cancel_rx = {
        let mut rx = cancel_tx.subscribe();
        let _ = rx.borrow_and_update();
        rx
    };

    let (tx, rx) = mpsc::channel::<String>(64);
    let state_for_finish = state.clone();
    let sid = session_id.clone();

    // Run the blocking crawler on a dedicated thread so we never block the
    // async runtime.
    tokio::task::spawn_blocking(move || {
        crawl(req, sid.clone(), tx, cancel_rx);
        state_for_finish.finish(&sid);
    });

    let stream = ReceiverStream::new(rx).map(|line| Ok::<_, std::convert::Infallible>(Event::default().data(line)));
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response()
}

async fn cancel_crawl(
    State(state): State<SharedState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let id = req.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
    let ok = state.cancel(id);
    Json(json!({ "success": ok }))
}
