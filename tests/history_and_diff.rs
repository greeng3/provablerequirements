//! Git history/diff endpoint integration tests (Phase 5d:
//! `/artifacts/:uuid/history`, `/artifact?at=<oid>`,
//! `/artifact/:uuid/diff`), ported from ReqForge
//! `tests/history_and_diff.rs` for #374 batch F and adapted to
//! provreq's single-subject model.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs`. In these tempdir tests the subject is an
//! external git repo — `.git` + `reqforge.json` share a root — so
//! `classify_single` yields `project_root == git_root`, matching
//! ReqForge's original single-dir classification. The git-repo setup
//! (init + three commits) runs verbatim inside the seed closure so
//! the commits are on disk before `refresh()` runs discovery.
//!
//! Dropped tests: none — every case is single-subject.

mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use axum::http::StatusCode;
use support::{build_app, get_json};

const UUID_REQ: &str = "0194f6d0-0002-7000-8000-000000000001";
const UUID_DES: &str = "0194f6d0-0002-7000-8000-000000000002";

fn run(dir: &Path, argv: &[&str]) {
    let status = Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| panic!("spawn {argv:?}: {e}"));
    assert!(status.success(), "command {argv:?} failed with {status}");
}

/// Build a real git repo under the subject root with three commits:
///   1. project skeleton + REQ-login body v1
///   2. REQ-login body -> v2
///   3. DES-spec added (content)
fn build_project_with_history(root: &Path) {
    fs::create_dir_all(root.join("artifacts/REQ")).unwrap();
    fs::create_dir_all(root.join("artifacts/DES")).unwrap();
    fs::write(
        root.join("reqforge.json"),
        serde_json::json!({
            "schemaVersion": 1,
            "slug": "sample",
            "name": "Sample",
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        root.join("artifacts/REQ/.collection.json"),
        serde_json::json!({
            "schemaVersion": 1,
            "prefix": "REQ",
            "name": "Requirements",
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        root.join("artifacts/DES/.collection.json"),
        serde_json::json!({
            "schemaVersion": 1,
            "prefix": "DES",
            "name": "Designs",
        })
        .to_string(),
    )
    .unwrap();

    let req_metadata = serde_json::json!({
        "schemaVersion": 1,
        "uuid": UUID_REQ,
        "title": "Login requirement",
        "shape": "content",
        "createdAt": "2026-04-18T00:00:00Z",
        "modifiedAt": "2026-04-18T00:00:00Z",
        "links": [],
        "reviewLog": [],
    });
    let req_path = root.join("artifacts/REQ/REQ-login.md");
    fs::write(
        &req_path,
        format!(
            "---\n{}\n---\nFirst draft of the login requirement.\n",
            req_metadata
        ),
    )
    .unwrap();

    run(root, &["git", "init", "-q", "-b", "main"]);
    run(root, &["git", "config", "user.email", "a@b"]);
    run(root, &["git", "config", "user.name", "Test"]);
    run(root, &["git", "add", "-A"]);
    run(
        root,
        &[
            "git",
            "commit",
            "-q",
            "-m",
            "bootstrap sample + REQ-login v1",
        ],
    );

    // Second commit: update the body.
    fs::write(
        &req_path,
        format!(
            "---\n{}\n---\nSecond draft — clarified the session-timeout wording.\n",
            req_metadata
        ),
    )
    .unwrap();
    run(root, &["git", "add", "-A"]);
    run(
        root,
        &["git", "commit", "-q", "-m", "refine REQ-login wording"],
    );

    // Third commit: add a DES-spec content artifact (separate from REQ).
    let des_metadata = serde_json::json!({
        "schemaVersion": 1,
        "uuid": UUID_DES,
        "title": "Design spec",
        "shape": "content",
        "createdAt": "2026-04-18T00:00:00Z",
        "modifiedAt": "2026-04-18T00:00:00Z",
        "links": [],
        "reviewLog": [],
    });
    fs::write(
        root.join("artifacts/DES/DES-spec.md"),
        format!("---\n{}\n---\nDesign placeholder.\n", des_metadata),
    )
    .unwrap();
    run(root, &["git", "add", "-A"]);
    run(
        root,
        &["git", "commit", "-q", "-m", "add DES-spec placeholder"],
    );
}

#[tokio::test]
async fn get_history_returns_commits_touching_the_tracked_file_newest_first() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    let (status, body) = get_json(&router, &format!("/api/artifacts/{UUID_REQ}/history")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["fallbackReason"].is_null());
    let commits = body["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 2, "REQ-login was touched in two commits");
    // Newest first — "refine" precedes the bootstrap commit.
    assert!(commits[0]["summary"].as_str().unwrap().contains("refine"));
}

#[tokio::test]
async fn get_history_on_empty_repo_surfaces_fallback_reason() {
    // Same project layout, but rewind to a freshly-init'd repo with no
    // commits — `head_commit()` fails and the handler must surface
    // fallback_reason rather than a 5xx. The wipe + re-init happen in
    // the seed closure so discovery sees the empty repo.
    let (router, _state, _temp) = build_app(|root| {
        build_project_with_history(root);
        fs::remove_dir_all(root.join(".git")).unwrap();
        run(root, &["git", "init", "-q", "-b", "main"]);
    })
    .await;
    let (status, body) = get_json(&router, &format!("/api/artifacts/{UUID_REQ}/history")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["commits"].as_array().unwrap().is_empty());
    assert!(
        body["fallbackReason"]
            .as_str()
            .unwrap()
            .contains("git history unavailable")
    );
}

#[tokio::test]
async fn get_artifact_at_oid_returns_historical_body() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    // Fetch history to pick the oldest REQ oid.
    let (_, history) = get_json(&router, &format!("/api/artifacts/{UUID_REQ}/history")).await;
    let commits = history["commits"].as_array().unwrap();
    let oldest_oid = commits[1]["oid"].as_str().unwrap();

    let (status, body) = get_json(
        &router,
        &format!("/api/artifacts/{UUID_REQ}?at={oldest_oid}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["body"].as_str().unwrap_or("").contains("First draft"),
        "expected v1 text, got: {}",
        body["body"]
    );
}

#[tokio::test]
async fn get_artifact_at_oid_surfaces_history_unavailable_on_invalid_oid() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    let (status, _) = get_json(
        &router,
        &format!("/api/artifacts/{UUID_REQ}?at=notarealoid"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn get_diff_returns_content_shape_line_diff_for_two_commits() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    let (_, history) = get_json(&router, &format!("/api/artifacts/{UUID_REQ}/history")).await;
    let commits = history["commits"].as_array().unwrap();
    let older = commits[1]["oid"].as_str().unwrap();
    let newer = commits[0]["oid"].as_str().unwrap();

    let (status, body) = get_json(
        &router,
        &format!("/api/artifacts/{UUID_REQ}/diff?from={older}&to={newer}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["shape"], "content");
    assert!(body["fallbackReason"].is_null());
    let lines = body["diff"]["lines"].as_array().unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l["kind"] == "removed"
                && l["text"].as_str().unwrap_or("").contains("First draft")),
        "expected v1 line to be marked removed"
    );
    assert!(
        lines
            .iter()
            .any(|l| l["kind"] == "added"
                && l["text"].as_str().unwrap_or("").contains("Second draft")),
        "expected v2 line to be marked added"
    );
}

#[tokio::test]
async fn get_diff_falls_back_with_reason_when_from_oid_is_invalid() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    let (status, body) = get_json(
        &router,
        &format!("/api/artifacts/{UUID_REQ}/diff?from=notarealoid"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["fallbackReason"].is_string());
    assert_eq!(body["shape"], "content");
}

#[tokio::test]
async fn get_diff_with_current_to_uses_working_tree_label() {
    let (router, _state, _temp) = build_app(build_project_with_history).await;
    let (_, history) = get_json(&router, &format!("/api/artifacts/{UUID_REQ}/history")).await;
    let oldest = history["commits"].as_array().unwrap()[1]["oid"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = get_json(
        &router,
        &format!("/api/artifacts/{UUID_REQ}/diff?from={oldest}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["toLabel"], "working tree");
}
