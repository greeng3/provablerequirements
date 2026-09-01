//! Schema-migration surface integration tests (Phase 11a), ported
//! from ReqForge `tests/schema_migration.rs` for #374 batch F and
//! adapted to provreq's single-subject model.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs` (`new_single_subject` + `refresh()`). The
//! harness has already written git + `reqforge.json` + an empty
//! `artifacts/` at the subject root, so the seed closure only adds the
//! collection + artifacts.
//!
//! Covers:
//! - The HTTP bulk-migrate endpoint against an all-v1 project returns
//!   `0` rewrites + no failures.
//! - A too-new artifact surfaces as a `failures` entry from the
//!   migrate endpoint.
//! - A too-new artifact surfaces as a `schemaDiagnostics` entry on the
//!   ProjectDetail response.
//! - The clean project omits the `schemaDiagnostics` field entirely.
//!
//! Dropped tests: none — every case is single-subject.

mod support;

use std::fs;
use std::path::Path;

use axum::http::StatusCode;
use serde_json::json;
use support::{build_app, get_json, post_json, write_collection};

fn write_artifact(root: &Path, collection_dir: &str, name: &str, uuid: &str, version: u64) {
    let fm = serde_json::to_string_pretty(&json!({
        "schemaVersion": version,
        "uuid": uuid,
        "title": name,
        "shape": "content",
        "createdAt": "2026-04-24T00:00:00Z",
        "modifiedAt": "2026-04-24T00:00:00Z",
        "links": [],
        "reviewLog": []
    }))
    .unwrap();
    fs::write(
        root.join("artifacts")
            .join(collection_dir)
            .join(format!("{name}.md")),
        format!("---\n{fm}\n---\n# {name}\n\nBody.\n"),
    )
    .unwrap();
}

/// Seed a clean all-v1 project: one REQ collection with one v1
/// artifact.
fn seed_v1(root: &Path) {
    write_collection(root, "req", "REQ");
    write_artifact(
        root,
        "req",
        "REQ-one",
        "11111111-1111-1111-1111-111111111111",
        1,
    );
}

/// The frontmatter of a hand-crafted, too-new (schemaVersion 99)
/// artifact — bypasses the write API, which always stamps the current
/// schemaVersion.
fn too_new_frontmatter() -> String {
    serde_json::to_string_pretty(&json!({
        "schemaVersion": 99,
        "uuid": "22222222-2222-2222-2222-222222222222",
        "title": "future",
        "shape": "content",
        "createdAt": "2026-04-24T00:00:00Z",
        "modifiedAt": "2026-04-24T00:00:00Z",
        "links": [],
        "reviewLog": []
    }))
    .unwrap()
}

#[tokio::test]
async fn migrate_schema_endpoint_on_all_v1_project_reports_zero_rewrites() {
    let (router, _state, _temp) = build_app(seed_v1).await;
    let (status, body) = post_json(
        &router,
        "/api/projects/sample/migrate-schema",
        &json!({ "force": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["projectSlug"], "sample");
    assert_eq!(body["result"]["filesRewritten"], 0);
    assert!(body["result"]["filesScanned"].as_u64().unwrap() >= 3);
    assert_eq!(body["result"]["failures"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn migrate_schema_endpoint_404s_for_unknown_project_slug() {
    let (router, _state, _temp) = build_app(seed_v1).await;
    let (status, _) = post_json(
        &router,
        "/api/projects/missing/migrate-schema",
        &json!({ "force": true }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn migrate_schema_endpoint_reports_failures_for_too_new_artifact() {
    let (router, _state, temp) = build_app(seed_v1).await;
    // Inject a too-new artifact directly on disk — bypasses the write
    // API because the write API uses the current schemaVersion.
    fs::write(
        temp.path().join("sample/artifacts/req/REQ-future.md"),
        format!("---\n{}\n---\n# future\n\nBody.\n", too_new_frontmatter()),
    )
    .unwrap();

    let (status, body) = post_json(
        &router,
        "/api/projects/sample/migrate-schema",
        &json!({ "force": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let failures = body["result"]["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1);
    assert!(
        failures[0]["error"]
            .as_str()
            .unwrap()
            .contains("newer than")
    );
}

#[tokio::test]
async fn project_detail_surfaces_schema_diagnostics_for_too_new_files() {
    // This test exercises the read path: after discovery sees a
    // too-new artifact, the project loader emits a SchemaTooNew
    // diagnostic that shows up in the ProjectDetail response. The
    // too-new file is seeded before the harness runs discovery.
    let (router, _state, _temp) = build_app(|root| {
        seed_v1(root);
        fs::write(
            root.join("artifacts/req/REQ-future.md"),
            format!("---\n{}\n---\n# future\n\nBody.\n", too_new_frontmatter()),
        )
        .unwrap();
    })
    .await;

    let (status, body) = get_json(&router, "/api/projects/sample").await;
    assert_eq!(status, StatusCode::OK);
    let diagnostics = body["schemaDiagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["fileType"], "artifact");
    assert_eq!(diagnostics[0]["foundVersion"], 99);
    assert_eq!(diagnostics[0]["currentVersion"], 1);
    assert!(
        diagnostics[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("REQ-future.md")
    );
}

#[tokio::test]
async fn project_detail_omits_schema_diagnostics_field_for_clean_project() {
    let (router, _state, _temp) = build_app(seed_v1).await;
    let (status, body) = get_json(&router, "/api/projects/sample").await;
    assert_eq!(status, StatusCode::OK);
    // When empty, the field is omitted entirely — preserving the prior
    // wire shape for v1-clean projects.
    assert!(body.get("schemaDiagnostics").is_none());
}
