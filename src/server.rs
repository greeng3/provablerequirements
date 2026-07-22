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
    routing::{get, post},
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
        .route("/api/requirements/{id}/triage", post(set_triage))
        .route("/api/requirements/{id}/verify", post(verify_requirement))
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

/// The body of a triage write: the bucket to set the item to.
#[derive(serde::Deserialize)]
struct TriageRequest {
    classification: String,
}

/// POST /api/requirements/:id/triage — set one item's triage bucket (REQ037).
///
/// The first operator write from the UI. Triage is companion state (A6 "the tool writes freely"),
/// so this writes `triage.yml` directly — no working-tree gate, unlike a source-code proof carrier.
/// Returns the updated backlog so the client reconciles against authoritative state (correct
/// coverage), not just its optimistic patch. Bad bucket → 400, unknown id → 404, unadopted → 409.
///
/// Implements: REQ037
async fn set_triage(
    State(subject): State<Subject>,
    Path(id): Path<String>,
    Json(req): Json<TriageRequest>,
) -> Response {
    let Some(classification) = crate::source::Classification::parse(&req.classification) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "unknown bucket '{}' (formalizable-now | falsifiable-only | stays-prose)",
                    req.classification
                )
            })),
        )
            .into_response();
    };
    match apply_triage(&subject, &id, classification) {
        Ok(Some(backlog)) => Json(backlog).into_response(),
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

/// Set the item's bucket and re-read the backlog, or `Ok(None)` when the id is unknown.
fn apply_triage(
    subject: &std::path::Path,
    id: &str,
    classification: crate::source::Classification,
) -> anyhow::Result<Option<Backlog>> {
    let (companion, items) = crate::adopt::resolve(subject)?;
    let Some(item) = items.iter().find(|i| i.id == id) else {
        return Ok(None);
    };
    let state = crate::triage::load(&companion)?;
    let next = crate::triage::set(&state, item, classification);
    crate::triage::save(&companion, &next)?;
    let drafts = crate::draft::load(&companion)?;
    Ok(Some(Backlog {
        coverage: crate::status::coverage(&items, &next, &drafts),
        items: crate::status::backlog(&items, &next, &drafts),
    }))
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
    let draft = drafts.drafts.get(id);
    let base = crate::detail::build(item, classification, draft);
    // Live D13 grounding dry-run: only meaningful when the candidate gates and has bindings.
    let grounding = draft.and_then(|d| grounding_report(subject, &companion, d));
    Ok(Some(crate::detail::Detail { grounding, ..base }))
}

/// Run the live grounding dry-run for a draft, or `None` when there is nothing to resolve (no
/// candidate, the candidate no longer gates, or no bindings attached yet).
fn grounding_report(
    subject: &std::path::Path,
    companion: &std::path::Path,
    draft: &crate::draft::Draft,
) -> Option<crate::detail::GroundingReport> {
    let candidate = draft.candidate.as_deref()?;
    if draft.bindings.is_empty() {
        return None;
    }
    let requirement = crate::prl::gate(candidate).ok()?.requirement;
    let (by_symbol, by_sort, by_model) =
        crate::grounding::resolve_bindings(subject, companion, &requirement, &draft.bindings);
    Some(crate::detail::grounding_report(
        &requirement,
        &draft.bindings,
        &by_symbol,
        &by_sort,
        &by_model,
    ))
}

/// POST /api/requirements/:id/verify — run the engine ensemble on demand (REQ038).
///
/// The heaviest surface: it re-gates the admitted candidate, re-runs the live grounding dry-run,
/// and — only when grounded — runs the wired engines, returning the aggregate verdict plus each
/// engine's own evidence (D2b). **Synchronous**: a loopback single-operator tool blocks for the
/// run rather than standing up a job queue. Unknown id → 404, unadopted → 409 (same split the
/// detail read uses). A not-yet-verifiable state (undrafted / unadmitted / no candidate / the
/// candidate no longer gates) is an honest 200 payload the operator can act on, never an error.
///
/// Implements: REQ038
async fn verify_requirement(State(subject): State<Subject>, Path(id): Path<String>) -> Response {
    match crate::verify::verify(&subject, &id) {
        Ok(Some(outcome)) => Json(verify_payload(&outcome)).into_response(),
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

/// Map a [`crate::verify::VerifyOutcome`] to its JSON wire shape. A `state` tag discriminates the
/// verdict from each honest not-yet-verifiable state, so the client renders one branch per state
/// rather than guessing from missing fields. The verdict itself is the [`crate::verdict::report`]
/// shape; the CLI-only grounding context (`grounded`/`resolutions`) is intentionally dropped.
fn verify_payload(outcome: &crate::verify::VerifyOutcome) -> serde_json::Value {
    use crate::verify::VerifyOutcome as O;
    match outcome {
        O::NoDraft => serde_json::json!({ "state": "no-draft" }),
        O::NotAdmitted => serde_json::json!({ "state": "not-admitted" }),
        O::NoCandidate => serde_json::json!({ "state": "no-candidate" }),
        O::GateFailed { errors } => {
            serde_json::json!({ "state": "gate-failed", "errors": errors })
        }
        O::Verdict { verdict, stale, .. } => serde_json::json!({
            "state": "verdict",
            "stale": stale,
            "verdict": crate::verdict::report(verdict),
        }),
    }
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

    async fn post_json(path: &str, subject: PathBuf, body: serde_json::Value) -> Response {
        router(subject)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    // Verifies: REQ037 — a triage write sets the bucket, persists it, and returns the updated
    // backlog (coverage reflects the new classification).
    #[tokio::test]
    async fn triage_write_sets_the_bucket_and_returns_updated_coverage() {
        let subject = adopted_subject_with_one_item();
        let res = post_json(
            "/api/requirements/REQ001/triage",
            subject.path().to_path_buf(),
            serde_json::json!({ "classification": "formalizable-now" }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["coverage"]["formalizable_now"], 1);
        assert_eq!(body["coverage"]["untriaged"], 0);
        assert_eq!(body["items"][0]["classification"], "formalizable-now");

        // The write persisted: a fresh GET reflects it.
        let got = get_path_on("/api/requirements", subject.path().to_path_buf()).await;
        let bytes = to_bytes(got.into_body(), usize::MAX).await.unwrap();
        let backlog: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(backlog["items"][0]["classification"], "formalizable-now");
    }

    // Verifies: REQ037 — an unknown bucket is a 400, never silently written.
    #[tokio::test]
    async fn triage_write_rejects_an_unknown_bucket() {
        let subject = adopted_subject_with_one_item();
        let res = post_json(
            "/api/requirements/REQ001/triage",
            subject.path().to_path_buf(),
            serde_json::json!({ "classification": "nonsense" }),
        )
        .await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    async fn post_empty(path: &str, subject: PathBuf) -> Response {
        router(subject)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    // Verifies: REQ038 — verifying an unknown id under an adopted subject is a 404, distinct from
    // the unadopted-subject 409.
    #[tokio::test]
    async fn verify_unknown_id_is_not_found() {
        let subject = adopted_subject_with_one_item();
        let res = post_empty(
            "/api/requirements/REQ999/verify",
            subject.path().to_path_buf(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    // Verifies: REQ038 — verifying against an unadopted subject is a 409, never a 500.
    #[tokio::test]
    async fn verify_unadopted_subject_is_conflict() {
        let empty = tempfile::tempdir().unwrap();
        let res = post_empty(
            "/api/requirements/REQ001/verify",
            empty.path().to_path_buf(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    // Verifies: REQ038 — verifying an item with no draft is an honest 200 naming the "no-draft"
    // state (nothing to run), never a fabricated verdict and never an error.
    #[tokio::test]
    async fn verify_undrafted_item_is_honest_no_draft_state() {
        let subject = adopted_subject_with_one_item();
        let res = post_empty(
            "/api/requirements/REQ001/verify",
            subject.path().to_path_buf(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["state"], "no-draft");
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
