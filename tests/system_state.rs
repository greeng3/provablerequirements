//! Ported from ReqForge `tests/system_state.rs` (#374), adapted to
//! provreq's single-subject model.
//!
//! ReqForge drove the `UX-systemConfigBanner` permutations with
//! multiple mounts and a named/unnamed System config. provreq serves
//! exactly one project and no multi-project System, so `/api/system`
//! reports `loaded: false, projectCount: 1`. The multi-project and
//! named-System permutations are dropped — they are multi-mount only
//! and have no single-subject variant.

mod support;

use axum::http::StatusCode;
use support::{SUBJECT_SLUG, build_app, get_json};

#[tokio::test]
async fn system_reports_single_unnamed_project() {
    let (router, _state, _temp) = build_app(|_subject| {}).await;
    let (status, body) = get_json(&router, "/api/system").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["loaded"], false);
    assert_eq!(body["projectCount"], 1);
    // `name` is omitted (not null) when absent — the frontend keys
    // off key-missing to decide rendering.
    assert!(body.get("name").is_none(), "name should be omitted: {body}");
}

#[tokio::test]
async fn mounts_yields_exactly_one_loaded_subject() {
    // Directly validates the single-subject bootstrap: discovery
    // must classify the seeded subject as one loaded project.
    let (router, _state, _temp) = build_app(|_subject| {}).await;
    let (status, body) = get_json(&router, "/api/mounts").await;
    assert_eq!(status, StatusCode::OK);
    let mounts = body.as_array().expect("mounts array");
    assert_eq!(mounts.len(), 1, "single-subject serves one mount: {body}");
    assert_eq!(mounts[0]["state"], "project");
    assert_eq!(mounts[0]["project"]["slug"], SUBJECT_SLUG);
}

// #374: dropped — `returns_unloaded_and_project_count_for_unnamed_multi_project_mount`
// and `returns_loaded_and_name_for_named_system` are multi-project /
// System-config permutations with no single-subject variant.
