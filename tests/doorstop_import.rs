//! Doorstop import endpoint integration tests (Phase 8.2: import
//! execute + cached report + Phase 6b-shaped exports), ported from
//! ReqForge `tests/doorstop_import.rs` for #374 batch F and adapted
//! to provreq's single-subject model.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs`. The harness has already written git +
//! `reqforge.json` + an empty `artifacts/` at the subject root, so
//! each file-local fixture writes only the doorstop source tree (and,
//! for the collision fixture, the pre-existing collection).
//!
//! Walks the happy path end-to-end: import → on-disk files + returned
//! report + report-export in each of JSON/CSV/HTML. Also covers the
//! prefix-collision refusal and the "no run yet" 404 surface.
//!
//! Dropped tests: none — every case is single-subject.

mod support;

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::json;
use support::{build_app, post_json};
use tower::util::ServiceExt;

fn write_doorstop_marker(dir: &Path, prefix: &str, sep: &str, parent: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join(".doorstop.yml"),
        format!(
            "settings:\n  prefix: {prefix}\n  sep: '{sep}'\n  digits: 3\n  parent: {parent}\n  itemformat: yaml\n"
        ),
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn write_doorstop_item(
    dir: &Path,
    filename: &str,
    header: &str,
    text: &str,
    level: &str,
    links: &str,
    ref_val: &str,
    reviewed: &str,
    normative: bool,
) {
    fs::create_dir_all(dir).unwrap();
    let yaml = format!(
        "active: true\nderived: false\nheader: |\n  {header}\nlevel: {level}\nlinks: {links}\nnormative: {normative}\nref: '{ref_val}'\nreviewed: {reviewed}\ntext: |\n  {text}\n",
    );
    fs::write(dir.join(filename), yaml).unwrap();
}

/// GET returning the raw body bytes + content-type, for the export
/// endpoints whose responses are not JSON.
async fn get_raw(router: &Router, uri: &str) -> (StatusCode, Vec<u8>, Option<String>) {
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
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, bytes, content_type)
}

fn fixture_clean(root: &Path) {
    let req_dir = root.join("doorstop/req");
    write_doorstop_marker(&req_dir, "REQ", "-", "");
    write_doorstop_item(
        &req_dir,
        "REQ-001.yml",
        "Pressure envelope",
        "body",
        "'1.1'",
        "[]",
        "https://example.com/spec",
        "abc123",
        true,
    );
    let des_dir = root.join("doorstop/des");
    write_doorstop_marker(&des_dir, "DES", "-", "REQ");
    write_doorstop_item(
        &des_dir,
        "DES-001.yml",
        "Titanium liner",
        "body",
        "'1.1'",
        "[REQ-001, REQ-ghost]",
        "Smith 1994",
        "",
        false,
    );
}

fn fixture_with_existing_req_collection(root: &Path) {
    let existing = root.join("artifacts/requirements");
    fs::create_dir_all(&existing).unwrap();
    fs::write(
        existing.join(".collection.json"),
        json!({
            "schemaVersion": 1,
            "prefix": "REQ",
            "name": "Requirements",
        })
        .to_string(),
    )
    .unwrap();
    let req_dir = root.join("doorstop/req");
    write_doorstop_marker(&req_dir, "REQ", "-", "");
    write_doorstop_item(
        &req_dir,
        "REQ-001.yml",
        "Pressure envelope",
        "body",
        "'1.1'",
        "[]",
        "",
        "",
        true,
    );
}

#[tokio::test]
async fn import_writes_collections_artifacts_and_url_companions() {
    let (router, _state, temp) = build_app(fixture_clean).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Totals reflect 2 collections, 3 artifacts (REQ-001 + its URL
    // companion + DES-001), 1 cites link, 1 legacy ref (Smith 1994),
    // 1 unresolved link (REQ-ghost).
    let totals = &value["totals"];
    assert_eq!(totals["collectionsCreated"], 2);
    assert_eq!(totals["artifactsImported"], 3);
    assert_eq!(totals["citesLinks"], 1);
    assert_eq!(totals["urlArtifacts"], 1);
    assert_eq!(totals["legacyRefs"], 1);
    assert_eq!(totals["syntheticReviewEntries"], 1);
    assert_eq!(totals["unresolvedLinkCount"], 1);

    // On-disk: collection sidecars and artifact files exist.
    let req_dir = temp.path().join("sample/artifacts/req");
    assert!(req_dir.join(".collection.json").is_file());
    assert!(req_dir.join("REQ-001.md").is_file());
    assert!(
        req_dir.join("REQ-001_ref.md").is_file(),
        "URL companion should have been written"
    );
    let des_dir = temp.path().join("sample/artifacts/des");
    assert!(des_dir.join("DES-001.md").is_file());

    // Sidecar carries importNotes with the doorstop settings.
    let sidecar = fs::read_to_string(req_dir.join(".collection.json")).unwrap();
    assert!(sidecar.contains("\"doorstopSep\": \"-\""));
    assert!(sidecar.contains("\"doorstopItemFormat\": \"yaml\""));

    // Content-artifact frontmatter carries the synthetic review entry
    // + legacy.doorstopUid.
    let req001 = fs::read_to_string(req_dir.join("REQ-001.md")).unwrap();
    assert!(req001.contains("imported-from-doorstop"));
    assert!(req001.contains("\"doorstopUid\": \"REQ-001\""));

    // DES-001 carries non-normative tag + legacy.ref.
    let des001 = fs::read_to_string(des_dir.join("DES-001.md")).unwrap();
    assert!(des001.contains("non-normative"));
    assert!(des001.contains("\"ref\": \"Smith 1994\""));
    // Its unresolved REQ-ghost link is present with an empty
    // collectionPrefix hint (the wire marker).
    assert!(des001.contains("REQ-ghost"));

    // Original doorstop files untouched.
    assert!(
        temp.path()
            .join("sample/doorstop/req/REQ-001.yml")
            .is_file(),
    );
    assert!(
        temp.path()
            .join("sample/doorstop/des/DES-001.yml")
            .is_file(),
    );
}

#[tokio::test]
async fn import_refuses_when_a_prefix_collision_is_present() {
    let (router, _state, temp) = build_app(fixture_with_existing_req_collection).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let collisions = value["collisions"].as_array().unwrap();
    assert_eq!(collisions[0]["prefix"], "REQ");
    // Confirm no new collection directory was written — the existing
    // requirements/ stays put, nothing under req/.
    assert!(!temp.path().join("sample/artifacts/req").exists());
}

#[tokio::test]
async fn report_endpoint_404s_before_any_import_runs() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (status, _bytes, _ct) = get_raw(&router, "/api/projects/sample/doorstop/report").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn report_endpoint_returns_cached_report_after_import() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (import_status, _) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(import_status, StatusCode::OK);

    let (status, bytes, _) = get_raw(&router, "/api/projects/sample/doorstop/report").await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["projectSlug"], "sample");
    assert_eq!(value["totals"]["collectionsCreated"], 2);
}

#[tokio::test]
async fn report_export_renders_json_csv_html_with_sensible_filenames() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (import_status, _) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(import_status, StatusCode::OK);

    for (ext, expected_mime_prefix, expected_contents) in [
        ("json", "application/json", "\"collectionsCreated\""),
        ("csv", "text/csv", "# collections"),
        ("html", "text/html", "<h1>Doorstop import report</h1>"),
    ] {
        let (status, bytes, content_type) = get_raw(
            &router,
            &format!("/api/projects/sample/doorstop/report/export/{ext}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "ext={ext}");
        let ct = content_type.unwrap_or_default();
        assert!(
            ct.starts_with(expected_mime_prefix),
            "ext={ext} content-type={ct}"
        );
        let body = String::from_utf8_lossy(&bytes);
        assert!(
            body.contains(expected_contents),
            "ext={ext} body missing expected contents: {body}"
        );
    }
}

#[tokio::test]
async fn report_export_rejects_unknown_extensions() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (_s, _b) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    let (status, _bytes, _) =
        get_raw(&router, "/api/projects/sample/doorstop/report/export/pdf").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn import_preserves_the_original_doorstop_files() {
    let (router, _state, temp) = build_app(fixture_clean).await;
    let before = fs::read(temp.path().join("sample/doorstop/req/REQ-001.yml")).unwrap();
    let (status, _) = post_json(
        &router,
        "/api/projects/sample/doorstop/import",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let after = fs::read(temp.path().join("sample/doorstop/req/REQ-001.yml")).unwrap();
    assert_eq!(before, after);
}
