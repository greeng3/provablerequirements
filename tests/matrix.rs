//! Ported from ReqForge `tests/matrix.rs` (#374 batch D): the
//! `GET /api/matrix` endpoint — two-axis scope parsing, per-axis
//! filters, and error mapping.
//!
//! Single-subject (#370): the fixture seeds one "sample" project;
//! `collection:sample/REQ` and `system` axis scopes resolve against
//! the one mount.

mod support;

use std::path::Path;

use axum::http::StatusCode;
use serde_json::{Value, json};
use support::{build_app, get_json, write_collection};

const UUID_REQ_A: &str = "0194f6d0-0006-7000-8000-00000000aaaa";
const UUID_REQ_B: &str = "0194f6d0-0006-7000-8000-00000000bbbb";
const UUID_DES_A: &str = "0194f6d0-0006-7000-8000-00000000dddd";
const UUID_DES_B: &str = "0194f6d0-0006-7000-8000-00000000dde2";

#[allow(clippy::too_many_arguments)]
fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: Value,
    review_log: Value,
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
        "reviewLog": review_log,
    });
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

    // REQ-a satisfies DES-a (the matrix is row=REQ col=DES
    // rendering the satisfies edge in one cell).
    write_content(
        root,
        "requirements",
        "REQ-a",
        UUID_REQ_A,
        "Alpha",
        json!([
            {
                "targetUuid": UUID_DES_A,
                "type": "satisfies",
                "hint": hint("sample", "DES", "DES-a"),
            },
        ]),
        json!([
            {
                "outcome": "approved",
                "reviewer": "alice",
                "timestamp": "2026-04-22T00:00:00Z",
                "addedTodos": [],
                "resolvedTodos": [],
            }
        ]),
        Some(vec!["core"]),
    );
    write_content(
        root,
        "requirements",
        "REQ-b",
        UUID_REQ_B,
        "Bravo",
        json!([]),
        json!([]),
        Some(vec!["fringe"]),
    );
    write_content(
        root,
        "designs",
        "DES-a",
        UUID_DES_A,
        "Design Alpha",
        json!([]),
        json!([]),
        None,
    );
    write_content(
        root,
        "designs",
        "DES-b",
        UUID_DES_B,
        "Design Bravo",
        json!([]),
        json!([]),
        None,
    );
}

#[tokio::test]
async fn matrix_default_surfaces_rows_columns_and_edges() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=collection:sample/REQ&columnScope=collection:sample/DES&linkType=satisfies",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalRows"], 2);
    assert_eq!(value["totalColumns"], 2);
    assert_eq!(value["rowsTruncated"], false);
    assert_eq!(value["columnsTruncated"], false);
    assert_eq!(value["linkType"]["name"], "satisfies");
    let rows = value["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["artifactName"], "REQ-a");
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["rowUuid"], UUID_REQ_A);
    assert_eq!(edges[0]["columnUuid"], UUID_DES_A);
}

#[tokio::test]
async fn matrix_requires_link_type_param() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=collection:sample/REQ&columnScope=collection:sample/DES",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("linkType query parameter is required")
    );
}

#[tokio::test]
async fn matrix_rejects_unknown_link_type() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=collection:sample/REQ&columnScope=collection:sample/DES&linkType=bogus",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("bogus"));
}

#[tokio::test]
async fn matrix_row_tag_filter_narrows_rows_only() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=collection:sample/REQ&columnScope=collection:sample/DES&linkType=satisfies&rowTags=core",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalRows"], 1);
    assert_eq!(value["rows"][0]["artifactName"], "REQ-a");
    // Columns unchanged.
    assert_eq!(value["totalColumns"], 2);
}

#[tokio::test]
async fn matrix_review_state_filter_uses_derived_state() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=collection:sample/REQ&columnScope=collection:sample/DES&linkType=satisfies&rowReviewStates=approved",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["totalRows"], 1);
    assert_eq!(value["rows"][0]["artifactName"], "REQ-a");
    assert_eq!(value["rows"][0]["reviewState"], "approved");
}

#[tokio::test]
async fn matrix_unknown_project_scope_is_404() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=project:nope&columnScope=system&linkType=satisfies",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("'nope' is not currently mounted")
    );
}

#[tokio::test]
async fn matrix_malformed_scope_is_400_with_axis_tag() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=bogus&columnScope=system&linkType=satisfies",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("row scope"));
}

#[tokio::test]
async fn matrix_unknown_review_state_is_400() {
    let (router, _state, _temp) = build_app(fixture_sample).await;
    let (status, value) = get_json(
        &router,
        "/api/matrix?rowScope=system&columnScope=system&linkType=satisfies&rowReviewStates=approved,bogus",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("bogus"));
}
