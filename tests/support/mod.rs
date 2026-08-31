//! Shared harness for the ported ReqForge HTTP handler integration
//! tests (#374). Drives the absorbed management API over
//! `tower::oneshot` — no real socket — against a single-subject
//! `AppState`.
//!
//! ReqForge's originals seeded several sibling projects under a
//! `mount_prefix` and ran multi-project `discover_mounts`. provreq
//! is single-subject (#370): one process serves exactly one
//! repository, so the harness seeds one subject at the tempdir root
//! and boots through `new_single_subject` + `refresh()`
//! (`discover_single`).
//!
//! Each `tests/<name>.rs` is its own binary and pulls in the whole
//! module, so helpers a given file doesn't touch read as dead code
//! there — allow it module-wide rather than per item.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::world::DiscoveryConfig;
use reqforge_model::write::OwnershipOverrides;
use serde_json::Value;
use tower::util::ServiceExt;

/// The slug every single-subject test project carries. In
/// single-subject mode there is exactly one, so tests build URLs
/// against this constant.
pub const SUBJECT_SLUG: &str = "sample";

/// Write a git + `reqforge.json` + empty `artifacts/` project at
/// `root`. Mirrors ReqForge's `write_project`; provreq classifies
/// the subject as an external repo (`reqforge.json` at the git
/// root), so `project_root == git_root` here.
pub fn write_project(root: &Path, slug: &str) {
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(
        root.join("reqforge.json"),
        serde_json::json!({
            "schemaVersion": 1,
            "slug": slug,
            "name": slug.to_uppercase(),
            "description": format!("{slug} description"),
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("artifacts")).unwrap();
}

/// Write an `artifacts/<dir>/.collection.json` under the subject.
pub fn write_collection(root: &Path, dir: &str, prefix: &str) {
    let path = root.join("artifacts").join(dir);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join(".collection.json"),
        serde_json::json!({
            "schemaVersion": 1,
            "prefix": prefix,
            "name": format!("{prefix} name"),
        })
        .to_string(),
    )
    .unwrap();
}

/// Write a content artifact (`<name>.md` with YAML front matter)
/// into a collection directory.
pub fn write_artifact(root: &Path, collection_dir: &str, name: &str, uuid: &str, title: &str) {
    let fm = serde_json::json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-18T00:00:00Z",
        "modifiedAt": "2026-04-18T00:00:00Z",
        "links": [],
        "reviewLog": [],
    })
    .to_string();
    std::fs::write(
        root.join("artifacts")
            .join(collection_dir)
            .join(format!("{name}.md")),
        format!("---\n{fm}\n---\n# {title}\n\nBody.\n"),
    )
    .unwrap();
}

/// A `DiscoveryConfig` pointing at `subject`, with the same limits
/// ReqForge's tests used. `workspace_dir: None` keeps thumbnails and
/// the blob workspace out of play.
pub fn test_config(subject: &Path) -> DiscoveryConfig {
    DiscoveryConfig {
        mount_prefix: subject.to_path_buf(),
        system_config_path: None,
        workspace_dir: None,
        max_blob_bytes: 50 * 1024 * 1024,
        thumbnail_cache_max_bytes: 500 * 1024 * 1024,
        external_url: None,
    }
}

/// Seed a single subject with `seed`, boot a ready single-subject
/// `AppState`, and wrap it in the management Router. `refresh()`
/// runs `discover_single` against the seeded subject so CRUD
/// endpoints round-trip on disk.
///
/// The returned `TempDir` must be kept alive for the duration of the
/// test — dropping it deletes the subject out from under the server.
pub async fn build_app(seed: impl FnOnce(&Path)) -> (Router, Arc<AppState>, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    seed(&subject);

    let state = Arc::new(AppState::new_single_subject(
        subject.clone(),
        test_config(&subject),
        OwnershipOverrides::default(),
    ));
    state.refresh().await.unwrap();
    let router = build_router(state.clone(), None);
    (router, state, temp)
}

/// The common case: one collection (`REQ`) holding `artifacts.len()`
/// content artifacts. Each tuple is `(name, uuid)`; titles are
/// `Title 0`, `Title 1`, …
pub async fn build_app_with_artifacts(
    artifacts: &[(&str, &str)],
) -> (Router, Arc<AppState>, tempfile::TempDir) {
    let artifacts = artifacts.to_vec();
    build_app(move |subject| {
        write_collection(subject, "requirements", "REQ");
        for (i, (name, uuid)) in artifacts.iter().enumerate() {
            write_artifact(subject, "requirements", name, uuid, &format!("Title {i}"));
        }
    })
    .await
}

async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

pub async fn get_json(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Request::get(uri).body(Body::empty()).unwrap()).await
}

pub async fn post_json(router: &Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    send(
        router,
        Request::post(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

pub async fn put_json(router: &Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    send(
        router,
        Request::put(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

pub async fn delete_json(router: &Router, uri: &str) -> (StatusCode, Value) {
    send(router, Request::delete(uri).body(Body::empty()).unwrap()).await
}
