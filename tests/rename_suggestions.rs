//! Rename-suggestion endpoint integration tests (Phase 10b.1), ported
//! from ReqForge `tests/rename_suggestions.rs` for #374 batch F and
//! adapted to provreq's single-subject model.
//!
//! ReqForge seeded its project under `prefix/sample` and drove
//! multi-project discovery; provreq serves exactly one repository
//! (#370), so these boot through the shared single-subject harness in
//! `tests/support/mod.rs`. The LLM-driven cases fold an `llm` block
//! into a named `system.json` (outside the subject, mode 0600) and
//! boot via `new_single_subject` with `system_config_path` set —
//! mirroring `suggestions_links.rs::app_with_llm` — because the shared
//! harness fixes `system_config_path: None`. A wiremock `MockServer`
//! stands in for an OpenAI-compatible provider.
//!
//! Covers:
//! - Single-artifact happy path returns three suggestions with a
//!   `servedBy` label.
//! - Malformed LLM output surfaces as a typed BAD_GATEWAY.
//! - Unack'd cloud provider returns `privacyAckRequired` carrying the
//!   indices the UI should ask about.
//! - Bulk endpoint runs with parallelism 4 (checked via wiremock
//!   response delay).
//! - Empty-LLM-config path returns 200 with `noProviders` / empty
//!   results so the UI degrades gracefully.
//!
//! Dropped tests: none — every case is single-subject.

mod support;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::world::DiscoveryConfig;
use reqforge_model::write::OwnershipOverrides;
use serde_json::{Value, json};
use support::{
    SUBJECT_SLUG, build_app, post_json, write_artifact, write_collection, write_project,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Seed one project with three artifacts (so style anchors are
/// non-empty).
fn seed_three(root: &Path) {
    write_collection(root, "requirements", "REQ");
    write_artifact(
        root,
        "requirements",
        "REQ-old-name",
        "11111111-1111-1111-1111-111111111111",
        "Pressure envelope",
    );
    write_artifact(
        root,
        "requirements",
        "REQ-valve-selection",
        "22222222-2222-2222-2222-222222222222",
        "Valve selection",
    );
    write_artifact(
        root,
        "requirements",
        "REQ-startup-sequence",
        "33333333-3333-3333-3333-333333333333",
        "Startup sequence",
    );
}

/// Build a ready app with the three seed artifacts and an optional
/// `llm` block. `None` boots the plain single-subject harness
/// (`LoadedSystem::Unnamed`); `Some(llm)` writes a named `system.json`
/// (mode 0600 on POSIX) carrying the block and boots via
/// `new_single_subject` with `system_config_path` set.
async fn app(llm: Option<Value>) -> (Router, Arc<AppState>, tempfile::TempDir) {
    let Some(llm) = llm else {
        return build_app(seed_three).await;
    };

    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    seed_three(&subject);

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
    let router = build_router(state.clone(), None);
    (router, state, temp)
}

fn chat_completion(content: &str) -> Value {
    json!({
        "choices": [{"message": {"content": content}}]
    })
}

// --- Single-artifact happy path -----------------------------------------

#[tokio::test]
async fn single_rename_suggestions_happy_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(
            "REQ-pressure-containment — aligns with sibling REQ-pressure-envelope\n\
             REQ-vessel-pressure — shorter restatement of the domain\n\
             REQ-pressure-boundary — standard systems-engineering phrasing",
        )))
        .mount(&server)
        .await;

    let (router, _, _temp) = app(Some(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ])))
    .await;

    let (status, body) = post_json(
        &router,
        "/api/artifacts/11111111-1111-1111-1111-111111111111/rename-suggestions",
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["kind"], "ok");
    let suggestions = body["suggestions"].as_array().unwrap();
    assert_eq!(suggestions.len(), 3);
    assert_eq!(suggestions[0]["name"], "REQ-pressure-containment");
    assert!(
        suggestions[0]["rationale"]
            .as_str()
            .unwrap()
            .starts_with("aligns with")
    );
    assert_eq!(body["servedByIndex"], 0);
    assert_eq!(body["servedBy"], "openai-compatible/gpt-4o-mini");
}

// --- Malformed LLM output surfaces as BAD_GATEWAY -----------------------

#[tokio::test]
async fn malformed_llm_output_returns_bad_gateway_with_typed_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(
            "sure thing\nhere are some ideas\nnothing parseable here",
        )))
        .mount(&server)
        .await;

    let (router, _, _temp) = app(Some(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ])))
    .await;

    let (status, body) = post_json(
        &router,
        "/api/artifacts/11111111-1111-1111-1111-111111111111/rename-suggestions",
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no usable suggestions")
    );
}

// --- Privacy-ack required arm ------------------------------------------

#[tokio::test]
async fn unacked_cloud_provider_returns_privacy_ack_required() {
    // Remote endpoint — never actually contacted because the privacy
    // gate trips first.
    let (router, _, _temp) = app(Some(json!([
        {
            "provider": "anthropic",
            "model": "claude-haiku-4-5",
            "endpoint": "https://api.anthropic.com",
            "apiKey": "secret",
        }
    ])))
    .await;

    let (status, body) = post_json(
        &router,
        "/api/artifacts/11111111-1111-1111-1111-111111111111/rename-suggestions",
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    assert_eq!(body["kind"], "privacyAckRequired");
    assert_eq!(body["indices"], json!([0]));
}

// --- No-LLM degrades gracefully to 200 with NoProviders ----------------

#[tokio::test]
async fn empty_llm_config_returns_no_providers_kind() {
    let (router, _, _temp) = app(None).await;
    let (status, body) = post_json(
        &router,
        "/api/artifacts/11111111-1111-1111-1111-111111111111/rename-suggestions",
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "noProviders");
}

// --- Bulk endpoint ------------------------------------------------------

#[tokio::test]
async fn bulk_returns_per_uuid_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(
            "REQ-alpha — first\nREQ-beta — second\nREQ-gamma — third",
        )))
        .mount(&server)
        .await;

    let (router, _, _temp) = app(Some(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ])))
    .await;

    let (status, body) = post_json(
        &router,
        "/api/projects/sample/rename-suggestions/bulk",
        &json!({
            "uuids": [
                "11111111-1111-1111-1111-111111111111",
                "22222222-2222-2222-2222-222222222222",
                "33333333-3333-3333-3333-333333333333",
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    for entry in results {
        assert_eq!(entry["kind"], "ok");
        assert_eq!(entry["suggestions"].as_array().unwrap().len(), 3);
    }
}

#[tokio::test]
async fn bulk_flags_unknown_uuids_as_not_found_without_failing_others() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(chat_completion("REQ-a — one\nREQ-b — two\nREQ-c — three")),
        )
        .mount(&server)
        .await;

    let (router, _, _temp) = app(Some(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ])))
    .await;

    let (status, body) = post_json(
        &router,
        "/api/projects/sample/rename-suggestions/bulk",
        &json!({
            "uuids": [
                "11111111-1111-1111-1111-111111111111",
                "99999999-9999-9999-9999-999999999999",
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    // Entry order is: NotFound entries first (built pre-spawn), then Ok
    // entries (built post-spawn).
    let kinds: Vec<&str> = results
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"ok"));
    assert!(kinds.contains(&"notFound"));
}

#[tokio::test]
async fn bulk_runs_concurrently_at_parallelism_four() {
    // Each request delays 300 ms. If the fanout were serial, four
    // requests would take ~1200 ms. At parallelism 4 they run
    // concurrently, so the whole run should fit well under 700 ms.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(chat_completion("REQ-a — one\nREQ-b — two\nREQ-c — three"))
                .set_delay(Duration::from_millis(300)),
        )
        .mount(&server)
        .await;

    let (router, _, temp) = app(Some(json!([
        {
            "provider": "openai-compatible",
            "model": "gpt-4o-mini",
            "endpoint": server.uri(),
            "apiKey": "secret",
        }
    ])))
    .await;
    let _ = temp;

    let start = std::time::Instant::now();
    let (status, body) = post_json(
        &router,
        "/api/projects/sample/rename-suggestions/bulk",
        &json!({
            "uuids": [
                "11111111-1111-1111-1111-111111111111",
                "22222222-2222-2222-2222-222222222222",
                "33333333-3333-3333-3333-333333333333",
            ]
        }),
    )
    .await;
    let elapsed = start.elapsed();
    assert_eq!(status, StatusCode::OK);
    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    // Three requests @ 300 ms each, concurrency 4 → ~300 ms plus
    // overhead. Generous upper bound (600 ms) avoids flakiness on slow
    // CI runners while still catching the serial-dispatch regression
    // (would be ~900 ms).
    assert!(
        elapsed < Duration::from_millis(600),
        "bulk endpoint appeared serial: elapsed {elapsed:?}"
    );
}

#[tokio::test]
async fn bulk_with_no_llm_returns_empty_results() {
    let (router, _, _temp) = app(None).await;
    let (status, body) = post_json(
        &router,
        "/api/projects/sample/rename-suggestions/bulk",
        &json!({
            "uuids": ["11111111-1111-1111-1111-111111111111"]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["results"], json!([]));
}
