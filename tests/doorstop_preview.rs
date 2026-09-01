//! Doorstop preview endpoint integration tests (Phase 8.1
//! `POST /api/projects/:slug/doorstop/preview`), ported from
//! ReqForge `tests/doorstop_preview.rs` for #374 batch F and adapted
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
//! Dropped tests: none — every case is single-subject.

mod support;

use std::fs;
use std::path::Path;

use serde_json::json;

use support::{build_app, post_json};

fn write_doorstop_marker(dir: &Path, prefix: &str, sep: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join(".doorstop.yml"),
        format!("settings:\n  prefix: {prefix}\n  sep: '{sep}'\n  digits: 3\n  itemformat: yaml\n"),
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

/// Seed a clean doorstop source tree (REQ + DES markers) under the
/// subject's `doorstop/` directory.
fn fixture_clean(root: &Path) {
    // Doorstop source sits under <project>/doorstop/ (a sibling to the
    // artifacts dir).
    let req_dir = root.join("doorstop/req");
    write_doorstop_marker(&req_dir, "REQ", "-");
    write_doorstop_item(
        &req_dir,
        "REQ-001.yml",
        "Pressure envelope",
        "The vessel shall hold 10 MPa.",
        "'1.1'",
        "[]",
        "https://example.com/spec",
        "abc123",
        true,
    );
    let des_dir = root.join("doorstop/des");
    write_doorstop_marker(&des_dir, "DES", "-");
    write_doorstop_item(
        &des_dir,
        "DES-001.yml",
        "Titanium liner",
        "body",
        "'1.1'",
        "[REQ-001, DES-ghost]",
        "",
        "",
        true,
    );
}

/// Seed a pre-existing ReqForge REQ collection (should produce a
/// prefix collision on import) alongside a REQ doorstop source.
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
    // Doorstop source also uses REQ.
    let req_dir = root.join("doorstop/req");
    write_doorstop_marker(&req_dir, "REQ", "-");
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
async fn preview_renders_full_plan_without_writing_files() {
    let (router, _state, temp) = build_app(fixture_clean).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, 200);
    let panes = value["collections"].as_array().unwrap();
    assert_eq!(panes.len(), 2);
    // Panes come out in discovered order: des first (sorted marker
    // path), then req — since the walker sorts by marker path
    // lexicographically.
    let prefixes: Vec<&str> = panes
        .iter()
        .map(|p| p["prefix"].as_str().unwrap())
        .collect();
    assert!(prefixes.contains(&"REQ"));
    assert!(prefixes.contains(&"DES"));
    // REQ-001 has a URL-shaped ref → URL companion artifact.
    let req_pane = panes.iter().find(|p| p["prefix"] == "REQ").unwrap();
    let artifacts = req_pane["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 2); // source + URL companion
    // Source artifact carries a cites link at the companion.
    let source = &artifacts[0];
    let links = source["links"].as_array().unwrap();
    assert!(links.iter().any(|l| l["linkType"] == "cites"));
    // Synthetic review from the reviewed hash.
    assert_eq!(source["syntheticReview"]["outcome"], "approved");
    // DES-ghost is unresolved — flagged in the report.
    let unresolved = value["unresolvedLinks"].as_array().unwrap();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0]["targetUid"], "DES-ghost");
    // No collisions on a clean project.
    assert!(value["prefixCollisions"].as_array().unwrap().is_empty());
    // And no collection sidecar was written under artifacts/. (The
    // harness pre-creates an empty artifacts/ dir so the project
    // classifies as mountable; what we're asserting here is that
    // preview didn't populate it.)
    let artifacts = temp.path().join("sample/artifacts");
    let populated = fs::read_dir(&artifacts).map(|rd| rd.count()).unwrap_or(0);
    assert_eq!(populated, 0, "preview must not write collection files");
}

#[tokio::test]
async fn preview_surfaces_prefix_collision_without_refusing() {
    let (router, _state, _temp) = build_app(fixture_with_existing_req_collection).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, 200);
    let collisions = value["prefixCollisions"].as_array().unwrap();
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0]["prefix"], "REQ");
    assert_eq!(collisions[0]["existingCollectionDirectory"], "requirements");
    // Plan still renders the would-be pane so operators see the
    // impact.
    let panes = value["collections"].as_array().unwrap();
    assert!(panes.iter().any(|p| p["prefix"] == "REQ"));
}

#[tokio::test]
async fn preview_rejects_traversal_source_paths() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "../outside" }),
    )
    .await;
    assert_eq!(status, 400);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("project-root-relative")
    );
}

#[tokio::test]
async fn preview_rejects_absolute_source_paths() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (status, _) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "/etc" }),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn preview_reports_missing_source_as_400() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "no-such-dir" }),
    )
    .await;
    assert_eq!(status, 400);
    assert!(value["error"].as_str().unwrap().contains("does not exist"));
}

#[tokio::test]
async fn preview_unknown_project_is_404() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    let (status, _) = post_json(
        &router,
        "/api/projects/nope/doorstop/preview",
        &json!({ "source": "doorstop" }),
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn preview_empty_source_defaults_to_project_root() {
    let (router, _state, _temp) = build_app(fixture_clean).await;
    // An empty source string should scan the project root.
    let (status, value) = post_json(
        &router,
        "/api/projects/sample/doorstop/preview",
        &json!({ "source": "" }),
    )
    .await;
    assert_eq!(status, 200);
    // Still discovers both markers.
    let panes = value["collections"].as_array().unwrap();
    assert_eq!(panes.len(), 2);
}
