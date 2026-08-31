//! Ported from ReqForge `tests/llm_provider_crud.rs` (#374 batch E):
//! the LLM provider CRUD endpoints (`POST/PUT/DELETE/PATCH
//! /api/llm/providers[/:index]`).
//!
//! Each test stands up a real tempdir-mounted `system.json` and
//! drives the CRUD endpoints. Because the handlers atomic-write back
//! to that file and then call `state.refresh()`, the assertions
//! cover both the wire response and the on-disk result.
//!
//! These boot via the multi-project `AppState::new` + `publish` path
//! (an empty mount prefix + a `system.json`), not the single-subject
//! `tests/support` harness — the endpoints operate on `world.system`,
//! not on mounted projects, and `refresh()` with no `subject_root`
//! reloads the system file after each write.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::index::UuidIndex;
use reqforge_model::system::{LoadedSystem, load_system_config};
use reqforge_model::world::{DiscoveryConfig, World};
use reqforge_model::write::OwnershipOverrides;
use serde_json::{Value, json};
use tower::util::ServiceExt;

fn write_system_json(path: &Path, llm: Value) {
    let body = json!({
        "schemaVersion": 2,
        "name": "test",
        "projects": [],
        "linkTypes": [],
        "llm": llm,
    });
    fs::write(path, body.to_string()).unwrap();
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// Build an app with a real on-disk system.json + a real (empty)
/// mount prefix. The mount prefix has to exist so `state.refresh()`
/// after a CRUD write succeeds — discovery walks it.
async fn app_from_system_file(path: &Path, mount_prefix: &Path) -> (Router, Arc<AppState>) {
    let system = load_system_config(Some(path)).expect("load system config");
    assert!(matches!(system, LoadedSystem::Named { .. }));

    let world = World {
        mounts: Vec::new(),
        index: UuidIndex::new(),
        duplicates: Vec::new(),
        system,
        missing_project_slugs: Vec::new(),
        link_catalog: reqforge_model::links::builtin_catalog().to_vec(),
        search_index: reqforge_model::search::empty_index(),
    };
    let state = Arc::new(AppState::new(
        DiscoveryConfig {
            mount_prefix: mount_prefix.to_path_buf(),
            system_config_path: Some(path.to_path_buf()),
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

async fn body_json(
    router: Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let body_bytes = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let request = builder.body(body_bytes).unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn post_appends_a_new_provider_and_persists_to_disk() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(&path, json!([]));
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(
        router.clone(),
        "POST",
        "/api/llm/providers",
        Some(json!({
            "provider": "openai-compatible",
            "model": "qwen2.5-coder:14b",
            "endpoint": "http://host.docker.internal:11434"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Wire-level verification.
    let (status, body) = body_json(router, "GET", "/api/llm/providers", None).await;
    assert_eq!(status, StatusCode::OK);
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["provider"], "openai-compatible");
    assert_eq!(providers[0]["model"], "qwen2.5-coder:14b");
    assert_eq!(providers[0]["enabled"], true);

    // On-disk verification: the file was atomic-rewritten with
    // the new entry and (on POSIX) carries mode 0600.
    let raw = fs::read_to_string(&path).unwrap();
    let cfg: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(cfg["llm"][0]["model"], "qwen2.5-coder:14b");
    #[cfg(unix)]
    {
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "system.json should stay mode 0600 after write");
    }
}

#[tokio::test]
async fn put_replaces_an_existing_provider() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            {
                "provider": "openai-compatible",
                "model": "old",
                "endpoint": "http://x.test"
            }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(
        router.clone(),
        "PUT",
        "/api/llm/providers/0",
        Some(json!({
            "provider": "anthropic",
            "model": "claude-haiku-4-5",
            "apiKey": "sk-test"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, body) = body_json(router, "GET", "/api/llm/providers", None).await;
    assert_eq!(body["providers"][0]["provider"], "anthropic");
    assert_eq!(body["providers"][0]["model"], "claude-haiku-4-5");
}

#[tokio::test]
async fn put_without_api_key_preserves_existing_key() {
    // Phase 13 merge-on-PUT: the wire never returns the apiKey
    // value to the frontend, so the Edit form can't re-supply it.
    // A PUT that omits apiKey must therefore preserve the
    // existing key on disk — otherwise editing the model name
    // (etc.) would silently wipe auth.
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            {
                "provider": "anthropic",
                "model": "claude-haiku-4-5",
                "apiKey": "sk-ant-original",
                "enabled": true
            }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    // PUT with no apiKey field — should merge to preserve.
    let (status, _) = body_json(
        router.clone(),
        "PUT",
        "/api/llm/providers/0",
        Some(json!({
            "provider": "anthropic",
            "model": "claude-sonnet-4-6"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let raw = fs::read_to_string(&path).unwrap();
    let cfg: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(cfg["llm"][0]["model"], "claude-sonnet-4-6");
    // Key preserved despite not being in the request body.
    assert_eq!(cfg["llm"][0]["apiKey"], "sk-ant-original");
    // Enabled preserved too.
    assert_eq!(cfg["llm"][0]["enabled"], true);
}

#[tokio::test]
async fn put_with_explicit_api_key_replaces_it() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            {
                "provider": "anthropic",
                "model": "x",
                "apiKey": "sk-old"
            }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(
        router.clone(),
        "PUT",
        "/api/llm/providers/0",
        Some(json!({
            "provider": "anthropic",
            "model": "x",
            "apiKey": "sk-new"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let cfg: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(cfg["llm"][0]["apiKey"], "sk-new");
}

#[tokio::test]
async fn delete_drops_the_provider_at_index() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            { "provider": "openai-compatible", "model": "a", "endpoint": "http://a.test" },
            { "provider": "anthropic", "model": "b", "apiKey": "k" }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(router.clone(), "DELETE", "/api/llm/providers/0", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, body) = body_json(router, "GET", "/api/llm/providers", None).await;
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["provider"], "anthropic");
}

#[tokio::test]
async fn patch_toggles_the_enabled_flag() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            { "provider": "openai-compatible", "model": "a", "endpoint": "http://a.test" }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(
        router.clone(),
        "PATCH",
        "/api/llm/providers/0",
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, body) = body_json(router, "GET", "/api/llm/providers", None).await;
    assert_eq!(body["providers"][0]["enabled"], false);
}

#[tokio::test]
async fn patch_moves_a_provider_to_a_new_position() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(
        &path,
        json!([
            { "provider": "openai-compatible", "model": "a", "endpoint": "http://a.test" },
            { "provider": "anthropic", "model": "b", "apiKey": "k" },
            { "provider": "gemini", "model": "c", "apiKey": "k" }
        ]),
    );
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    // Move index 2 to position 0.
    let (status, _) = body_json(
        router.clone(),
        "PATCH",
        "/api/llm/providers/2",
        Some(json!({ "position": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, body) = body_json(router, "GET", "/api/llm/providers", None).await;
    let providers = body["providers"].as_array().unwrap();
    assert_eq!(providers[0]["model"], "c");
    assert_eq!(providers[1]["model"], "a");
    assert_eq!(providers[2]["model"], "b");
}

#[tokio::test]
async fn post_with_unknown_provider_family_is_400() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(&path, json!([]));
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, body) = body_json(
        router,
        "POST",
        "/api/llm/providers",
        Some(json!({ "provider": "magic-llm", "model": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["error"].as_str().unwrap_or("").contains("magic-llm"),
        "expected error to name the bad provider, got {body}",
    );
}

#[tokio::test]
async fn put_with_out_of_range_index_returns_404() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("system.json");
    write_system_json(&path, json!([]));
    let (router, _state) = app_from_system_file(&path, temp.path()).await;

    let (status, _) = body_json(
        router,
        "PUT",
        "/api/llm/providers/99",
        Some(json!({ "provider": "anthropic", "model": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_when_no_system_config_loaded_returns_409() {
    // Build an app whose LoadedSystem is Unnamed.
    let world = World {
        mounts: Vec::new(),
        index: UuidIndex::new(),
        duplicates: Vec::new(),
        system: LoadedSystem::Unnamed,
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
    let router = build_router(state, None);

    let (status, body) = body_json(
        router,
        "POST",
        "/api/llm/providers",
        Some(json!({ "provider": "anthropic", "model": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("system config")
    );
}
