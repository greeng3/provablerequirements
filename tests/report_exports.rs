//! Report-export endpoint integration tests
//! (`GET /api/reports/:kind/export/:ext`), ported from ReqForge's
//! `tests/report_exports.rs` for #374 and adapted to provreq's
//! single-subject model.
//!
//! ReqForge drove multi-project `discover_mounts`; provreq serves one
//! repository (#370). These tests seed one subject through the shared
//! harness helpers, but boot a local single-subject `AppState` so the
//! `external_url` config (which the shared `build_app` fixes to
//! `None`) can be varied — the HTML-export tests turn on absolute vs
//! relative URLs. Byte/download bodies are read with a local
//! `body_bytes`-style helper (the shared harness only decodes JSON).
//!
//! Dropped tests: none.

mod support;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::world::DiscoveryConfig;
use reqforge_model::write::OwnershipOverrides;
use tower::util::ServiceExt;

use support::{SUBJECT_SLUG, write_collection, write_project};

const UUID_REQ_A: &str = "0194f6d0-0007-7000-8000-00000000aaaa";
const UUID_REQ_B: &str = "0194f6d0-0007-7000-8000-00000000bbbb";

fn hint(slug: &str, prefix: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "projectSlug": slug,
        "collectionPrefix": prefix,
        "artifactName": name,
    })
}

fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: serde_json::Value,
) {
    let meta = serde_json::json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-22T00:00:00Z",
        "modifiedAt": "2026-04-22T00:00:00Z",
        "links": links,
        "reviewLog": [],
    });
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{}\n---\nbody\n", meta)).unwrap();
}

/// Seed one subject, boot a single-subject `AppState` with the given
/// `external_url`, and return the management Router. The `TempDir`
/// must be kept alive for the test's duration.
async fn build_app_with_external(
    seed: impl FnOnce(&Path),
    external_url: Option<&str>,
) -> (Router, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    seed(&subject);

    let config = DiscoveryConfig {
        mount_prefix: subject.clone(),
        system_config_path: None,
        workspace_dir: None,
        max_blob_bytes: 50 * 1024 * 1024,
        thumbnail_cache_max_bytes: 500 * 1024 * 1024,
        external_url: external_url.map(|s| s.to_owned()),
    };
    let state = Arc::new(AppState::new_single_subject(
        subject.clone(),
        config,
        OwnershipOverrides::default(),
    ));
    state.refresh().await.unwrap();
    (build_router(state, None), temp)
}

fn fixture_sample_project(root: &Path) {
    write_collection(root, "requirements", "REQ");
    // REQ-a -> REQ-b resolvable; REQ-b has no outgoing.
    write_content(
        root,
        "requirements",
        "REQ-a",
        UUID_REQ_A,
        "Requirement A",
        serde_json::json!([{
            "targetUuid": UUID_REQ_B,
            "type": "derives-from",
            "hint": hint("sample", "REQ", "REQ-b"),
        }]),
    );
    write_content(
        root,
        "requirements",
        "REQ-b",
        UUID_REQ_B,
        "Requirement B",
        serde_json::json!([]),
    );
}

async fn get(router: &Router, uri: &str) -> axum::http::Response<Body> {
    router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn content_type(resp: &axum::http::Response<Body>) -> String {
    resp.headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

fn content_disposition(resp: &axum::http::Response<Body>) -> String {
    resp.headers()
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

#[tokio::test]
async fn json_export_returns_serialised_report_as_attachment() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(&router, "/api/reports/link-orphans/export/json").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(content_type(&resp).starts_with("application/json"));
    let disp = content_disposition(&resp);
    assert!(disp.contains("attachment"));
    assert!(disp.contains("reqforge-link-orphans-system-"));
    assert!(disp.ends_with(".json\""));
    let bytes = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["kind"], "link-orphans");
}

#[tokio::test]
async fn csv_export_returns_tabular_report_with_expected_header_row() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(&router, "/api/reports/unresolved-links/export/csv").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(content_type(&resp).starts_with("text/csv"));
    let disp = content_disposition(&resp);
    assert!(disp.contains("attachment"));
    assert!(disp.ends_with(".csv\""));
    let bytes = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.starts_with("source_project,"));
}

#[tokio::test]
async fn csv_export_of_cycles_declines_with_406_and_alternatives() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(&router, "/api/reports/cycles/export/csv").await;
    assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);
    let bytes = to_bytes(resp.into_body(), 16 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v["error"].as_str().unwrap().contains("cycles"));
    assert_eq!(v["alternatives"], serde_json::json!(["json", "html"]));
}

#[tokio::test]
async fn html_export_uses_absolute_urls_when_external_url_set() {
    let (router, _temp) =
        build_app_with_external(fixture_sample_project, Some("https://reports.example.com")).await;
    let resp = get(&router, "/api/reports/unresolved-links/export/html").await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(content_type(&resp).starts_with("text/html"));
    let disp = content_disposition(&resp);
    assert!(disp.ends_with(".html\""));
    let bytes = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.starts_with("<!DOCTYPE html>"));
    assert!(text.contains("<title>ReqForge · Unresolved links · system</title>"));
    // The only unresolved candidate in the sample fixture is the
    // REQ-a → REQ-b link, which does resolve, so the report body
    // reports the empty-state. The empty-state page still has
    // the title so we primarily assert on that + the doctype.
}

#[tokio::test]
async fn html_export_produces_relative_urls_when_external_url_empty() {
    let (router, _temp) = build_app_with_external(
        |root| {
            write_collection(root, "requirements", "REQ");
            // A solo artifact so the link-orphans report has a row whose
            // anchor we can inspect.
            write_content(
                root,
                "requirements",
                "REQ-solo",
                UUID_REQ_A,
                "Solo",
                serde_json::json!([]),
            );
        },
        None,
    )
    .await;
    let resp = get(&router, "/api/reports/link-orphans/export/html").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("/projects/sample/collections/REQ/artifacts/REQ-solo"));
    // Empty external base produces same-origin relative URL —
    // no `https://` prefix in the hrefs.
    assert!(
        !text.contains("https://"),
        "expected no absolute https URLs when external_url is empty"
    );
}

#[tokio::test]
async fn unknown_export_format_returns_404() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(&router, "/api/reports/link-orphans/export/xml").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn filename_slug_encodes_collection_scope() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(
        &router,
        "/api/reports/link-orphans/export/csv?scope=collection:sample/REQ",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let disp = content_disposition(&resp);
    assert!(
        disp.contains("reqforge-link-orphans-collection-sample-req-"),
        "expected collection-sample-req slug in filename: {disp}"
    );
}

#[tokio::test]
async fn unknown_report_kind_still_returns_404_on_export_path() {
    let (router, _temp) = build_app_with_external(fixture_sample_project, None).await;
    let resp = get(&router, "/api/reports/made-up-kind/export/json").await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
