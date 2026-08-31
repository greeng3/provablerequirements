//! Ported from ReqForge `tests/graph.rs` (#374 batch D): the
//! `GET /api/graph` endpoint — scope + filter query parameters,
//! error mapping, and camelCase DTO shape.
//!
//! Single-subject (#370): the original already seeded one "sample"
//! project, so the fixture maps straight onto the harness (the
//! `scope=project:sample` / `collection:sample/REQ` forms resolve
//! against the one mount).

mod support;

use std::path::Path;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::{build_app, get_json, write_collection};

const UUID_REQ_A: &str = "0194f6d0-0006-7000-8000-00000000aaaa";
const UUID_REQ_B: &str = "0194f6d0-0006-7000-8000-00000000bbbb";
const UUID_REQ_C: &str = "0194f6d0-0006-7000-8000-00000000cccc";
const UUID_DES_A: &str = "0194f6d0-0006-7000-8000-00000000dddd";

#[allow(clippy::too_many_arguments)]
fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: Value,
    active: Option<bool>,
    tags: Option<Vec<&str>>,
) {
    let mut meta = json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-22T00:00:00Z",
        "modifiedAt": "2026-04-22T00:00:00Z",
        "links": links,
        "reviewLog": [],
    });
    if let Some(flag) = active {
        meta["active"] = Value::Bool(flag);
    }
    if let Some(ts) = tags {
        meta["tags"] = Value::Array(
            ts.into_iter()
                .map(|t| Value::String(t.to_owned()))
                .collect(),
        );
    }
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{meta}\n---\nbody text\n")).unwrap();
}

fn hint(slug: &str, prefix: &str, name: &str) -> Value {
    json!({
        "projectSlug": slug,
        "collectionPrefix": prefix,
        "artifactName": name,
    })
}

fn fixture_sample(root: &Path) {
    write_collection(root, "requirements", "REQ");
    write_collection(root, "designs", "DES");

    // REQ-a derives-from REQ-b, related-to REQ-c. Tagged "core".
    write_content(
        root,
        "requirements",
        "REQ-a",
        UUID_REQ_A,
        "Alpha",
        json!([
            {
                "targetUuid": UUID_REQ_B,
                "type": "derives-from",
                "hint": hint("sample", "REQ", "REQ-b"),
            },
            {
                "targetUuid": UUID_REQ_C,
                "type": "related-to",
                "hint": hint("sample", "REQ", "REQ-c"),
            },
        ]),
        None,
        Some(vec!["core"]),
    );
    write_content(
        root,
        "requirements",
        "REQ-b",
        UUID_REQ_B,
        "Bravo",
        json!([]),
        None,
        Some(vec!["core", "safety"]),
    );
    // REQ-c is inactive (excluded by default).
    write_content(
        root,
        "requirements",
        "REQ-c",
        UUID_REQ_C,
        "Charlie",
        json!([]),
        Some(false),
        Some(vec!["docs"]),
    );
    // DES-a satisfies REQ-a (non-acyclic link type).
    write_content(
        root,
        "designs",
        "DES-a",
        UUID_DES_A,
        "Design Alpha",
        json!([
            {
                "targetUuid": UUID_REQ_A,
                "type": "satisfies",
                "hint": hint("sample", "REQ", "REQ-a"),
            },
        ]),
        None,
        None,
    );
}

#[tokio::test]
async fn graph_default_scope_returns_all_active_nodes() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph").await;
    assert_eq!(status, StatusCode::OK);
    // 3 active nodes: REQ-a, REQ-b, DES-a (REQ-c inactive by default).
    assert_eq!(value["totalNodes"], 3);
    assert_eq!(value["truncated"], false);
    let nodes = value["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    // Edges: REQ-a→REQ-b (derives-from), DES-a→REQ-a (satisfies).
    // REQ-a→REQ-c dropped because REQ-c is inactive.
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 2);
    // Hint is false because 'satisfies' is not acyclic.
    assert_eq!(value["hintAllEdgesAcyclic"], false);
    // Every referenced link type carries its metadata.
    let ref_types = value["referencedLinkTypes"].as_array().unwrap();
    assert_eq!(ref_types.len(), 2);
    let names: Vec<&str> = ref_types
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"derives-from"));
    assert!(names.contains(&"satisfies"));
}

#[tokio::test]
async fn graph_include_inactive_pulls_in_excluded_nodes() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?includeInactive=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalNodes"], 4);
    // Now REQ-a→REQ-c is visible too.
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 3);
}

#[tokio::test]
async fn graph_link_type_filter_restricts_edges() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?linkTypes=derives-from").await;
    assert_eq!(status, StatusCode::OK);
    // Nodes unchanged — only edges are pruned.
    assert_eq!(value["totalNodes"], 3);
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["linkType"], "derives-from");
    // Now the only edge is acyclic, so the hint flips on.
    assert_eq!(value["hintAllEdgesAcyclic"], true);
}

#[tokio::test]
async fn graph_tag_filter_keeps_only_matching_artifacts() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?tags=safety").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalNodes"], 1);
    let nodes = value["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["artifactName"], "REQ-b");
    assert_eq!(value["edges"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn graph_collection_scope_narrows_to_prefix() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?scope=collection:sample/REQ").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalNodes"], 2); // REQ-a + REQ-b (active)
    // DES-a is filtered out, so its satisfies→REQ-a edge is too.
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["linkType"], "derives-from");
}

#[tokio::test]
async fn graph_unknown_project_scope_is_404() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?scope=project:nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("'nope' is not currently mounted")
    );
}

#[tokio::test]
async fn graph_unknown_collection_scope_is_404() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, value) = get_json(&router, "/api/graph?scope=collection:sample/NOPE").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("collection 'NOPE'")
    );
}

#[tokio::test]
async fn graph_malformed_scope_is_400() {
    let (router, _state, _temp) = build_app(fixture_sample).await;

    let (status, _) = get_json(&router, "/api/graph?scope=bogus-form").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
