//! Sample-content endpoint integration tests (Phase 11b), ported
//! from ReqForge `tests/sample_content.rs` for #374 batch F and
//! adapted to provreq's single-subject model.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs`. The harness already writes git +
//! `reqforge.json` + an empty `artifacts/`, so the seed closure is
//! empty and the sample-content handler operates on a clean subject.
//!
//! Dropped tests: none — every case is single-subject.

mod support;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use support::{build_app, get_json};
use tower::util::ServiceExt;

/// POST with an empty body — the sample-content route takes its input
/// from the path, not a request body.
async fn post_empty(router: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn happy_path_returns_201_and_writes_task_tracker_content() {
    let (router, _state, temp) = build_app(|_| {}).await;
    let (status, body) = post_empty(&router, "/api/projects/sample/sample-content").await;
    assert_eq!(status, StatusCode::CREATED, "body={body}");
    assert_eq!(body["projectSlug"], "sample");
    assert_eq!(body["collectionsCreated"], 3);
    assert_eq!(body["artifactsCreated"], 7);
    // Every collection summary carries non-empty artifact lists.
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 3);
    for c in collections {
        assert!(c["artifactCount"].as_u64().unwrap() >= 1);
        assert!(!c["artifactNames"].as_array().unwrap().is_empty());
    }
    // Files actually landed on disk.
    let root = temp.path().join("sample/artifacts");
    assert!(root.join("requirements/.collection.json").is_file());
    assert!(root.join("design/.collection.json").is_file());
    assert!(root.join("use-cases/.collection.json").is_file());
    assert!(root.join("requirements/REQ-task-creation.md").is_file());
    assert!(root.join("design/DES-data-model.md").is_file());
    assert!(root.join("use-cases/UC-receive-notification.md").is_file());
}

#[tokio::test]
async fn refuses_409_when_project_already_has_collections() {
    let (router, _state, _temp) = build_app(|_| {}).await;
    // First run — succeeds.
    let (status, _) = post_empty(&router, "/api/projects/sample/sample-content").await;
    assert_eq!(status, StatusCode::CREATED);
    // Second run — project is no longer empty.
    let (status, body) = post_empty(&router, "/api/projects/sample/sample-content").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("already has"));
}

#[tokio::test]
async fn returns_404_for_unknown_project_slug() {
    let (router, _state, _temp) = build_app(|_| {}).await;
    let (status, body) = post_empty(&router, "/api/projects/ghost/sample-content").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn seeded_project_listing_reports_the_new_collections() {
    let (router, _state, _temp) = build_app(|_| {}).await;
    let (status, _) = post_empty(&router, "/api/projects/sample/sample-content").await;
    assert_eq!(status, StatusCode::CREATED);

    // Fetch the project detail via the existing endpoint to prove the
    // refresh() call picked up the new content.
    let (status, body) = get_json(&router, "/api/projects/sample").await;
    assert_eq!(status, StatusCode::OK);
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 3);
    let prefixes: Vec<&str> = collections
        .iter()
        .map(|c| c["prefix"].as_str().unwrap())
        .collect();
    assert!(prefixes.contains(&"REQ"));
    assert!(prefixes.contains(&"DES"));
    assert!(prefixes.contains(&"UC"));
}
