//! Ported from ReqForge `tests/browse.rs` (#374 batch D): the
//! `GET /api/browse` endpoint — prefix-keyed panes, in-pane sort,
//! the filter vocabulary shared with search, and error mapping.
//!
//! Single-subject (#370): the fixture seeds one "sample" project.
//! #374: `browse_name_variants_surface_across_projects` is DROPPED —
//! it seeds two sibling projects (alpha + beta) sharing the "REQ"
//! prefix to prove name variants merge across projects, which a
//! one-mount World cannot represent.

mod support;

use std::path::Path;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::{build_app, get_json};

fn write_collection_named(root: &Path, dir: &str, prefix: &str, name: &str) {
    let path = root.join("artifacts").join(dir);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join(".collection.json"),
        json!({
            "schemaVersion": 1,
            "prefix": prefix,
            "name": name,
        })
        .to_string(),
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    tags: Option<Vec<&str>>,
    active: Option<bool>,
    review_log: Value,
) {
    let mut meta = json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-22T00:00:00Z",
        "modifiedAt": "2026-04-22T00:00:00Z",
        "links": [],
        "reviewLog": review_log,
    });
    if let Some(ts) = tags {
        meta["tags"] = Value::Array(
            ts.into_iter()
                .map(|t| Value::String(t.to_owned()))
                .collect(),
        );
    }
    if let Some(flag) = active {
        meta["active"] = Value::Bool(flag);
    }
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{meta}\n---\nbody\n")).unwrap();
}

fn fixture_sample(root: &Path) {
    write_collection_named(root, "requirements", "REQ", "Requirements");
    write_collection_named(root, "designs", "DES", "Design Documents");

    write_content(
        root,
        "requirements",
        "REQ-apple",
        "0194f6d0-0006-7000-8000-000000000001",
        "Apple",
        Some(vec!["core"]),
        None,
        json!([
            {
                "outcome": "approved",
                "reviewer": "alice",
                "timestamp": "2026-04-22T00:00:00Z",
                "addedTodos": [],
                "resolvedTodos": []
            }
        ]),
    );
    write_content(
        root,
        "requirements",
        "REQ-banana",
        "0194f6d0-0006-7000-8000-000000000002",
        "Banana",
        None,
        None,
        json!([]),
    );
    write_content(
        root,
        "requirements",
        "REQ-dropped",
        "0194f6d0-0006-7000-8000-000000000003",
        "Dropped",
        None,
        Some(false),
        json!([]),
    );
    write_content(
        root,
        "designs",
        "DES-alpha",
        "0194f6d0-0006-7000-8000-000000000004",
        "Alpha",
        None,
        None,
        json!([]),
    );
}

#[tokio::test]
async fn browse_default_system_scope_groups_by_prefix() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalPanes"], 2);
    let panes = value["panes"].as_array().unwrap();
    assert_eq!(panes[0]["prefix"], "DES");
    assert_eq!(panes[1]["prefix"], "REQ");
    // REQ pane excludes the inactive artifact by default.
    assert_eq!(panes[1]["totalArtifacts"], 2);
    let names: Vec<&str> = panes[1]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["artifactName"].as_str().unwrap())
        .collect();
    // Sorted by title case-insensitive: Apple before Banana.
    assert_eq!(names, vec!["REQ-apple", "REQ-banana"]);
}

#[tokio::test]
async fn browse_include_inactive_brings_dropped_back() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?includeInactive=true").await;
    assert_eq!(status, StatusCode::OK);
    let panes = value["panes"].as_array().unwrap();
    let req = panes.iter().find(|p| p["prefix"] == "REQ").unwrap();
    assert_eq!(req["totalArtifacts"], 3);
}

#[tokio::test]
async fn browse_scope_narrows_to_one_pane() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?scope=collection:sample/DES").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalPanes"], 1);
    assert_eq!(value["panes"][0]["prefix"], "DES");
}

#[tokio::test]
async fn browse_tag_filter_narrows_within_pane() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?tags=core").await;
    assert_eq!(status, StatusCode::OK);
    let panes = value["panes"].as_array().unwrap();
    // REQ survives (REQ-apple tagged core) but DES has no
    // tagged artifacts; the DES pane still appears (the
    // collection exists) but empty. Panes for empty
    // collections are acceptable — the UI filter narrows
    // further — but we still confirm REQ-apple is the only
    // artifact anywhere in the response.
    let total = value["totalArtifacts"].as_u64().unwrap();
    assert_eq!(total, 1);
    let req = panes.iter().find(|p| p["prefix"] == "REQ").unwrap();
    assert_eq!(req["totalArtifacts"], 1);
    assert_eq!(req["artifacts"][0]["artifactName"], "REQ-apple");
}

#[tokio::test]
async fn browse_review_state_filter_uses_derived_state() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?reviewState=approved").await;
    assert_eq!(status, StatusCode::OK);
    let panes = value["panes"].as_array().unwrap();
    let req = panes.iter().find(|p| p["prefix"] == "REQ").unwrap();
    assert_eq!(req["totalArtifacts"], 1);
    assert_eq!(req["artifacts"][0]["artifactName"], "REQ-apple");
    assert_eq!(req["artifacts"][0]["reviewState"], "approved");
}

#[tokio::test]
async fn browse_unknown_review_state_is_400() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?reviewState=approved,bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("bogus"));
}

#[tokio::test]
async fn browse_unknown_project_scope_is_404() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/browse?scope=project:nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("'nope' is not currently mounted")
    );
}
