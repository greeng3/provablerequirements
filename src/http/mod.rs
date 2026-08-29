//! HTTP surface: router construction and per-endpoint handlers.

pub mod dto;
pub mod handlers;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tower::util::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::app::AppState;

use self::dto::ErrorResponse;

/// Build the full axum Router with the given shared state and an
/// optional path to a built frontend bundle.
///
/// When `static_root` is `Some(root)` and `root/index.html` exists,
/// the Router mounts a fallback that serves static files and falls
/// back to `index.html` for unmatched paths so client-side routes
/// like `/projects/sample` are served the SPA shell.
///
/// `/api/*` paths that don't match an explicit handler are
/// intercepted before reaching the static service and receive a
/// proper JSON 404 instead of the SPA shell.
pub fn build_router(state: Arc<AppState>, static_root: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/readyz", get(handlers::readyz))
        .route("/api/mounts", get(handlers::list_mounts))
        .route("/api/system", get(handlers::get_system))
        .route(
            "/api/mounts/{dir_name}/init",
            axum::routing::post(handlers::init_project),
        )
        .route("/api/projects", get(handlers::list_projects))
        .route("/api/projects/{slug}", get(handlers::get_project))
        .route(
            "/api/projects/{slug}/artifacts",
            axum::routing::delete(handlers::wipe_project_artifacts),
        )
        .route(
            "/api/projects/{slug}/collections",
            get(handlers::list_collections).post(handlers::create_collection),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}",
            get(handlers::get_collection).delete(handlers::delete_collection),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}/artifacts",
            get(handlers::list_artifacts).post(handlers::create_artifact),
        )
        .route(
            "/api/artifacts/{uuid}",
            get(handlers::get_artifact)
                .put(handlers::update_artifact)
                .patch(handlers::rename_artifact)
                .delete(handlers::delete_artifact),
        )
        .route(
            "/api/artifacts/{uuid}/incoming-links",
            get(handlers::list_incoming_links),
        )
        .route(
            "/api/artifacts/{uuid}/reviews",
            axum::routing::post(handlers::create_review),
        )
        .route(
            "/api/artifacts/{uuid}/reviews/last-approval-snapshot",
            get(handlers::last_approval_snapshot),
        )
        .route("/api/artifacts/search", get(handlers::search_artifacts))
        .route("/api/link-types", get(handlers::list_link_types))
        .route("/api/reviewers", get(handlers::list_reviewers))
        .route("/api/reviews/queue", get(handlers::review_queue))
        .route(
            "/api/artifacts/{uuid}/blob",
            get(handlers::download_blob).put(handlers::replace_blob),
        )
        .route(
            "/api/artifacts/{uuid}/thumbnail",
            get(handlers::get_thumbnail),
        )
        .route(
            "/api/artifacts/{uuid}/check-url",
            axum::routing::post(handlers::check_url),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}/artifacts/blob",
            axum::routing::post(handlers::create_blob_artifact),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}/artifacts/blob/adopt",
            axum::routing::post(handlers::adopt_orphan_blob),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}/artifacts/url",
            axum::routing::post(handlers::create_url_artifact),
        )
        .route(
            "/api/projects/{slug}/collections/{prefix}/check-urls",
            axum::routing::post(handlers::bulk_check_urls),
        )
        .route("/api/events", get(handlers::events))
        .route("/api/artifacts/{uuid}/history", get(handlers::get_history))
        .route("/api/artifacts/{uuid}/diff", get(handlers::get_diff))
        .route("/api/reports/{kind}", get(handlers::run_report))
        .route(
            "/api/reports/{kind}/config",
            get(handlers::read_report_config)
                .put(handlers::write_report_config)
                .delete(handlers::clear_report_config),
        )
        .route(
            "/api/reports/{kind}/export/{ext}",
            get(handlers::export_report),
        )
        .route("/api/graph", get(handlers::get_graph))
        .route("/api/matrix", get(handlers::get_matrix))
        .route("/api/search", get(handlers::search))
        .route("/api/browse", get(handlers::browse))
        .route("/api/projects/{slug}/code-scan", get(handlers::code_scan))
        .route(
            "/api/projects/{slug}/doorstop/preview",
            axum::routing::post(handlers::doorstop_preview),
        )
        .route(
            "/api/projects/{slug}/doorstop/import",
            axum::routing::post(handlers::doorstop_import),
        )
        .route(
            "/api/projects/{slug}/doorstop/report",
            get(handlers::doorstop_report),
        )
        .route(
            "/api/projects/{slug}/doorstop/report/export/{ext}",
            get(handlers::doorstop_report_export),
        )
        .route(
            "/api/llm/providers",
            get(handlers::list_llm_providers).post(handlers::add_llm_provider),
        )
        .route(
            "/api/llm/providers/{index}",
            axum::routing::put(handlers::replace_llm_provider)
                .delete(handlers::delete_llm_provider)
                .patch(handlers::patch_llm_provider),
        )
        .route(
            "/api/llm/providers/{index}/retest",
            axum::routing::post(handlers::retest_llm_provider),
        )
        .route(
            "/api/llm/providers/{index}/acknowledge-privacy",
            axum::routing::post(handlers::acknowledge_llm_privacy),
        )
        .route(
            "/api/llm/prompt",
            axum::routing::post(handlers::debug_llm_prompt),
        )
        .route(
            "/api/artifacts/{uuid}/rename-suggestions",
            axum::routing::post(handlers::rename_suggestions),
        )
        .route(
            "/api/projects/{slug}/rename-suggestions/bulk",
            axum::routing::post(handlers::bulk_rename_suggestions),
        )
        .route(
            "/api/projects/{slug}/migrate-schema",
            axum::routing::post(handlers::migrate_project_schema),
        )
        .route(
            "/api/projects/{slug}/sample-content",
            axum::routing::post(handlers::create_sample_content),
        )
        .route(
            "/api/projects/{slug}/suggestions/links/analyze",
            axum::routing::post(handlers::analyze_link_suggestions),
        )
        .route(
            "/api/projects/{slug}/suggestions/links",
            get(handlers::list_pending_link_suggestions),
        )
        .route(
            "/api/projects/{slug}/suggestions/links/declined",
            get(handlers::list_declined_link_suggestions),
        )
        .route(
            "/api/projects/{slug}/suggestions/links/{id}/accept",
            axum::routing::post(handlers::accept_link_suggestion),
        )
        .route(
            "/api/projects/{slug}/suggestions/links/{id}/reject",
            axum::routing::post(handlers::reject_link_suggestion),
        )
        .route(
            "/api/projects/{slug}/suggestions/links/{id}/reinstate",
            axum::routing::post(handlers::reinstate_link_suggestion),
        );

    // Phase 5b body-limit: multipart uploads are capped at
    // max_blob_bytes + a small envelope for the multipart framing
    // fields. axum's DefaultBodyLimit applies across the whole
    // router; the in-handler check catches the precise per-part
    // size for a tighter 413 message.
    let body_limit = state
        .config()
        .max_blob_bytes
        .saturating_add(1024 * 1024)
        .try_into()
        .unwrap_or(usize::MAX);
    router = router.layer(DefaultBodyLimit::max(body_limit));

    if let Some(root) = resolve_static_root(static_root) {
        router = router.fallback(move |uri: Uri, request: Request| {
            let root = root.clone();
            async move { frontend_fallback(uri, request, root).await }
        });
    }

    router
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Returns `Some(root)` only when the path exists and contains an
/// `index.html` — otherwise the server runs without static serving,
/// which is the normal dev-machine case.
fn resolve_static_root(candidate: Option<PathBuf>) -> Option<PathBuf> {
    let root = candidate?;
    if root.is_dir() && root.join("index.html").is_file() {
        Some(root)
    } else {
        tracing::warn!(
            path = %root.display(),
            "static-root does not contain index.html; frontend will not be served"
        );
        None
    }
}

/// The fallback handler mounted when a frontend bundle is present.
/// Unknown `/api/*` paths still receive a JSON 404; everything else
/// goes through ServeDir with an `index.html` SPA fallback.
async fn frontend_fallback(uri: Uri, request: Request, root: PathBuf) -> Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("unknown api endpoint: {}", uri.path()),
            }),
        )
            .into_response();
    }

    let index = root.join("index.html");
    let service = ServeDir::new(root).fallback(ServeFile::new(index));

    match service.oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("static file service error: {err}"),
        )
            .into_response(),
    }
}
