//! Ported from ReqForge `tests/llm_adapters.rs` (#374 batch E): the
//! LLM adapter layer.
//!
//! Each test stands up a `wiremock::MockServer` for each adapter
//! family, builds a minimal `AppState` with a `SystemConfig.llm`
//! block pointing at the mock, and drives the HTTP surface:
//!
//! - `GET /api/llm/providers`
//! - `POST /api/llm/providers/{index}/retest`
//! - `POST /api/llm/providers/{index}/acknowledge-privacy`
//! - `POST /api/llm/prompt` (debug)
//!
//! Covers: chain success per-family, fallback on 5xx, hard-disable
//! on 401, retest flipping hard-disabled back to healthy, privacy-
//! ack flow for a cloud endpoint, local-endpoint bypass.
//!
//! Boots via the multi-project `AppState::new` + `publish` path (an
//! in-memory World carrying the `llm` block, no mounts) rather than
//! the single-subject `tests/support` harness — the adapter machinery
//! reads `world.system.llm`, independent of any project on disk.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::index::UuidIndex;
use reqforge_model::schema::SystemConfig;
use reqforge_model::system::LoadedSystem;
use reqforge_model::world::{DiscoveryConfig, World};
use reqforge_model::write::OwnershipOverrides;
use serde_json::{Value, json};
use tower::util::ServiceExt;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

// --- Test harness --------------------------------------------------------

/// Build an `AppState` wrapped in a Router with a World that
/// carries the provided `llm` block. Every other World field is
/// minimally-populated so the LLM machinery can run without
/// needing real projects on disk.
async fn app_with_llm(llm: Value) -> (Router, Arc<AppState>) {
    let system_config = SystemConfig {
        schema_version: 1,
        name: "test".into(),
        projects: Vec::new(),
        link_types: Vec::new(),
        languages: None,
        llm: Some(llm),
        overflow: Default::default(),
    };
    let world = World {
        mounts: Vec::new(),
        index: UuidIndex::new(),
        duplicates: Vec::new(),
        system: LoadedSystem::Named {
            config: Box::new(system_config),
            source_path: PathBuf::from("/fake/system.json"),
        },
        missing_project_slugs: Vec::new(),
        link_catalog: reqforge_model::links::builtin_catalog().to_vec(),
        search_index: reqforge_model::search::empty_index(),
    };
    let state = Arc::new(AppState::new(
        DiscoveryConfig {
            mount_prefix: PathBuf::from("/nonexistent"),
            system_config_path: None,
            workspace_dir: None,
            max_blob_bytes: 50 * 1024 * 1024,
            thumbnail_cache_max_bytes: 500 * 1024 * 1024,
            external_url: None,
        },
        OwnershipOverrides::default(),
    ));
    state.publish(world).await;
    let router = build_router(state.clone(), None);
    (router, state)
}

async fn get_json(router: &Router, uri: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn post_json(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn post_empty(router: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

// --- OpenAI-compatible -----------------------------------------------------

#[tokio::test]
async fn openai_compatible_chain_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": "hello-openai"}}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 2}
        })))
        .mount(&server)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (status, body) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["servedByIndex"], 0);
    assert_eq!(body["text"], "hello-openai");
    assert_eq!(body["usage"]["inputTokens"], 1);
}

// --- Anthropic -------------------------------------------------------------

#[tokio::test]
async fn anthropic_chain_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                {"type": "text", "text": "hello-claude"},
                {"type": "tool_use", "name": "ignored"}
            ],
            "usage": {"input_tokens": 7, "output_tokens": 3}
        })))
        .mount(&server)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "anthropic",
            "model": "claude-haiku-4-5",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (status, body) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["text"], "hello-claude");
    assert_eq!(body["usage"]["outputTokens"], 3);
}

// --- Gemini ----------------------------------------------------------------

#[tokio::test]
async fn gemini_chain_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path_regex(r"/v1beta/models/.+:generateContent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "hello-gemini"}],
                    "role": "model"
                }
            }],
            "usageMetadata": {
                "promptTokenCount": 2,
                "candidatesTokenCount": 5
            }
        })))
        .mount(&server)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "gemini",
            "model": "gemini-2.0-flash",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (status, body) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["text"], "hello-gemini");
    assert_eq!(body["usage"]["inputTokens"], 2);
}

// --- Fallback on 5xx -------------------------------------------------------

#[tokio::test]
async fn fallback_on_server_error_advances_to_next_slot() {
    let broken = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&broken)
        .await;

    let ok = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": "fallback-text"}}]
        })))
        .mount(&ok)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "m-a",
            "endpoint": broken.uri(),
            "apiKey": "x",
        },
        {
            "provider": "openai-compatible",
            "model": "m-b",
            "endpoint": ok.uri(),
            "apiKey": "y",
        },
    ]))
    .await;

    let (status, body) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["servedByIndex"], 1);
    assert_eq!(body["text"], "fallback-text");

    // Provider 0 is now transient-degraded in the snapshot.
    let (status, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        providers["providers"][0]["health"]["kind"],
        "transient-degraded"
    );
    assert_eq!(providers["providers"][1]["health"]["kind"], "healthy");
}

// --- Hard-disable on 401 ---------------------------------------------------

#[tokio::test]
async fn auth_failure_hard_disables_slot() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (status, _body) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    let (_, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(providers["providers"][0]["health"]["kind"], "hard-disabled");
}

// --- Retest flips hard-disabled back to healthy ---------------------------

#[tokio::test]
async fn retest_recovers_from_hard_disabled_when_probe_now_succeeds() {
    let server = MockServer::start().await;
    // Start with a 401 mock so the first call hard-disables the slot.
    let failing = Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .named("initial-401")
        .mount_as_scoped(&server)
        .await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    // First call → hard-disabled.
    let (_, _) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    let (_, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(providers["providers"][0]["health"]["kind"], "hard-disabled");

    // Swap the mock for a 200 response (drop the scoped 401 mock first).
    drop(failing);
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": "pong"}}]
        })))
        .mount(&server)
        .await;

    // Retest → health back to healthy.
    let (status, body) = post_empty(&router, "/api/llm/providers/0/retest").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["health"]["kind"], "healthy");
}

// --- Privacy ack for a cloud endpoint -------------------------------------

#[tokio::test]
async fn privacy_ack_gates_remote_endpoint_then_clears() {
    // Point at a public URL (never actually contacted). The
    // privacy gate blocks the call before any HTTP work
    // happens, so the test is self-contained.
    let (router, _) = app_with_llm(json!([
        {
            "provider": "anthropic",
            "model": "claude-haiku-4-5",
            "endpoint": "https://api.anthropic.com",
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (_, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(providers["providers"][0]["isLocal"], false);
    assert_eq!(providers["providers"][0]["requiresPrivacyAck"], true);

    // Without ack, the chain skips the slot → AllFailed.
    let (status, _) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    // Ack → subsequent providers list shows requiresPrivacyAck cleared.
    let (status, body) = post_empty(&router, "/api/llm/providers/0/acknowledge-privacy").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["acknowledged"], true);

    let (_, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(providers["providers"][0]["requiresPrivacyAck"], false);
}

// --- Local endpoint bypass -------------------------------------------------

#[tokio::test]
async fn local_endpoint_bypasses_privacy_ack() {
    // wiremock binds to 127.0.0.1, which is-local per RFC 1918.
    let server = MockServer::start().await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "local-model",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        }
    ]))
    .await;

    let (_, providers) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(providers["providers"][0]["isLocal"], true);
    assert_eq!(providers["providers"][0]["requiresPrivacyAck"], false);
}

// --- Providers list shape --------------------------------------------------

#[tokio::test]
async fn providers_list_exposes_enabled_flag_and_key_availability() {
    let server = MockServer::start().await;

    let (router, _) = app_with_llm(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret-value",
        },
        {
            "provider": "anthropic",
            "model": "claude-haiku-4-5",
            "enabled": false,
        }
    ]))
    .await;

    let (status, body) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["providers"].as_array().unwrap();
    assert_eq!(entries.len(), 2);

    // openai-compatible: keyed → apiKeyAvailable, enabled by default,
    // no `apiKeyEnvVar` field on the wire (Phase 13 dropped it).
    assert_eq!(entries[0]["index"], 0);
    assert_eq!(entries[0]["provider"], "openai-compatible");
    assert_eq!(entries[0]["model"], "gpt-4o-mini");
    assert_eq!(entries[0]["apiKeyAvailable"], true);
    assert_eq!(entries[0]["enabled"], true);
    assert!(entries[0].get("apiKeyEnvVar").is_none());
    assert_eq!(entries[0]["health"]["kind"], "healthy");

    // anthropic: no apiKey → apiKeyAvailable false; enabled=false
    // mirrors the System config flag.
    assert_eq!(entries[1]["index"], 1);
    assert_eq!(entries[1]["provider"], "anthropic");
    assert_eq!(entries[1]["apiKeyAvailable"], false);
    assert_eq!(entries[1]["enabled"], false);
}

// --- No-config path --------------------------------------------------------

#[tokio::test]
async fn empty_llm_array_yields_empty_providers_list() {
    let (router, _) = app_with_llm(json!([])).await;
    let (status, body) = get_json(&router, "/api/llm/providers").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["providers"].as_array().unwrap().len(), 0);

    let (status, _) = post_json(&router, "/api/llm/prompt", json!({"prompt": "hi"})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
