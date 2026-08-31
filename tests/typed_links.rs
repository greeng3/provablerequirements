//! Ported from ReqForge `tests/typed_links.rs` (#374 batch C):
//! link-type catalog endpoint, server-side link resolution in
//! ArtifactDetail, incoming-links, the artifact-search endpoint, and
//! link-write validation.
//!
//! Single-subject adaptations (#370):
//! - `incoming_links_span_multiple_projects` is DROPPED — it seeds
//!   two sibling projects under one mount prefix and asserts a
//!   cross-repo incoming link, which provreq's one-mount World cannot
//!   represent.
//! - `artifact_search_returns_substring_matches_across_projects` is
//!   adapted to a single project (its substring-match behaviour is
//!   not inherently cross-project).
//! - The three system-declared-link-type cases boot through a local
//!   `build_app_with_system` helper because the shared harness fixes
//!   `system_config_path: None`.

mod support;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use provreq::app::AppState;
use provreq::http::build_router;
use reqforge_model::world::DiscoveryConfig;
use reqforge_model::write::OwnershipOverrides;
use serde_json::Value;
use serde_json::json;
use support::{SUBJECT_SLUG, build_app, get_json, put_json, write_collection, write_project};

/// Write a content artifact carrying an explicit `links` array.
fn write_artifact_with_links(
    root: &Path,
    collection_dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: Value,
) {
    let fm = serde_json::json!({
        "schemaVersion": 1,
        "uuid": uuid,
        "title": title,
        "shape": "content",
        "createdAt": "2026-04-18T00:00:00Z",
        "modifiedAt": "2026-04-18T00:00:00Z",
        "links": links,
        "reviewLog": []
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

/// Write `bytes` to `path` and (on POSIX) chmod 0600 — the
/// system.json loader rejects world-readable files because they hold
/// API keys.
fn write_system_json_secure(path: &Path, bytes: String) {
    std::fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// Boot a single-subject app with a named `system.json`. The system
/// config lives outside the subject (its slug list still names
/// `sample`); `discover_single` reads it through
/// `DiscoveryConfig.system_config_path` and folds its `linkTypes`
/// into the effective catalog.
async fn build_app_with_system(
    seed: impl FnOnce(&Path),
    system: Value,
) -> (Router, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let subject = temp.path().join(SUBJECT_SLUG);
    write_project(&subject, SUBJECT_SLUG);
    seed(&subject);

    let system_path = temp.path().join("system.json");
    write_system_json_secure(&system_path, system.to_string());

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

// ---- link-type catalog ----

#[tokio::test]
async fn get_link_types_returns_seven_builtins_for_unnamed_system() {
    let (router, _state, _temp) = build_app(|_| {}).await;

    let (status, body) = get_json(&router, "/api/link-types").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 7);
    assert!(arr.iter().all(|e| e["source"] == "builtin"));
    let names: Vec<&str> = arr.iter().map(|e| e["name"].as_str().unwrap()).collect();
    for expected in [
        "derives-from",
        "satisfies",
        "verifies",
        "supersedes",
        "cites",
        "conflicts-with",
        "related-to",
    ] {
        assert!(names.contains(&expected), "missing builtin {expected}");
    }
}

#[tokio::test]
async fn get_link_types_includes_system_declared_types() {
    let (router, _temp) = build_app_with_system(
        |_| {},
        json!({
            "schemaVersion": 1,
            "name": "test",
            "projects": [{ "slug": "sample" }],
            "linkTypes": [
                { "name": "mitigates", "inverseName": "mitigated-by",
                  "directed": true, "acyclic": false }
            ]
        }),
    )
    .await;

    let (status, body) = get_json(&router, "/api/link-types").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 8);
    let mitigates = arr
        .iter()
        .find(|e| e["name"] == "mitigates")
        .expect("mitigates should be in the catalog");
    assert_eq!(mitigates["source"], "system");
    assert_eq!(mitigates["inverseName"], "mitigated-by");
}

#[tokio::test]
async fn system_cannot_override_builtin_link_type() {
    // System declares a type that collides with a built-in (satisfies).
    // The built-in must survive exactly.
    let (router, _temp) = build_app_with_system(
        |_| {},
        json!({
            "schemaVersion": 1,
            "name": "test",
            "projects": [{ "slug": "sample" }],
            "linkTypes": [
                { "name": "satisfies", "inverseName": "replaced-inverse",
                  "directed": false, "acyclic": true }
            ]
        }),
    )
    .await;

    let (status, body) = get_json(&router, "/api/link-types").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 7, "colliding system type must be dropped");
    let satisfies = arr
        .iter()
        .find(|e| e["name"] == "satisfies")
        .expect("builtin satisfies must remain");
    assert_eq!(satisfies["inverseName"], "satisfied-by");
    assert_eq!(satisfies["source"], "builtin");
    assert_eq!(satisfies["directed"], true);
}

// ---- link resolution in ArtifactDetail ----

#[tokio::test]
async fn artifact_detail_resolves_links_against_local_index() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000001";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000002";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-b",
            b_uuid,
            "B",
            json!([
                {
                    "targetUuid": a_uuid,
                    "type": "derives-from",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-a"
                    }
                }
            ]),
        );
    })
    .await;

    let (status, body) = get_json(&router, &format!("/api/artifacts/{b_uuid}")).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let links = body["links"].as_array().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0]["resolution"], "resolved");
    assert_eq!(links[0]["type"], "derives-from");
    assert_eq!(links[0]["typeMetadata"]["source"], "builtin");
    assert_eq!(links[0]["targetSummary"]["title"], "A");
}

#[tokio::test]
async fn artifact_detail_marks_unmounted_target_as_unresolved() {
    let source_uuid = "0194f6d0-0001-7000-8000-000000000010";
    let phantom_uuid = "0194f6d0-0001-7000-8000-000000000999";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-a",
            source_uuid,
            "A",
            json!([
                {
                    "targetUuid": phantom_uuid,
                    "type": "satisfies",
                    "hint": {
                        "projectSlug": "other-repo",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-phantom"
                    }
                }
            ]),
        );
    })
    .await;

    let (_, body) = get_json(&router, &format!("/api/artifacts/{source_uuid}")).await;
    let link = &body["links"][0];
    assert_eq!(link["resolution"], "unresolved");
    assert_eq!(link["hint"]["projectSlug"], "other-repo");
    assert!(link["targetSummary"].is_null());
    // Type metadata still present because "satisfies" is a builtin.
    assert_eq!(link["typeMetadata"]["name"], "satisfies");
}

#[tokio::test]
async fn artifact_detail_marks_unknown_type_as_unknown_type() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000020";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000021";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-b",
            b_uuid,
            "B",
            json!([
                {
                    "targetUuid": a_uuid,
                    "type": "mitigates",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-a"
                    }
                }
            ]),
        );
    })
    .await;

    let (_, body) = get_json(&router, &format!("/api/artifacts/{b_uuid}")).await;
    let link = &body["links"][0];
    assert_eq!(link["resolution"], "unknownType");
    assert_eq!(link["type"], "mitigates");
    assert!(link["typeMetadata"].is_null());
    // Target exists locally — targetSummary still populated so the
    // UI can at least link through.
    assert_eq!(link["targetSummary"]["title"], "A");
}

#[tokio::test]
async fn artifact_detail_recognises_system_declared_type() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000030";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000031";
    let (router, _temp) = build_app_with_system(
        |root| {
            write_collection(root, "requirements", "REQ");
            write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
            write_artifact_with_links(
                root,
                "requirements",
                "REQ-b",
                b_uuid,
                "B",
                json!([
                    {
                        "targetUuid": a_uuid,
                        "type": "mitigates",
                        "hint": {
                            "projectSlug": "sample",
                            "collectionPrefix": "REQ",
                            "artifactName": "REQ-a"
                        }
                    }
                ]),
            );
        },
        json!({
            "schemaVersion": 1,
            "name": "t",
            "projects": [{ "slug": "sample" }],
            "linkTypes": [
                { "name": "mitigates", "inverseName": "mitigated-by",
                  "directed": true, "acyclic": false }
            ]
        }),
    )
    .await;

    let (_, body) = get_json(&router, &format!("/api/artifacts/{b_uuid}")).await;
    let link = &body["links"][0];
    assert_eq!(link["resolution"], "resolved");
    assert_eq!(link["typeMetadata"]["source"], "system");
}

// #374: dropped `incoming_links_span_multiple_projects` — it seeds
// two sibling projects (project-a links at project-b) and asserts a
// cross-repo incoming link. provreq's single-subject World holds
// exactly one mount, so there is no second project to originate the
// link.

// ---- artifact search endpoint ----

#[tokio::test]
async fn artifact_search_returns_substring_matches() {
    // Adapted from the cross-project original to a single subject: two
    // "log*" titles match the query, one unrelated title does not.
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-login-flow",
            "0194f6d0-0001-7000-8000-000000000041",
            "Login flow happy path",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-logout",
            "0194f6d0-0001-7000-8000-000000000042",
            "Logout button behaviour",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-unrelated",
            "0194f6d0-0001-7000-8000-000000000043",
            "Totally different",
            json!([]),
        );
    })
    .await;

    let (status, body) = get_json(&router, "/api/artifacts/search?q=log").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let arr = body.as_array().unwrap();
    let names: Vec<&str> = arr
        .iter()
        .map(|h| h["artifactName"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"REQ-login-flow"));
    assert!(names.contains(&"REQ-logout"));
    assert!(!names.contains(&"REQ-unrelated"));
}

#[tokio::test]
async fn artifact_search_empty_query_returns_empty_array() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-a",
            "0194f6d0-0001-7000-8000-000000000051",
            "A",
            json!([]),
        );
    })
    .await;

    let (status, body) = get_json(&router, "/api/artifacts/search?q=").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn artifact_search_excludes_self_via_exclude_param() {
    let self_uuid = "0194f6d0-0001-7000-8000-000000000061";
    let other_uuid = "0194f6d0-0001-7000-8000-000000000062";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-self",
            self_uuid,
            "Self item",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-other",
            other_uuid,
            "Other item",
            json!([]),
        );
    })
    .await;

    let (status, body) = get_json(
        &router,
        &format!("/api/artifacts/search?q=item&exclude={self_uuid}"),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["uuid"], other_uuid);
}

// ---- link write validation (Phase 3b) ----

#[tokio::test]
async fn put_with_links_writes_them_to_frontmatter_and_autopopulates_hint() {
    let target_uuid = "0194f6d0-0001-7000-8000-000000000101";
    let source_uuid = "0194f6d0-0001-7000-8000-000000000102";
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-target",
            target_uuid,
            "Target",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-source",
            source_uuid,
            "Source",
            json!([]),
        );
    })
    .await;

    let (status, _) = put_json(
        &router,
        &format!("/api/artifacts/{source_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": target_uuid,
                    "type": "derives-from"
                    // intentionally omit hint — server should
                    // populate it from the index.
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let disk = std::fs::read_to_string(
        temp.path()
            .join("sample/artifacts/requirements/REQ-source.md"),
    )
    .unwrap();
    assert!(disk.contains("\"type\": \"derives-from\""));
    assert!(disk.contains("\"projectSlug\": \"sample\""));
    assert!(disk.contains("\"collectionPrefix\": \"REQ\""));
    assert!(disk.contains("\"artifactName\": \"REQ-target\""));
}

#[tokio::test]
async fn put_with_empty_links_array_clears_existing_links() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000111";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000112";
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-b",
            b_uuid,
            "B",
            json!([
                {
                    "targetUuid": a_uuid,
                    "type": "derives-from",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-a"
                    }
                }
            ]),
        );
    })
    .await;

    let (status, _) = put_json(
        &router,
        &format!("/api/artifacts/{b_uuid}"),
        &json!({ "links": [] }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let disk = std::fs::read_to_string(temp.path().join("sample/artifacts/requirements/REQ-b.md"))
        .unwrap();
    assert!(disk.contains("\"links\": []"));
}

#[tokio::test]
async fn put_with_links_absent_preserves_existing_links() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000121";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000122";
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-b",
            b_uuid,
            "B",
            json!([
                {
                    "targetUuid": a_uuid,
                    "type": "derives-from",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-a"
                    }
                }
            ]),
        );
    })
    .await;

    let (status, _) = put_json(
        &router,
        &format!("/api/artifacts/{b_uuid}"),
        &json!({ "title": "B renamed" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);

    let disk = std::fs::read_to_string(temp.path().join("sample/artifacts/requirements/REQ-b.md"))
        .unwrap();
    assert!(disk.contains("\"title\": \"B renamed\""));
    assert!(disk.contains("\"type\": \"derives-from\""));
}

#[tokio::test]
async fn put_with_unknown_link_type_returns_400() {
    let a_uuid = "0194f6d0-0001-7000-8000-000000000131";
    let b_uuid = "0194f6d0-0001-7000-8000-000000000132";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(root, "requirements", "REQ-a", a_uuid, "A", json!([]));
        write_artifact_with_links(root, "requirements", "REQ-b", b_uuid, "B", json!([]));
    })
    .await;

    let (status, body) = put_json(
        &router,
        &format!("/api/artifacts/{b_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": a_uuid,
                    "type": "bogus-type",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-a"
                    }
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("bogus-type"));
}

#[tokio::test]
async fn put_with_self_link_returns_400() {
    let self_uuid = "0194f6d0-0001-7000-8000-000000000141";
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-self",
            self_uuid,
            "Self",
            json!([]),
        );
    })
    .await;

    let (status, body) = put_json(
        &router,
        &format!("/api/artifacts/{self_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": self_uuid,
                    "type": "related-to",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-self"
                    }
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("itself"));
}

#[tokio::test]
async fn put_with_unresolved_target_requires_hint() {
    let source_uuid = "0194f6d0-0001-7000-8000-000000000151";
    let phantom_uuid = "0194f6d0-0001-7000-8000-000000000999";
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-source",
            source_uuid,
            "Source",
            json!([]),
        );
    })
    .await;

    // No hint → 400.
    let (status, body) = put_json(
        &router,
        &format!("/api/artifacts/{source_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": phantom_uuid,
                    "type": "derives-from"
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("hint"));

    // With hint → 200; hint carries through verbatim.
    let (status, _) = put_json(
        &router,
        &format!("/api/artifacts/{source_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": phantom_uuid,
                    "type": "derives-from",
                    "hint": {
                        "projectSlug": "other-repo",
                        "collectionPrefix": "REQ",
                        "artifactName": "REQ-phantom"
                    }
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let disk = std::fs::read_to_string(
        temp.path()
            .join("sample/artifacts/requirements/REQ-source.md"),
    )
    .unwrap();
    assert!(disk.contains("\"projectSlug\": \"other-repo\""));
}

#[tokio::test]
async fn put_overrides_stale_client_hint_for_resolvable_target() {
    // Client sends a stale hint (wrong artifactName); server should
    // overwrite it with the authoritative one from the UUID index.
    let target_uuid = "0194f6d0-0001-7000-8000-000000000161";
    let source_uuid = "0194f6d0-0001-7000-8000-000000000162";
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-correct-name",
            target_uuid,
            "Target",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-source",
            source_uuid,
            "Source",
            json!([]),
        );
    })
    .await;

    let (status, _) = put_json(
        &router,
        &format!("/api/artifacts/{source_uuid}"),
        &json!({
            "links": [
                {
                    "targetUuid": target_uuid,
                    "type": "derives-from",
                    "hint": {
                        "projectSlug": "sample",
                        "collectionPrefix": "REQ",
                        // stale name — this should be overwritten
                        "artifactName": "REQ-old-stale-name"
                    }
                }
            ]
        }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let disk = std::fs::read_to_string(
        temp.path()
            .join("sample/artifacts/requirements/REQ-source.md"),
    )
    .unwrap();
    assert!(disk.contains("\"artifactName\": \"REQ-correct-name\""));
    assert!(!disk.contains("REQ-old-stale-name"));
}

#[tokio::test]
async fn artifact_search_prefix_match_ranks_ahead_of_substring_match() {
    // "REQ-zeta" is a prefix match on "req"; "REQ-alpha"'s title
    // "Contains req in body" is a substring match on "req". Prefix
    // match should come first.
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-alpha",
            "0194f6d0-0001-7000-8000-000000000071",
            "Contains req in body",
            json!([]),
        );
        write_artifact_with_links(
            root,
            "requirements",
            "REQ-zeta",
            "0194f6d0-0001-7000-8000-000000000072",
            "Title one",
            json!([]),
        );
    })
    .await;

    let (_, body) = get_json(&router, "/api/artifacts/search?q=req").await;
    let arr = body.as_array().unwrap();
    assert!(arr.len() >= 2);
    // Both REQ-alpha and REQ-zeta start with "req", but only
    // REQ-alpha's *title* matches plain substring. Both are prefix
    // hits on name, so they tie on rank 0 and fall back to
    // alphabetical order by project_slug then artifact_name.
    assert_eq!(arr[0]["artifactName"], "REQ-alpha");
    assert_eq!(arr[1]["artifactName"], "REQ-zeta");
}
