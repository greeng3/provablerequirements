//! Ported from ReqForge `tests/search.rs` (#374 batch D): the
//! `GET /api/search` endpoint — Tantivy text queries, field-scoped
//! and boolean queries, structured filters, pagination, snippets,
//! and error mapping.
//!
//! Single-subject (#370): the fixture seeds one "sample" project.
//! The shared harness's `refresh()` builds the real search index
//! through `discover_single`, so no bespoke index wiring is needed.

mod support;

use std::path::Path;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::{build_app, get_json, write_collection};

#[allow(clippy::too_many_arguments)]
fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    body: &str,
    tags: Option<Vec<&str>>,
    active: Option<bool>,
    review_log: Value,
    links: Value,
    description: Option<&str>,
) {
    let mut meta = json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-22T00:00:00Z",
        "modifiedAt": "2026-04-22T00:00:00Z",
        "links": links,
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
    if let Some(d) = description {
        meta["description"] = Value::String(d.to_owned());
    }
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{meta}\n---\n{body}\n")).unwrap();
}

fn fixture_sample(root: &Path) {
    write_collection(root, "requirements", "REQ");
    write_collection(root, "designs", "DES");

    // REQ-core: approved, has outgoing link, tagged core/safety.
    write_content(
        root,
        "requirements",
        "REQ-core",
        "0194f6d0-0006-7000-8000-000000000001",
        "Core requirement",
        "The reactor vessel shall satisfy the pressure envelope.",
        Some(vec!["core", "safety"]),
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
        json!([
            {
                "targetUuid": "0194f6d0-0006-7000-8000-000000000002",
                "type": "satisfies",
                "hint": {
                    "projectSlug": "sample",
                    "collectionPrefix": "DES",
                    "artifactName": "DES-impl"
                }
            }
        ]),
        Some("root-level safety goal"),
    );
    // REQ-legacy: inactive, no links, never reviewed.
    write_content(
        root,
        "requirements",
        "REQ-legacy",
        "0194f6d0-0006-7000-8000-000000000003",
        "Legacy interlock",
        "Legacy paragraph about interlocks and wiring.",
        None,
        Some(false),
        json!([]),
        json!([]),
        None,
    );
    // DES-impl: active, no outgoing links, never reviewed.
    write_content(
        root,
        "designs",
        "DES-impl",
        "0194f6d0-0006-7000-8000-000000000002",
        "Implementation design",
        "Implementation details for the reactor vessel.",
        None,
        None,
        json!([]),
        json!([]),
        None,
    );
}

#[tokio::test]
async fn search_default_field_hits_title_body_and_tags() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?q=reactor").await;
    assert_eq!(status, StatusCode::OK);
    // Only active artifacts — REQ-core (body) + DES-impl (body).
    assert_eq!(value["totalHits"], 2);
    let names: Vec<&str> = value["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["artifactName"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"REQ-core"));
    assert!(names.contains(&"DES-impl"));
}

#[tokio::test]
async fn search_empty_query_runs_match_all() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search").await;
    assert_eq!(status, StatusCode::OK);
    // Default excludes inactive: REQ-core + DES-impl.
    assert_eq!(value["totalHits"], 2);
}

#[tokio::test]
async fn search_include_inactive_adds_legacy_hits() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?includeInactive=true").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalHits"], 3);
}

#[tokio::test]
async fn search_scope_filter_narrows_to_collection() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?scope=collection:sample/DES").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalHits"], 1);
    assert_eq!(value["hits"][0]["artifactName"], "DES-impl");
}

#[tokio::test]
async fn search_review_state_filter_uses_derived_state() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?reviewState=approved").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalHits"], 1);
    assert_eq!(value["hits"][0]["artifactName"], "REQ-core");
    assert_eq!(value["hits"][0]["reviewState"], "approved");
}

#[tokio::test]
async fn search_has_links_filter_partitions_results() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (_, with_links) = get_json(&router, "/api/search?hasLinks=true").await;
    assert_eq!(with_links["totalHits"], 1);
    assert_eq!(with_links["hits"][0]["artifactName"], "REQ-core");
    let (_, no_links) = get_json(&router, "/api/search?hasLinks=false").await;
    assert_eq!(no_links["totalHits"], 1);
    assert_eq!(no_links["hits"][0]["artifactName"], "DES-impl");
}

#[tokio::test]
async fn search_malformed_query_returns_400() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?q=%22unterminated").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("malformed query"));
}

#[tokio::test]
async fn search_unknown_review_state_returns_400() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?reviewState=approved,bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("bogus"));
}

#[tokio::test]
async fn search_unknown_project_scope_is_404() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, _) = get_json(&router, "/api/search?scope=project:nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn search_pagination_threads_offset_and_limit() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (_, page1) = get_json(&router, "/api/search?limit=1&offset=0").await;
    assert_eq!(page1["hits"].as_array().unwrap().len(), 1);
    assert_eq!(page1["truncated"], true);
    let (_, page2) = get_json(&router, "/api/search?limit=1&offset=1").await;
    assert_eq!(page2["hits"].as_array().unwrap().len(), 1);
    assert_eq!(page2["truncated"], false);
}

#[tokio::test]
async fn search_snippet_carries_mark_tag_when_body_matches() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(&router, "/api/search?q=reactor").await;
    assert_eq!(status, StatusCode::OK);
    let snippets: Vec<Option<&str>> = value["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["snippet"].as_str())
        .collect();
    // At least one hit has a snippet with the <mark> tag.
    assert!(
        snippets
            .iter()
            .any(|s| s.is_some_and(|t| t.contains("<mark>")))
    );
}
