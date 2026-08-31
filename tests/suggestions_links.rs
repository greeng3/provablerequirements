//! Ported from ReqForge `tests/suggestions_links.rs` (#374 batches
//! C + E): the LLM-assisted link-suggestion HTTP surface (analyze /
//! list / accept / reject / reinstate).
//!
//! Single-subject: the original already stood up one "sample"
//! project, so the seeders map straight onto the shared harness.
//!
//! The two wiremock-driven analyze cases
//! (`analyze_happy_path_writes_pending_and_returns_ok`,
//! `analyze_surfaces_malformed_llm_output_as_bad_gateway`) were
//! deferred out of batch C (`wiremock` was not yet a provreq
//! dev-dependency) and restored here in batch E, which adds it.
//! They boot through a local `app_with_llm` helper that folds an
//! `llm` block into the subject's `system.json`.

mod support;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::world::DiscoveryConfig;
use reqforge_model::write::OwnershipOverrides;
use serde_json::{Value, json};
use support::{SUBJECT_SLUG, build_app, get_json, write_artifact, write_collection, write_project};
use tower::util::ServiceExt;
use wiremock::matchers::{method, path as wm_path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

/// Boot the app with the two seed artifacts AND a named `system.json`
/// carrying the given `llm` block, so `POST .../analyze` has a live
/// runtime to drive. Mirrors `app()` but sets `system_config_path`,
/// which `discover_single` reads into a `LoadedSystem::Named` with
/// the llm providers. The system config lives outside the subject and
/// (on POSIX) is mode 0600 — the loader rejects world-readable files
/// because they hold API keys.
async fn app_with_llm(llm: Value) -> (Router, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    write_collection(&subject, "requirements", "REQ");
    write_artifact(&subject, "requirements", "REQ-a", FROM_UUID, "From");
    write_artifact(&subject, "requirements", "REQ-b", TO_UUID, "To");

    let system_path = temp.path().join("system.json");
    std::fs::write(
        &system_path,
        json!({
            "schemaVersion": 2,
            "name": "test",
            "projects": [],
            "linkTypes": [],
            "llm": llm,
        })
        .to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&system_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let config = DiscoveryConfig {
        mount_prefix: subject.clone(),
        system_config_path: Some(system_path),
        workspace_dir: None,
        max_blob_bytes: 50 * 1024 * 1024,
        thumbnail_cache_max_bytes: 500 * 1024 * 1024,
        external_url: None,
    };
    let state = Arc::new(AppState::new_single_subject(
        subject.clone(),
        config,
        OwnershipOverrides::default(),
    ));
    state.refresh().await.unwrap();
    (build_router(state, None), temp)
}

fn chat_completion(content: &str) -> Value {
    json!({
        "choices": [{"message": {"content": content}}]
    })
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

#[tokio::test]
async fn analyze_happy_path_writes_pending_and_returns_ok() {
    // Stand up a wiremock server that pretends to be an
    // OpenAI-compatible LLM. Configure the AppState's LlmRuntime
    // to point at it. POST /analyze, assert the response is a
    // discriminated `ok` carrying the parsed suggestions and the
    // server-by attribution, and that pending.json on disk
    // matches the same suggestions.
    let server = MockServer::start().await;
    let suggestion_payload = format!(
        r#"[{{"from":"{FROM_UUID}","to":"{TO_UUID}","linkType":"derives-from","confidence":0.85,"rationale":"REQ-a derives from REQ-b"}}]"#
    );
    Mock::given(method("POST"))
        .and(wm_path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_completion(&suggestion_payload)),
        )
        .mount(&server)
        .await;

    let (router, temp) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ]))
    .await;

    let (status, body) =
        post_empty(&router, "/api/projects/sample/suggestions/links/analyze").await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["kind"], "ok");
    let suggestions = body["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0]["from"], FROM_UUID);
    assert_eq!(suggestions[0]["to"], TO_UUID);
    assert_eq!(suggestions[0]["linkType"], "derives-from");
    assert_eq!(suggestions[0]["rationale"], "REQ-a derives from REQ-b");
    assert!((suggestions[0]["confidence"].as_f64().unwrap() - 0.85).abs() < 1e-6);
    assert_eq!(body["servedByIndex"], 0);
    assert_eq!(body["servedBy"], "openai-compatible/gpt-4o-mini");

    // pending.json on disk mirrors the response so a refresh
    // sees the same set.
    let pending: Value = serde_json::from_str(
        &std::fs::read_to_string(
            temp.path()
                .join("sample/artifacts/.suggestions/pending.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let persisted = pending["suggestions"].as_array().unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0]["linkType"], "derives-from");
}

#[tokio::test]
async fn analyze_surfaces_malformed_llm_output_as_bad_gateway() {
    // The LLM returns prose without a JSON array at all. The
    // engine's parser should reject this as ParseError::NoArray,
    // which the handler maps to BAD_GATEWAY.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(wm_path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(
            "Sorry, I can't make any suggestions today.",
        )))
        .mount(&server)
        .await;

    let (router, _temp) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ]))
    .await;

    let (status, body) =
        post_empty(&router, "/api/projects/sample/suggestions/links/analyze").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body["error"].as_str().unwrap_or("").contains("JSON array"));
}
