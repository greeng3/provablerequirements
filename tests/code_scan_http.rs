//! Code-scan HTTP endpoint integration tests (Phase 9a.3), ported
//! from ReqForge `tests/code_scan_http.rs` for #374 batch F and
//! adapted to provreq's single-subject model. A thin HTTP wrapper
//! over `run_scan`, so these focus on the wire shape + error
//! handling.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs`. The harness has already written git +
//! `reqforge.json` at the subject root, so the seed closure only adds
//! collections/artifacts and the source tree the scanner walks.
//!
//! Dropped tests: none — both cases are single-subject.

mod support;

use axum::http::StatusCode;
use support::{build_app, get_json, write_artifact, write_collection};

#[tokio::test]
async fn code_scan_returns_resolved_tags_and_orphans() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact(
            root,
            "requirements",
            "REQ-apple",
            "0194f6d0-0006-7000-8000-000000000001",
            "REQ-apple",
        );
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            "// Satisfies: REQ-apple\n// Verifies: REQ-ghost\n",
        )
        .unwrap();
    })
    .await;
    let (status, value) = get_json(&router, "/api/projects/sample/code-scan").await;
    assert_eq!(status, StatusCode::OK);
    let by_artifact = value["tagsByArtifact"].as_object().unwrap();
    assert!(by_artifact.contains_key("sample/REQ/REQ-apple"));
    let orphans = value["orphanTags"].as_array().unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0]["rawId"], "REQ-ghost");
    assert_eq!(value["scannedFileCount"], 1);
    assert!(
        value["missingDeclaredScanPaths"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn code_scan_unknown_project_is_404() {
    let (router, _state, _temp) = build_app(|_| {}).await;
    let (status, value) = get_json(&router, "/api/projects/nope/code-scan").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("project 'nope' not found")
    );
}
