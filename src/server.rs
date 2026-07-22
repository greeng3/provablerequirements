//! HTTP server: the local backend that serves the embedded web UI.
//!
//! The router is built separately from the network bind so handlers are
//! testable via `tower::ServiceExt::oneshot` without opening a socket.
//!
//! Implements: REQ005 (the serve command exposes GET /health as JSON)
//! Implements: REQ006 (embedded UI served locally with SPA fallback to index.html)
//! Implements: REQ034 (GET /api/requirements — the read-only backlog + coverage surface)

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

/// The built frontend (`web/dist`), baked into the binary at compile time.
/// In debug builds rust-embed reads these files from disk at runtime; release
/// builds embed them so the executable is self-contained (REQ006).
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

/// The subject repository this server browses. `serve` runs co-resident in the operator's dev
/// env, so the subject is a path it is pointed at — shared by every request as router state.
type Subject = Arc<PathBuf>;

/// Build the application router for `subject`.
pub fn router(subject: PathBuf) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/requirements", get(requirements))
        .route("/api/requirements/{id}", get(requirement_detail))
        .fallback(static_asset)
        .with_state(Arc::new(subject))
}

/// Bind to the loopback interface on `port` and serve `subject` until the process stops.
///
/// Implements: REQ005 (local server)
pub async fn serve(port: u16, subject: PathBuf) -> std::io::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("provreq serving {} on http://{addr}", subject.display());
    axum::serve(listener, router(subject)).await
}

/// GET /health — the health payload as JSON.
///
/// Implements: REQ005
async fn health() -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        crate::health_json(),
    )
        .into_response()
}

/// The read-only backlog surface: the coverage funnel plus every item's triage + formalization
/// state. A passive listing — it reads persisted companion state only and runs no engine.
#[derive(serde::Serialize)]
struct Backlog {
    coverage: crate::status::Coverage,
    items: Vec<crate::status::ItemState>,
}

/// GET /api/requirements — the backlog + coverage for the served subject (REQ034).
///
/// Resolves the companion tree and reads triage + draft state fresh on each request: this is a
/// local single-operator tool, so a per-request read is simpler than a cache and always current.
/// A subject that has not been adopted yet is an honest 409 naming `init`, not an empty list.
///
/// Implements: REQ034
async fn requirements(State(subject): State<Subject>) -> Response {
    match load_backlog(&subject) {
        Ok(backlog) => Json(backlog).into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Assemble the backlog from persisted companion state. Split out so the error path stays a
/// single `?`-chain; the handler only maps ok/err to a response.
fn load_backlog(subject: &std::path::Path) -> anyhow::Result<Backlog> {
    let (companion, items) = crate::adopt::resolve(subject)?;
    let triage = crate::triage::load(&companion)?;
    let drafts = crate::draft::load(&companion)?;
    Ok(Backlog {
        coverage: crate::status::coverage(&items, &triage, &drafts),
        items: crate::status::backlog(&items, &triage, &drafts),
    })
}

/// GET /api/requirements/:id — one item's read-only formalization detail (REQ035).
///
/// Reads persisted companion state fresh, like [`requirements`]. An unadopted subject is a 409
/// (same as the list); an unknown id under an adopted subject is a 404 — the two are distinct
/// operator conditions and must not collapse.
///
/// Implements: REQ035
async fn requirement_detail(State(subject): State<Subject>, Path(id): Path<String>) -> Response {
    match load_detail(&subject, &id) {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no requirement '{id}' in the subject") })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Assemble one item's detail, or `Ok(None)` when the id is not in the subject.
fn load_detail(
    subject: &std::path::Path,
    id: &str,
) -> anyhow::Result<Option<crate::detail::Detail>> {
    let (companion, items) = crate::adopt::resolve(subject)?;
    let Some(item) = items.iter().find(|i| i.id == id) else {
        return Ok(None);
    };
    let triage = crate::triage::load(&companion)?;
    let drafts = crate::draft::load(&companion)?;
    let classification = triage.items.get(id).map(|e| e.classification);
    Ok(Some(crate::detail::build(
        item,
        classification,
        drafts.drafts.get(id),
    )))
}

/// Serve an embedded asset by request path, falling back to `index.html` for
/// paths with no embedded file so the single-page app can route them.
///
/// Implements: REQ006
async fn static_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(file) => embedded_response(path, file.data.into_owned()),
        // ponytail: any unmatched path returns index.html (standard SPA fallback);
        // fine until there is a real 404 surface to distinguish.
        None => match WebAssets::get("index.html") {
            Some(index) => embedded_response("index.html", index.data.into_owned()),
            None => (StatusCode::NOT_FOUND, "web UI not built").into_response(),
        },
    }
}

fn embedded_response(path: &str, bytes: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    ([(header::CONTENT_TYPE, mime.as_ref())], Body::from(bytes)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn get_path(path: &str) -> Response {
        get_path_on(path, PathBuf::from(".")).await
    }

    async fn get_path_on(path: &str, subject: PathBuf) -> Response {
        router(subject)
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    fn content_type(res: &Response) -> String {
        res.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    }

    // Verifies: REQ005
    #[tokio::test]
    async fn health_route_returns_ok_json() {
        let res = get_path("/health").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(content_type(&res), "application/json");
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("\"status\":\"ok\""), "body: {body}");
    }

    // Verifies: REQ006
    #[tokio::test]
    async fn root_serves_embedded_index_html() {
        let res = get_path("/").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(content_type(&res).starts_with("text/html"), "{res:?}");
    }

    // Verifies: REQ006 — an unknown client-side path falls back to index.html.
    #[tokio::test]
    async fn unknown_path_falls_back_to_index() {
        let res = get_path("/some/spa/route").await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(content_type(&res).starts_with("text/html"), "{res:?}");
    }

    // Verifies: REQ034 — a subject that has not been adopted is an honest 409 that names `init`,
    // never an empty listing that would read as "no requirements".
    #[tokio::test]
    async fn requirements_on_unadopted_subject_is_conflict() {
        let empty = tempfile::tempdir().unwrap();
        let res = get_path_on("/api/requirements", empty.path().to_path_buf()).await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("init"), "error should name init: {body}");
    }

    // Verifies: REQ034 — an adopted subject returns the coverage funnel and one row per item.
    #[tokio::test]
    async fn requirements_lists_items_with_coverage() {
        let subject = adopted_subject_with_one_item();
        let res = get_path_on("/api/requirements", subject.path().to_path_buf()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(content_type(&res), "application/json");
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["coverage"]["discovered"], 1);
        assert_eq!(body["items"][0]["id"], "REQ001");
        // Untriaged + undrafted item reports honest "null"/"none", not a guessed state.
        assert!(body["items"][0]["classification"].is_null());
        assert_eq!(body["items"][0]["formalization"], "none");
    }

    // Verifies: REQ035 — an unknown id under an adopted subject is a 404, distinct from the
    // unadopted-subject 409.
    #[tokio::test]
    async fn detail_for_unknown_id_is_not_found() {
        let subject = adopted_subject_with_one_item();
        let res = get_path_on("/api/requirements/REQ999", subject.path().to_path_buf()).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    // Verifies: REQ035 — a known item returns its detail (identity + prose + honest unformalized
    // state for an item with no draft).
    #[tokio::test]
    async fn detail_for_known_id_returns_the_item() {
        let subject = adopted_subject_with_one_item();
        let res = get_path_on("/api/requirements/REQ001", subject.path().to_path_buf()).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(content_type(&res), "application/json");
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["id"], "REQ001");
        assert_eq!(body["formalization"], "none");
        assert!(body["candidate"].is_null());
    }

    /// A minimal adopted subject: a Doorstop document with one item plus the `provreq.yml`
    /// companion manifest that `adopt::resolve` looks for.
    fn adopted_subject_with_one_item() -> tempfile::TempDir {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join(".doorstop.yml"),
            "settings:\n  prefix: REQ\n  digits: 3\n",
        )
        .unwrap();
        fs::write(
            root.join("REQ001.yml"),
            "active: true\nlevel: 1.0\nnormative: true\nref: ''\nreviewed: null\ntext: |\n  A requirement.\n",
        )
        .unwrap();
        fs::write(
            root.join(crate::adopt::MANIFEST_FILE),
            "subject_requirements_root: .\n",
        )
        .unwrap();
        dir
    }
}
