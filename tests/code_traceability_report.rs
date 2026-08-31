//! Code-traceability report + coverage-matrix code-evidence
//! integration tests, ported from ReqForge's
//! `tests/code_traceability_report.rs` for #374 and adapted to
//! provreq's single-subject model.
//!
//! ReqForge drove multi-project `discover_mounts`; provreq serves one
//! repository (#370). These tests seed one subject (with a
//! mixed-language `src/`+`tests/` code tree) through the shared
//! single-subject harness — `refresh()` / `discover_single` builds
//! the search index the code scan relies on. Collections and content
//! carry an `expectsCodeTrace` flag, so this file ports ReqForge's
//! own richer `write_collection` / `write_content` as local helpers.
//!
//! Dropped tests: none.

mod support;

use std::path::Path;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;

use support::{build_app, get_json};

fn write_collection(root: &Path, dir: &str, prefix: &str, expects_code_trace: Option<bool>) {
    let path = root.join("artifacts").join(dir);
    std::fs::create_dir_all(&path).unwrap();
    let mut meta = serde_json::json!({
        "schemaVersion": 1,
        "prefix": prefix,
        "name": prefix,
    });
    if let Some(flag) = expects_code_trace {
        meta["expectsCodeTrace"] = serde_json::Value::Bool(flag);
    }
    std::fs::write(path.join(".collection.json"), meta.to_string()).unwrap();
}

fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    expects_code_trace: Option<bool>,
    links: serde_json::Value,
) {
    let mut meta = serde_json::json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": name,
        "shape": "content",
        "createdAt": "2026-04-22T00:00:00Z",
        "modifiedAt": "2026-04-22T00:00:00Z",
        "links": links,
        "reviewLog": [],
    });
    if let Some(flag) = expects_code_trace {
        meta["expectsCodeTrace"] = serde_json::Value::Bool(flag);
    }
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{}\n---\nbody\n", meta)).unwrap();
}

async fn body_bytes(router: &Router, uri: &str) -> (StatusCode, Vec<u8>, Option<String>) {
    let response = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, bytes.to_vec(), content_type)
}

/// Sample project with two requirements, one design, and a
/// mixed-language code tree covering the happy path + orphan
/// + uncovered.
fn fixture_sample(root: &Path) {
    write_collection(root, "requirements", "REQ", None);
    write_collection(root, "designs", "DES", Some(false));

    // REQ-covered has tags → not uncovered. REQ-uncovered
    // has no tags + expectsCodeTrace is default true → gap.
    // DES-impl lives in DES which has expectsCodeTrace=false
    // → never a gap.
    write_content(
        root,
        "requirements",
        "REQ-covered",
        "0194f6d0-0006-7000-8000-00000000aaaa",
        None,
        serde_json::json!([]),
    );
    write_content(
        root,
        "requirements",
        "REQ-uncovered",
        "0194f6d0-0006-7000-8000-00000000bbbb",
        None,
        serde_json::json!([]),
    );
    write_content(
        root,
        "designs",
        "DES-impl",
        "0194f6d0-0006-7000-8000-00000000dddd",
        None,
        serde_json::json!([]),
    );

    // src/lib.rs: Rust line tag for REQ-covered + one
    // orphan tag pointing at REQ-ghost.
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "// Satisfies: REQ-covered\n// Verifies: REQ-ghost\n",
    )
    .unwrap();
    // tests/smoke.rs: Rust block-comment tag for REQ-covered.
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::write(
        root.join("tests/smoke.rs"),
        "/*\nVerifies: REQ-covered\n*/\n",
    )
    .unwrap();
}

#[tokio::test]
async fn code_traceability_report_groups_locations_by_verb() {
    let (app, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&app, "/api/reports/code-traceability").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["kind"], "code-traceability");

    // REQ-covered has two verbs; REQ-uncovered has none and
    // is a gap; DES-impl has no tags and is NOT a gap because
    // its collection has expectsCodeTrace=false.
    let entries = value["entries"].as_array().unwrap();
    let covered = entries
        .iter()
        .find(|e| e["artifact"]["artifactName"] == "REQ-covered")
        .unwrap();
    let locations = covered["locationsByVerb"].as_object().unwrap();
    assert!(locations.contains_key("Satisfies"));
    assert!(locations.contains_key("Verifies"));
    assert_eq!(covered["hasGap"], false);

    let uncovered = entries
        .iter()
        .find(|e| e["artifact"]["artifactName"] == "REQ-uncovered")
        .unwrap();
    assert_eq!(uncovered["hasGap"], true);
    assert_eq!(uncovered["expectsCodeTrace"], true);

    let des = entries
        .iter()
        .find(|e| e["artifact"]["artifactName"] == "DES-impl")
        .unwrap();
    assert_eq!(des["hasGap"], false);
    assert_eq!(des["expectsCodeTrace"], false);

    // Orphans: REQ-ghost from the Verifies tag.
    let orphans = value["orphanTags"].as_array().unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0]["rawId"], "REQ-ghost");

    assert_eq!(value["totalArtifacts"], 3);
    assert_eq!(value["uncoveredCount"], 1);
    assert_eq!(value["orphanTagCount"], 1);
}

#[tokio::test]
async fn code_traceability_report_respects_scope_filter() {
    let (app, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &app,
        "/api/reports/code-traceability?scope=collection:sample/REQ",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = value["entries"].as_array().unwrap();
    // Only REQ-* artifacts.
    assert_eq!(entries.len(), 2);
    for e in entries {
        assert_eq!(e["artifact"]["collectionPrefix"], "REQ");
    }
}

#[tokio::test]
async fn coverage_matrix_surfaces_covering_code_evidence() {
    let (app, _state, _temp) = build_app(fixture_sample).await;
    // Satisfies + Verifies are the default covering types.
    let (status, value) = get_json(&app, "/api/reports/coverage-matrix").await;
    assert_eq!(status, StatusCode::OK);
    let parents = value["parents"].as_array().unwrap();
    let req_covered = parents
        .iter()
        .find(|p| p["parent"]["artifactName"] == "REQ-covered")
        .expect("REQ-covered must be present in parents list");
    // covering_children is empty (no satisfies-link from any
    // artifact) but covering_code_evidence now lists two
    // tags; hasGap drops to false as a result.
    let evidence = req_covered["coveringCodeEvidence"]
        .as_array()
        .expect("coveringCodeEvidence must serialise when non-empty");
    assert_eq!(evidence.len(), 2);
    assert_eq!(req_covered["hasGap"], false);
    let req_uncovered = parents
        .iter()
        .find(|p| p["parent"]["artifactName"] == "REQ-uncovered")
        .expect("REQ-uncovered must be present in parents list");
    // No children, no code evidence → gap. The field
    // skip_serializes when empty, so we check that the key
    // is either absent or an empty array.
    match req_uncovered.get("coveringCodeEvidence") {
        None | Some(serde_json::Value::Null) => {}
        Some(value) => assert!(
            value.as_array().map(|a| a.is_empty()).unwrap_or(false),
            "coveringCodeEvidence should be absent or empty, got {value:?}",
        ),
    }
    assert_eq!(req_uncovered["hasGap"], true);
}

#[tokio::test]
async fn code_traceability_exports_in_all_three_formats() {
    let (app, _state, _temp) = build_app(fixture_sample).await;
    for (ext, expected_mime_prefix, expected_contents) in [
        ("json", "application/json", "\"orphanTags\""),
        ("csv", "text/csv", "#### locations"),
        ("html", "text/html", "<h1>Code traceability</h1>"),
    ] {
        let uri = format!("/api/reports/code-traceability/export/{ext}");
        let (status, bytes, content_type) = body_bytes(&app, &uri).await;
        assert_eq!(status, StatusCode::OK, "ext={ext}");
        let ct = content_type.unwrap_or_default();
        assert!(
            ct.starts_with(expected_mime_prefix),
            "ext={ext} content-type={ct}"
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains(expected_contents),
            "ext={ext} body missing expected contents: {}",
            &text[..text.len().min(400)],
        );
    }
}

#[tokio::test]
async fn code_traceability_unknown_kind_404_remains_intact() {
    // Ensures the `from_kebab` addition didn't accidentally
    // shadow an unknown kind through to a 500.
    let (app, _state, _temp) = build_app(fixture_sample).await;
    let (status, _) = get_json(&app, "/api/reports/not-a-real-kind").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
