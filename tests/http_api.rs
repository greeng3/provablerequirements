//! HTTP handler integration tests, ported from ReqForge's
//! `tests/http_api.rs` for #374 and adapted to provreq's
//! single-subject model.
//!
//! ReqForge seeded several sibling projects under a `mount_prefix`
//! and drove multi-project `discover_mounts`. provreq serves exactly
//! one repository (#370), so these tests go through the shared
//! single-subject harness in `tests/support/mod.rs`
//! (`new_single_subject` + `refresh()` / `discover_single`) rather
//! than a bespoke multi-project harness.
//!
//! Dropped tests (multi-mount only, no single-subject variant):
//! - `lists_mounts_with_validity_state_tags` — seeds needsInit/noGit
//!   sibling mounts; single-subject serves exactly one loaded mount
//!   (already covered by `tests/system_state.rs`).
//! - `post_mount_init_promotes_needs_init_to_project` — promotes a
//!   NeedsInit *sibling* mount; single-subject boots one
//!   already-initialised subject.
//! - `post_mount_init_still_routes_when_static_root_is_set` — the
//!   same sibling-init scenario, guarded against the SPA fallback.
//!   (The already-initialised → CONFLICT case survives as
//!   `post_mount_init_refuses_already_initialised_project`.)

mod support;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::write::OwnershipOverrides;
use serde_json::{Value, json};
use tower::util::ServiceExt;

use support::{
    SUBJECT_SLUG, build_app, build_app_with_artifacts, delete_json, get_json, post_json, put_json,
    test_config, write_artifact, write_collection, write_project,
};

/// A single-subject `AppState` that has NOT been published yet —
/// `is_ready()` is false until `publish`/`refresh` runs. The subject
/// on disk is a valid project so a later `refresh()` succeeds. The
/// returned `TempDir` must be kept alive for the test's duration.
fn not_ready_state() -> (Arc<AppState>, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    let state = Arc::new(AppState::new_single_subject(
        subject.clone(),
        test_config(&subject),
        OwnershipOverrides::default(),
    ));
    (state, temp)
}

/// PATCH counterpart to the harness's request helpers (the shared
/// module exposes get/post/put/delete but not patch).
async fn patch_json(router: &Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::patch(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

/// Static-file bundle seeding (a frontend `dist/`), separate from
/// subject seeding — ported verbatim from ReqForge.
fn seed_static_bundle(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("index.html"),
        "<!doctype html><html><body><div id=\"root\"></div></body></html>",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("assets/app.js"), "console.log('hi')").unwrap();
}

const UUID_A: &str = "0194f6d0-0001-7000-8000-000000000001";
const UUID_B: &str = "0194f6d0-0001-7000-8000-000000000002";

#[tokio::test]
async fn healthz_always_returns_ok() {
    let (state, _temp) = not_ready_state(); // deliberately not ready
    let router = build_router(state, None);
    let (status, body) = get_json(&router, "/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn readyz_is_503_before_publish_and_200_after() {
    let (state, _temp) = not_ready_state();
    let router = build_router(state.clone(), None);

    let (status, body) = get_json(&router, "/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["ready"], false);

    // Ready flips only on publish; refresh() runs discovery then
    // publishes the resulting World.
    state.refresh().await.unwrap();

    let (status, body) = get_json(&router, "/readyz").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ready"], true);
}

#[tokio::test]
async fn lists_projects_with_counts() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, "/api/projects").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["slug"], "sample");
    assert_eq!(arr[0]["collectionCount"], 1);
    assert_eq!(arr[0]["artifactCount"], 2);
}

#[tokio::test]
async fn project_detail_lists_collections() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, "/api/projects/sample").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["slug"], "sample");
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0]["prefix"], "REQ");
    assert_eq!(collections[0]["artifactCount"], 2);
}

#[tokio::test]
async fn unknown_project_slug_is_404() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, _) = get_json(&router, "/api/projects/does-not-exist").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lists_collections_and_single_collection() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, "/api/projects/sample/collections").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["prefix"], "REQ");

    let (status, body) = get_json(&router, "/api/projects/sample/collections/REQ").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["prefix"], "REQ");
    assert_eq!(body["artifactCount"], 2);
}

#[tokio::test]
async fn lists_artifacts_in_collection() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, "/api/projects/sample/collections/REQ/artifacts").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let names: Vec<&str> = arr.iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"REQ-a"));
    assert!(names.contains(&"REQ-b"));
}

#[tokio::test]
async fn gets_artifact_by_uuid() {
    let uuid = UUID_A;
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", uuid), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, &format!("/api/artifacts/{uuid}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["uuid"], uuid);
    assert_eq!(body["name"], "REQ-a");
    assert_eq!(body["projectSlug"], "sample");
    assert_eq!(body["collectionPrefix"], "REQ");
    assert_eq!(body["shape"], "content");
    assert!(body["body"].as_str().unwrap().contains("Body."));
}

#[tokio::test]
async fn unknown_uuid_is_404() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, _) = get_json(
        &router,
        "/api/artifacts/00000000-0000-7000-8000-000000000000",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// #374: dropped `lists_mounts_with_validity_state_tags` — multi-mount
// only (needsInit/noGit sibling mounts); see the module doc comment.

#[tokio::test]
async fn events_endpoint_opens_sse_stream() {
    // We don't read bytes from the stream — just verify the
    // response headers look right and the route is wired.
    let (state, _temp) = not_ready_state();
    let router = build_router(state, None);
    let response = router
        .oneshot(Request::get("/api/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/event-stream"),
        "expected text/event-stream, got {content_type}"
    );
    // Drop the body without draining it.
    drop(response.into_body().into_data_stream());
}

#[tokio::test]
async fn serves_index_html_for_root_when_static_root_is_set() {
    let static_root = tempfile::tempdir().unwrap();
    seed_static_bundle(static_root.path());
    let (_r, state, _t) = build_app_with_artifacts(&[]).await;
    let router = build_router(state.clone(), Some(static_root.path().to_path_buf()));

    let response = router
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<div id=\"root\">"), "got body: {body}");
}

#[tokio::test]
async fn serves_named_asset_when_it_exists_under_static_root() {
    let static_root = tempfile::tempdir().unwrap();
    seed_static_bundle(static_root.path());
    let (_r, state, _t) = build_app_with_artifacts(&[]).await;
    let router = build_router(state.clone(), Some(static_root.path().to_path_buf()));

    let response = router
        .oneshot(Request::get("/assets/app.js").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&bytes), "console.log('hi')");
}

#[tokio::test]
async fn falls_back_to_index_html_for_client_side_routes() {
    // SPA routing: /projects/sample is not a file on disk, but the
    // frontend router handles it. The backend must return
    // index.html so the SPA loads.
    let static_root = tempfile::tempdir().unwrap();
    seed_static_bundle(static_root.path());
    let (_r, state, _t) = build_app_with_artifacts(&[]).await;
    let router = build_router(state.clone(), Some(static_root.path().to_path_buf()));

    let response = router
        .oneshot(
            Request::get("/projects/sample/collections/REQ")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<div id=\"root\">"), "got body: {body}");
}

#[tokio::test]
async fn unknown_api_path_returns_json_404_even_with_static_root() {
    // Regression guard: the static fallback must not swallow
    // /api/* paths, otherwise clients get HTML back for JSON
    // errors and the error message is nonsensical.
    let static_root = tempfile::tempdir().unwrap();
    seed_static_bundle(static_root.path());
    let (_r, state, _t) = build_app_with_artifacts(&[]).await;
    let router = build_router(state.clone(), Some(static_root.path().to_path_buf()));

    let response = router
        .oneshot(Request::get("/api/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let ctype = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(
        ctype.starts_with("application/json"),
        "expected JSON, got {ctype}"
    );
}

#[tokio::test]
async fn no_static_root_means_unmatched_paths_return_404_as_before() {
    let (state, _temp) = not_ready_state();
    let router = build_router(state, None);

    let response = router
        .oneshot(
            Request::get("/projects/sample")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn static_root_without_index_html_is_ignored_gracefully() {
    let static_root = tempfile::tempdir().unwrap();
    // Create the directory but no index.html.
    std::fs::write(static_root.path().join("README.md"), "not a frontend").unwrap();
    let (_r, state, _t) = build_app_with_artifacts(&[]).await;
    let router = build_router(state.clone(), Some(static_root.path().to_path_buf()));

    // API still works.
    let response = router
        .clone()
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Unknown path still 404s rather than blowing up.
    let response = router
        .oneshot(
            Request::get("/projects/sample")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_artifact_updates_title_and_body_on_disk() {
    let uuid = UUID_A;
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-original", uuid), ("REQ-other", UUID_B)]).await;

    let body = json!({
        "title": "Updated title",
        "body": "# Updated body\n\nNew paragraph.\n",
        "tags": ["urgent", "phase-2"],
    });
    let (status, _) = put_json(&router, &format!("/api/artifacts/{uuid}"), &body).await;
    assert_eq!(status, StatusCode::OK);

    // On-disk round-trip: open the file and confirm the new title
    // and body landed.
    let disk = std::fs::read_to_string(
        temp.path()
            .join("sample/artifacts/requirements/REQ-original.md"),
    )
    .unwrap();
    assert!(disk.contains("\"title\": \"Updated title\""));
    assert!(disk.contains("# Updated body"));
    assert!(disk.contains("urgent"));
}

#[tokio::test]
async fn put_artifact_returns_404_for_unknown_uuid() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let body = json!({ "title": "x" });
    let (status, _) = put_json(
        &router,
        "/api/artifacts/00000000-0000-7000-8000-000000000000",
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_artifact_creates_a_new_content_artifact_on_disk() {
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let body = json!({
        "name": "REQ-new",
        "title": "A new requirement",
        "description": "Freshly minted.",
        "body": "# New\n\nBody paragraph.\n",
        "tags": ["new"],
    });
    let (status, detail) = post_json(
        &router,
        "/api/projects/sample/collections/REQ/artifacts",
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(detail["name"], "REQ-new");
    assert_eq!(detail["title"], "A new requirement");
    assert!(detail["uuid"].as_str().unwrap().len() >= 36);

    let path = temp.path().join("sample/artifacts/requirements/REQ-new.md");
    let disk = std::fs::read_to_string(path).unwrap();
    assert!(disk.contains("\"title\": \"A new requirement\""));
    assert!(disk.contains("# New"));
}

#[tokio::test]
async fn post_artifact_rejects_duplicate_name() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let body = json!({
        "name": "REQ-a",
        "title": "Collision",
    });
    let (status, _) = post_json(
        &router,
        "/api/projects/sample/collections/REQ/artifacts",
        &body,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn post_artifact_rejects_unsafe_names() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    for bad in ["../escape", "has space", "weird/slash", ""] {
        let body = json!({
            "name": bad,
            "title": "x",
        });
        let (status, _) = post_json(
            &router,
            "/api/projects/sample/collections/REQ/artifacts",
            &body,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "name {bad:?} should have been rejected"
        );
    }
}

#[tokio::test]
async fn delete_artifact_removes_the_file_and_updates_the_index() {
    let uuid = UUID_A;
    let (router, state, temp) =
        build_app_with_artifacts(&[("REQ-doomed", uuid), ("REQ-surviving", UUID_B)]).await;

    let (status, _) = delete_json(&router, &format!("/api/artifacts/{uuid}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let path = temp
        .path()
        .join("sample/artifacts/requirements/REQ-doomed.md");
    assert!(!path.exists(), "file should have been removed");

    let (status, _) = get_json(&router, &format!("/api/artifacts/{uuid}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let world = state.snapshot().await.unwrap();
    assert!(world.index.get(&uuid.parse().unwrap()).is_none());
}

#[tokio::test]
async fn delete_unknown_artifact_returns_404() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let (status, _) = delete_json(
        &router,
        "/api/artifacts/00000000-0000-7000-8000-000000000000",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn incoming_links_returns_empty_for_isolated_artifact() {
    let uuid = UUID_A;
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", uuid), ("REQ-b", UUID_B)]).await;
    let (status, body) = get_json(&router, &format!("/api/artifacts/{uuid}/incoming-links")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn incoming_links_finds_other_artifacts_that_link_to_target() {
    let (router, _state, _temp) = build_app(|subject| {
        write_collection(subject, "requirements", "REQ");
        write_artifact(subject, "requirements", "REQ-parent", UUID_A, "Parent");
        let fm = json!({
            "schemaVersion": 1,
            "uuid": UUID_B,
            "title": "Child",
            "shape": "content",
            "createdAt": "2026-04-18T00:00:00Z",
            "modifiedAt": "2026-04-18T00:00:00Z",
            "links": [{
                "targetUuid": UUID_A,
                "type": "derives-from",
                "hint": {
                    "projectSlug": "sample",
                    "collectionPrefix": "REQ",
                    "artifactName": "REQ-parent"
                }
            }],
            "reviewLog": []
        })
        .to_string();
        std::fs::write(
            subject.join("artifacts/requirements/REQ-child.md"),
            format!("---\n{fm}\n---\n# Child\n"),
        )
        .unwrap();
    })
    .await;

    let (status, body) =
        get_json(&router, &format!("/api/artifacts/{UUID_A}/incoming-links")).await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["artifactName"], "REQ-child");
    assert_eq!(arr[0]["linkType"], "derives-from");
}

#[tokio::test]
async fn put_artifact_bumps_modified_at() {
    let uuid = UUID_A;
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", uuid), ("REQ-b", UUID_B)]).await;

    // Initial modifiedAt from the fixture.
    let (status, before) = get_json(&router, &format!("/api/artifacts/{uuid}")).await;
    assert_eq!(status, StatusCode::OK);
    let before_modified = before["modifiedAt"].as_str().unwrap().to_owned();

    let body = json!({ "title": "bumped" });
    let (status, _) = put_json(&router, &format!("/api/artifacts/{uuid}"), &body).await;
    assert_eq!(status, StatusCode::OK);

    let (_, after) = get_json(&router, &format!("/api/artifacts/{uuid}")).await;
    let after_modified = after["modifiedAt"].as_str().unwrap().to_owned();
    assert_ne!(before_modified, after_modified);
}

#[tokio::test]
async fn patch_artifact_renames_within_collection_preserving_uuid() {
    let uuid = UUID_A;
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-original", uuid), ("REQ-other", UUID_B)]).await;

    let body = json!({ "name": "REQ-renamed" });
    let (status, detail) = patch_json(&router, &format!("/api/artifacts/{uuid}"), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["name"], "REQ-renamed");
    assert_eq!(detail["uuid"], uuid);

    assert!(
        !temp
            .path()
            .join("sample/artifacts/requirements/REQ-original.md")
            .exists(),
    );
    assert!(
        temp.path()
            .join("sample/artifacts/requirements/REQ-renamed.md")
            .exists(),
    );

    // By-UUID GET still works.
    let (status, _) = get_json(&router, &format!("/api/artifacts/{uuid}")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn patch_artifact_rejects_collision_with_existing_name() {
    let uuid = UUID_A;
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-original", uuid), ("REQ-taken", UUID_B)]).await;

    let body = json!({ "name": "REQ-taken" });
    let (status, _) = patch_json(&router, &format!("/api/artifacts/{uuid}"), &body).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn patch_artifact_noop_when_name_unchanged() {
    let uuid = UUID_A;
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-keeps-name", uuid), ("REQ-other", UUID_B)]).await;

    let body = json!({ "name": "REQ-keeps-name" });
    let (status, _) = patch_json(&router, &format!("/api/artifacts/{uuid}"), &body).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn post_collection_creates_a_new_collection_on_disk() {
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let body = json!({
        "dirName": "designs",
        "prefix": "DES",
        "name": "Designs",
        "description": "Design documents"
    });
    let (status, _) = post_json(&router, "/api/projects/sample/collections", &body).await;
    assert_eq!(status, StatusCode::CREATED);

    let cfg = temp
        .path()
        .join("sample/artifacts/designs/.collection.json");
    assert!(cfg.exists());
    let text = std::fs::read_to_string(cfg).unwrap();
    assert!(text.contains("\"prefix\": \"DES\""));
}

#[tokio::test]
async fn post_collection_rejects_duplicate_prefix() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let body = json!({
        "dirName": "other",
        "prefix": "REQ",
        "name": "Taken"
    });
    let (status, _) = post_json(&router, "/api/projects/sample/collections", &body).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_empty_collection_succeeds() {
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    // Create an empty collection first, then delete it.
    let body = json!({
        "dirName": "empty",
        "prefix": "EMP",
        "name": "Empty"
    });
    post_json(&router, "/api/projects/sample/collections", &body).await;

    let (status, _) = delete_json(&router, "/api/projects/sample/collections/EMP").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        !temp.path().join("sample/artifacts/empty").exists(),
        "collection directory should have been removed"
    );
}

#[tokio::test]
async fn delete_non_empty_collection_is_refused() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let (status, _) = delete_json(&router, "/api/projects/sample/collections/REQ").await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn wipe_project_artifacts_removes_all_collections_and_their_files() {
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    // Add a second collection so the loop is exercised.
    write_collection(&temp.path().join("sample"), "design", "DES");
    write_artifact(
        &temp.path().join("sample"),
        "design",
        "DES-a",
        "0194f6d0-0002-7000-8000-000000000001",
        "Design A",
    );

    let req_dir = temp.path().join("sample/artifacts/requirements");
    let des_dir = temp.path().join("sample/artifacts/design");
    let artifacts_root = temp.path().join("sample/artifacts");
    let reqforge_json = temp.path().join("sample/reqforge.json");
    assert!(req_dir.exists() && des_dir.exists());

    let (status, _) = delete_json(&router, "/api/projects/sample/artifacts").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(!req_dir.exists(), "REQ collection dir should be gone");
    assert!(!des_dir.exists(), "DES collection dir should be gone");
    assert!(
        artifacts_root.exists(),
        "artifacts/ root must remain or the project fails to load"
    );
    assert!(reqforge_json.exists(), "reqforge.json must be untouched");

    // Project still resolves and now reports zero collections.
    let (status, body) = get_json(&router, "/api/projects/sample").await;
    assert_eq!(status, StatusCode::OK);
    let collections = body["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 0);
}

#[tokio::test]
async fn wipe_with_deinit_also_removes_reqforge_json_and_artifacts_dir() {
    let (router, _state, temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let req_dir = temp.path().join("sample/artifacts/requirements");
    let artifacts_root = temp.path().join("sample/artifacts");
    let reqforge_json = temp.path().join("sample/reqforge.json");
    assert!(req_dir.exists() && reqforge_json.exists());

    let (status, _) = delete_json(&router, "/api/projects/sample/artifacts?deinit=true").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert!(!req_dir.exists(), "collection dir should be gone");
    assert!(
        !artifacts_root.exists(),
        "artifacts/ dir itself should be gone in deinit mode"
    );
    assert!(
        !reqforge_json.exists(),
        "reqforge.json should be gone in deinit mode"
    );

    // Mount reverts to NeedsInit since reqforge.json is gone.
    let (status, body) = get_json(&router, "/api/mounts").await;
    assert_eq!(status, StatusCode::OK);
    let entries = body.as_array().unwrap();
    let sample = entries
        .iter()
        .find(|m| m["dirName"] == "sample")
        .expect("sample mount must still be discoverable");
    assert_eq!(sample["state"], "needsInit");
}

#[tokio::test]
async fn wipe_unknown_project_returns_404() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;

    let (status, _) = delete_json(&router, "/api/projects/nope/artifacts").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn wipe_project_with_no_collections_is_a_noop() {
    // A project with reqforge.json + empty artifacts/, no collections.
    let (router, _state, temp) = build_app(|_subject| {}).await;

    let (status, _) = delete_json(&router, "/api/projects/sample/artifacts").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(temp.path().join("sample/artifacts").exists());
    assert!(temp.path().join("sample/reqforge.json").exists());
}

// #374: dropped `post_mount_init_promotes_needs_init_to_project` and
// `post_mount_init_still_routes_when_static_root_is_set` — both
// promote a NeedsInit *sibling* mount, which requires multi-mount
// discovery; see the module doc comment.

#[tokio::test]
async fn post_mount_init_refuses_already_initialised_project() {
    let (router, _state, _temp) =
        build_app_with_artifacts(&[("REQ-a", UUID_A), ("REQ-b", UUID_B)]).await;
    let body = json!({ "slug": "another", "name": "Another" });
    let (status, _) = post_json(&router, "/api/mounts/sample/init", &body).await;
    assert_eq!(status, StatusCode::CONFLICT);
}
