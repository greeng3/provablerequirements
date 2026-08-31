//! Ported from ReqForge `tests/suggestions_links.rs` (#374 batch C):
//! the LLM-assisted link-suggestion HTTP surface (analyze / list /
//! accept / reject / reinstate).
//!
//! Single-subject: the original already stood up one "sample"
//! project, so the seeders map straight onto the shared harness.
//!
//! #374: the two wiremock-driven analyze cases
//! (`analyze_happy_path_writes_pending_and_returns_ok`,
//! `analyze_surfaces_malformed_llm_output_as_bad_gateway`) are
//! DROPPED — they need the `wiremock` dev-dependency, which provreq
//! does not carry, and this batch is test-only (no Cargo.toml
//! change). The live-adapter happy path is deferred to batch E,
//! alongside `llm_adapters`.

mod support;

use std::path::Path;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use support::{build_app, get_json, write_artifact, write_collection};
use tower::util::ServiceExt;

const FROM_UUID: &str = "0194f6d0-0001-7000-8000-000000000001";
const TO_UUID: &str = "0194f6d0-0001-7000-8000-000000000002";

/// Boot the app with the two seed artifacts the suggestion routes
/// operate on: REQ-a (`FROM_UUID`) and REQ-b (`TO_UUID`).
async fn app() -> (Router, tempfile::TempDir) {
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact(root, "requirements", "REQ-a", FROM_UUID, "From");
        write_artifact(root, "requirements", "REQ-b", TO_UUID, "To");
    })
    .await;
    (router, temp)
}

/// POST with an empty body — the suggestion routes take their inputs
/// from the path, not a request body.
async fn post_empty(router: &Router, uri: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(Request::post(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// Seed the pending sidecar with a single from→to suggestion of the
/// given link type. Returns the suggestion's id so tests can hit
/// accept/reject by id.
fn seed_pending(temp: &Path, link_type: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    let pending = serde_json::json!({
        "schemaVersion": 1,
        "suggestions": [{
            "id": id,
            "from": FROM_UUID,
            "to": TO_UUID,
            "linkType": link_type,
            "confidence": 0.85,
            "rationale": "test seed"
        }]
    });
    let dir = temp.join("sample/artifacts/.suggestions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("pending.json"), pending.to_string()).unwrap();
    id
}

fn seed_declined(temp: &Path, link_type: &str) -> String {
    let id = uuid::Uuid::now_v7().to_string();
    let declined = serde_json::json!({
        "schemaVersion": 1,
        "declined": [{
            "id": id,
            "from": FROM_UUID,
            "to": TO_UUID,
            "linkType": link_type,
            "confidence": 0.85,
            "rationale": "test seed",
            "declinedAt": "2026-05-04T12:00:00Z"
        }]
    });
    let dir = temp.join("sample/artifacts/.suggestions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("declined.json"), declined.to_string()).unwrap();
    id
}

#[tokio::test]
async fn analyze_returns_no_providers_when_llm_runtime_is_empty() {
    let (router, _temp) = app().await;
    let (status, body) =
        post_empty(&router, "/api/projects/sample/suggestions/links/analyze").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "noProviders");
}

#[tokio::test]
async fn list_pending_returns_empty_when_no_sidecar() {
    let (router, _temp) = app().await;
    let (status, body) = get_json(&router, "/api/projects/sample/suggestions/links").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["suggestions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn list_declined_returns_empty_when_no_sidecar() {
    let (router, _temp) = app().await;
    let (status, body) = get_json(&router, "/api/projects/sample/suggestions/links/declined").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["declined"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn list_pending_reads_seeded_sidecar() {
    let (router, temp) = app().await;
    seed_pending(temp.path(), "derives-from");
    let (status, body) = get_json(&router, "/api/projects/sample/suggestions/links").await;
    assert_eq!(status, StatusCode::OK);
    let suggestions = body["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0]["linkType"], "derives-from");
    assert_eq!(suggestions[0]["from"], FROM_UUID);
    assert_eq!(suggestions[0]["to"], TO_UUID);
}

#[tokio::test]
async fn accept_applies_link_and_drops_from_pending() {
    let (router, temp) = app().await;
    let id = seed_pending(temp.path(), "derives-from");

    let (status, _body) = post_empty(
        &router,
        &format!("/api/projects/sample/suggestions/links/{id}/accept"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // pending.json should now contain zero suggestions.
    let raw = std::fs::read_to_string(
        temp.path()
            .join("sample/artifacts/.suggestions/pending.json"),
    )
    .unwrap();
    let pending: Value = serde_json::from_str(&raw).unwrap();
    assert!(pending["suggestions"].as_array().unwrap().is_empty());

    // The from artifact's links field should now contain the new
    // link. Read the .md file directly to verify.
    let from_md =
        std::fs::read_to_string(temp.path().join("sample/artifacts/requirements/REQ-a.md"))
            .unwrap();
    assert!(
        from_md.contains("derives-from"),
        "expected REQ-a.md to gain a derives-from link, got: {from_md}",
    );
    assert!(
        from_md.contains(TO_UUID),
        "expected REQ-a.md to reference the target uuid, got: {from_md}",
    );
}

#[tokio::test]
async fn accept_unknown_id_returns_404() {
    let (router, _temp) = app().await;
    let unknown = uuid::Uuid::now_v7();
    let (status, _body) = post_empty(
        &router,
        &format!("/api/projects/sample/suggestions/links/{unknown}/accept"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reject_moves_pending_into_declined() {
    let (router, temp) = app().await;
    let id = seed_pending(temp.path(), "satisfies");

    let (status, _body) = post_empty(
        &router,
        &format!("/api/projects/sample/suggestions/links/{id}/reject"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let pending: Value = serde_json::from_str(
        &std::fs::read_to_string(
            temp.path()
                .join("sample/artifacts/.suggestions/pending.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(pending["suggestions"].as_array().unwrap().is_empty());

    let declined: Value = serde_json::from_str(
        &std::fs::read_to_string(
            temp.path()
                .join("sample/artifacts/.suggestions/declined.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let entries = declined["declined"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["linkType"], "satisfies");
    assert_eq!(entries[0]["id"], id);
    assert!(entries[0]["declinedAt"].is_string());
}

#[tokio::test]
async fn reinstate_applies_link_and_drops_from_declined() {
    let (router, temp) = app().await;
    let id = seed_declined(temp.path(), "verifies");

    let (status, _body) = post_empty(
        &router,
        &format!("/api/projects/sample/suggestions/links/{id}/reinstate"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let declined: Value = serde_json::from_str(
        &std::fs::read_to_string(
            temp.path()
                .join("sample/artifacts/.suggestions/declined.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(declined["declined"].as_array().unwrap().is_empty());

    let from_md =
        std::fs::read_to_string(temp.path().join("sample/artifacts/requirements/REQ-a.md"))
            .unwrap();
    assert!(
        from_md.contains("verifies"),
        "expected REQ-a.md to gain a verifies link after reinstate"
    );
}

#[tokio::test]
async fn reinstate_unknown_id_returns_404() {
    let (router, _temp) = app().await;
    let unknown = uuid::Uuid::now_v7();
    let (status, _body) = post_empty(
        &router,
        &format!("/api/projects/sample/suggestions/links/{unknown}/reinstate"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn analyze_for_unknown_project_returns_404() {
    let (router, _temp) = app().await;
    let (status, _body) = post_empty(&router, "/api/projects/nope/suggestions/links/analyze").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
