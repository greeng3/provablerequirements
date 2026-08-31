//! Report catalog endpoint integration tests, ported from ReqForge's
//! `tests/reports.rs` for #374 and adapted to provreq's
//! single-subject model.
//!
//! ReqForge seeded several sibling projects under a `mount_prefix`
//! and drove multi-project `discover_mounts`. provreq serves exactly
//! one repository (#370), so these tests go through the shared
//! single-subject harness in `tests/support/mod.rs`
//! (`new_single_subject` + `refresh()` / `discover_single`). Each
//! ReqForge test seeded its project under `prefix/sample`; here the
//! seed closure writes directly at the subject root (the harness has
//! already written git + `reqforge.json` there).
//!
//! Dropped tests: none — every case is expressible single-subject.
//! `scope_for_unmounted_project_returns_404` is kept: it targets an
//! unknown slug (`project:nowhere`), which is still a 404 with one
//! loaded subject.

mod support;

use std::path::Path;

use serde_json::{Value, json};

use support::{
    build_app, build_app_with_workspace, delete_json, get_json, put_json, write_collection,
};

const UUID_REQ_A: &str = "0194f6d0-0006-7000-8000-00000000aaaa";
const UUID_REQ_B: &str = "0194f6d0-0006-7000-8000-00000000bbbb";
const UUID_DES_A: &str = "0194f6d0-0006-7000-8000-00000000dddd";
/// Points at an artifact that is not loaded in the subject.
const UUID_GHOST: &str = "0194f6d0-0006-7000-8000-000000009999";

fn write_content(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: Value,
    active: Option<bool>,
) {
    write_content_with_log(root, dir, name, uuid, title, links, json!([]), active);
}

#[allow(clippy::too_many_arguments)]
fn write_content_with_log(
    root: &Path,
    dir: &str,
    name: &str,
    uuid: &str,
    title: &str,
    links: Value,
    review_log: Value,
    active: Option<bool>,
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
    if let Some(flag) = active {
        meta["active"] = Value::Bool(flag);
    }
    let path = root.join("artifacts").join(dir).join(format!("{name}.md"));
    std::fs::write(&path, format!("---\n{}\n---\nbody text\n", meta)).unwrap();
}

fn hint(slug: &str, prefix: &str, name: &str) -> Value {
    json!({
        "projectSlug": slug,
        "collectionPrefix": prefix,
        "artifactName": name,
    })
}

/// Seed the shared sample fixture into the subject root: two
/// collections (REQ, DES), a resolvable REQ-a → REQ-b link, and a
/// DES-ghost artifact whose link dangles (target-missing).
fn fixture_sample_project(root: &Path) {
    write_collection(root, "requirements", "REQ");
    write_collection(root, "designs", "DES");

    // REQ-a links to REQ-b (resolvable).
    write_content(
        root,
        "requirements",
        "REQ-a",
        UUID_REQ_A,
        "A",
        json!([
            {
                "targetUuid": UUID_REQ_B,
                "type": "derives-from",
                "hint": hint("sample", "REQ", "REQ-b"),
            }
        ]),
        None,
    );
    // REQ-b has no outgoing links, but is targeted by REQ-a above.
    write_content(
        root,
        "requirements",
        "REQ-b",
        UUID_REQ_B,
        "B",
        json!([]),
        None,
    );
    // DES-ghost points at UUID_GHOST (unresolved; hint mentions the
    // currently-mounted project so the reason is "target-missing").
    write_content(
        root,
        "designs",
        "DES-ghost",
        UUID_DES_A,
        "Design pointing nowhere",
        json!([
            {
                "targetUuid": UUID_GHOST,
                "type": "satisfies",
                "hint": hint("sample", "REQ", "REQ-ghost"),
            }
        ]),
        None,
    );
}

#[tokio::test]
async fn unresolved_links_report_finds_dangling_targets_with_correct_reason() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, body) = get_json(&router, "/api/reports/unresolved-links").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "unresolved-links");
    assert_eq!(body["totalUnresolved"], 1);
    let entry = &body["entries"][0];
    assert_eq!(entry["sourceArtifactName"], "DES-ghost");
    assert_eq!(entry["targetUuid"], UUID_GHOST);
    assert_eq!(entry["reason"], "target-missing");
}

#[tokio::test]
async fn unresolved_links_scoped_to_one_collection_omits_others() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, body) = get_json(
        &router,
        "/api/reports/unresolved-links?scope=collection:sample/REQ",
    )
    .await;
    assert_eq!(status, 200);
    // DES-ghost is the only unresolved link in the whole world;
    // scoping to REQ eliminates it.
    assert_eq!(body["totalUnresolved"], 0);
    assert_eq!(body["scope"]["kind"], "collection");
    assert_eq!(body["scope"]["prefix"], "REQ");
}

#[tokio::test]
async fn link_orphans_reports_artifact_with_no_in_or_out_links() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_content(
            root,
            "requirements",
            "REQ-solo",
            UUID_REQ_A,
            "Solo",
            json!([]),
            None,
        );
    })
    .await;
    let (status, body) = get_json(&router, "/api/reports/link-orphans").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "link-orphans");
    assert_eq!(body["totalOrphans"], 1);
    assert_eq!(body["entries"][0]["artifactName"], "REQ-solo");
}

#[tokio::test]
async fn unknown_report_kind_returns_404() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, _) = get_json(&router, "/api/reports/made-up-kind").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn scope_for_unmounted_project_returns_404() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, _) = get_json(
        &router,
        "/api/reports/unresolved-links?scope=project:nowhere",
    )
    .await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn malformed_scope_string_returns_400() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, _) = get_json(&router, "/api/reports/unresolved-links?scope=garbage").await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn review_status_report_aggregates_totals_and_facets() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        // Three artifacts: approved, never-reviewed, rejected.
        let log_approved = json!([{
            "timestamp": "2026-04-22T12:00:00Z",
            "reviewer": "alice",
            "outcome": "approved",
        }]);
        let log_rejected = json!([{
            "timestamp": "2026-04-22T12:00:00Z",
            "reviewer": "alice",
            "outcome": "rejected",
        }]);
        write_content_with_log(
            root,
            "requirements",
            "REQ-ok",
            UUID_REQ_A,
            "OK",
            json!([]),
            log_approved,
            None,
        );
        write_content(
            root,
            "requirements",
            "REQ-fresh",
            UUID_REQ_B,
            "Fresh",
            json!([]),
            None,
        );
        write_content_with_log(
            root,
            "requirements",
            "REQ-nope",
            UUID_DES_A,
            "Nope",
            json!([]),
            log_rejected,
            None,
        );
    })
    .await;

    let (status, body) = get_json(&router, "/api/reports/review-status").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "review-status");
    assert_eq!(body["totals"]["approved"], 1);
    assert_eq!(body["totals"]["rejected"], 1);
    assert_eq!(body["totals"]["neverReviewed"], 1);
    assert_eq!(body["byShape"]["content"]["approved"], 1);
}

#[tokio::test]
async fn filesystem_orphans_report_surfaces_both_sides() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "designs", "DES");
        // Binary without sidecar.
        std::fs::write(
            root.join("artifacts/designs/DES-logo.png"),
            b"\x89PNG\r\n\x1a\n",
        )
        .unwrap();
        // Sidecar whose blobPath doesn't resolve.
        let ghost_sidecar = json!({
            "schemaVersion": 1,
            "uuid": UUID_DES_A,
            "title": "Ghost",
            "shape": "blob",
            "createdAt": "2026-04-22T00:00:00Z",
            "modifiedAt": "2026-04-22T00:00:00Z",
            "links": [],
            "reviewLog": [],
            "blobPath": "artifacts/designs/DES-ghost.pdf",
        });
        std::fs::write(
            root.join("artifacts/designs/DES-ghost.pdf.reqforge.json"),
            ghost_sidecar.to_string(),
        )
        .unwrap();
    })
    .await;

    let (status, body) = get_json(&router, "/api/reports/filesystem-orphans").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "filesystem-orphans");
    let missing_sidecar = body["missingSidecar"].as_array().unwrap();
    assert_eq!(missing_sidecar.len(), 1);
    assert_eq!(missing_sidecar[0]["filename"], "DES-logo.png");
    let missing_binary = body["missingBinary"].as_array().unwrap();
    assert_eq!(missing_binary.len(), 1);
    assert_eq!(
        missing_binary[0]["sidecarFilename"],
        "DES-ghost.pdf.reqforge.json"
    );
}

#[tokio::test]
async fn adopt_orphan_blob_writes_sidecar_and_returns_created_artifact() {
    let (router, _state, temp) = build_app(|root| {
        write_collection(root, "designs", "DES");
        std::fs::write(
            root.join("artifacts/designs/DES-logo.png"),
            b"\x89PNG\r\n\x1a\nstub",
        )
        .unwrap();
    })
    .await;

    let (status, _) = support::post_json(
        &router,
        "/api/projects/sample/collections/DES/artifacts/blob/adopt",
        &json!({
            "name": "DES-logo",
            "title": "Adopted logo",
            "binaryRelativePath": "artifacts/designs/DES-logo.png",
        }),
    )
    .await;
    assert_eq!(status, 201);
    // Sidecar should now exist alongside the binary.
    assert!(
        temp.path()
            .join("sample/artifacts/designs/DES-logo.png.reqforge.json")
            .exists()
    );
}

#[tokio::test]
async fn adopt_orphan_blob_rejects_path_outside_collection() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "designs", "DES");
    })
    .await;

    let (status, _) = support::post_json(
        &router,
        "/api/projects/sample/collections/DES/artifacts/blob/adopt",
        &json!({
            "name": "DES-escape",
            "title": "Escape",
            "binaryRelativePath": "../outside.png",
        }),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn saved_config_put_then_get_round_trips_the_opaque_json() {
    let workspace = tempfile::tempdir().unwrap();
    let (router, _state, _temp) =
        build_app_with_workspace(fixture_sample_project, Some(workspace.path().to_path_buf()))
            .await;

    // PUT the config.
    let (put_status, _) = put_json(
        &router,
        "/api/reports/unresolved-links/config",
        &json!({
            "scope": "collection:sample/REQ",
            "includeInactive": true,
        }),
    )
    .await;
    assert_eq!(put_status, 204);

    // GET it back.
    let (status, body) = get_json(&router, "/api/reports/unresolved-links/config").await;
    assert_eq!(status, 200);
    assert_eq!(body["scope"], "collection:sample/REQ");
    assert_eq!(body["includeInactive"], true);
}

#[tokio::test]
async fn saved_config_get_without_prior_put_returns_empty_object() {
    let workspace = tempfile::tempdir().unwrap();
    let (router, _state, _temp) =
        build_app_with_workspace(fixture_sample_project, Some(workspace.path().to_path_buf()))
            .await;
    let (status, body) = get_json(&router, "/api/reports/cycles/config").await;
    assert_eq!(status, 200);
    assert_eq!(body, json!({}));
}

#[tokio::test]
async fn saved_config_put_without_workspace_returns_409() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, _) = put_json(
        &router,
        "/api/reports/unresolved-links/config",
        &json!({"x":1}),
    )
    .await;
    assert_eq!(status, 409);
}

#[tokio::test]
async fn cycles_report_surfaces_a_derives_from_loop() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        // REQ-a derives-from REQ-b, REQ-b derives-from REQ-a.
        write_content(
            root,
            "requirements",
            "REQ-a",
            UUID_REQ_A,
            "A",
            json!([
                {
                    "targetUuid": UUID_REQ_B,
                    "type": "derives-from",
                    "hint": hint("sample", "REQ", "REQ-b"),
                }
            ]),
            None,
        );
        write_content(
            root,
            "requirements",
            "REQ-b",
            UUID_REQ_B,
            "B",
            json!([
                {
                    "targetUuid": UUID_REQ_A,
                    "type": "derives-from",
                    "hint": hint("sample", "REQ", "REQ-a"),
                }
            ]),
            None,
        );
    })
    .await;

    let (status, body) = get_json(&router, "/api/reports/cycles").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "cycles");
    assert_eq!(body["totalCycles"], 1);
    assert_eq!(body["cycles"][0]["linkType"], "derives-from");
    assert_eq!(body["cycles"][0]["nodes"].as_array().unwrap().len(), 2);
    assert!(
        body["linkTypesChecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "derives-from")
    );
}

#[tokio::test]
async fn coverage_matrix_default_set_flags_uncovered_parents() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_collection(root, "designs", "DES");
        // REQ-a is covered by DES-a via satisfies; REQ-b has nothing.
        write_content(
            root,
            "requirements",
            "REQ-a",
            UUID_REQ_A,
            "A",
            json!([]),
            None,
        );
        write_content(
            root,
            "requirements",
            "REQ-b",
            UUID_REQ_B,
            "B",
            json!([]),
            None,
        );
        write_content(
            root,
            "designs",
            "DES-a",
            UUID_DES_A,
            "Design for A",
            json!([
                {
                    "targetUuid": UUID_REQ_A,
                    "type": "satisfies",
                    "hint": hint("sample", "REQ", "REQ-a"),
                }
            ]),
            None,
        );
    })
    .await;
    let (status, body) = get_json(
        &router,
        "/api/reports/coverage-matrix?scope=collection:sample/REQ",
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "coverage-matrix");
    assert_eq!(body["totalParents"], 2);
    assert_eq!(body["gapCount"], 1);
    let cov_types = body["coveringLinkTypes"].as_array().unwrap();
    assert!(cov_types.iter().any(|v| v == "satisfies"));
    assert!(cov_types.iter().any(|v| v == "verifies"));
}

#[tokio::test]
async fn coverage_matrix_custom_covering_types_are_honoured() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_content(
            root,
            "requirements",
            "REQ-a",
            UUID_REQ_A,
            "A",
            json!([]),
            None,
        );
    })
    .await;
    let (status, body) = get_json(
        &router,
        "/api/reports/coverage-matrix?coveringLinkTypes=derives-from,bogus",
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["coveringLinkTypes"], json!(["derives-from"]));
    assert_eq!(body["unknownRequestedTypes"], json!(["bogus"]));
}

#[tokio::test]
async fn impact_analysis_dependents_walks_incoming_edges() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        // REQ-b derives-from REQ-a so dependents of REQ-a = {REQ-b}.
        write_content(
            root,
            "requirements",
            "REQ-a",
            UUID_REQ_A,
            "A",
            json!([]),
            None,
        );
        write_content(
            root,
            "requirements",
            "REQ-b",
            UUID_REQ_B,
            "B",
            json!([
                {
                    "targetUuid": UUID_REQ_A,
                    "type": "derives-from",
                    "hint": hint("sample", "REQ", "REQ-a"),
                }
            ]),
            None,
        );
    })
    .await;

    let uri = format!("/api/reports/impact-analysis?seed={UUID_REQ_A}&direction=dependents");
    let (status, body) = get_json(&router, &uri).await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "impact-analysis");
    assert_eq!(body["direction"], "dependents");
    assert_eq!(body["totalImpacted"], 1);
    assert_eq!(body["impacted"][0]["node"]["artifactName"], "REQ-b");
    assert_eq!(body["impacted"][0]["depth"], 1);
}

#[tokio::test]
async fn impact_analysis_without_seed_returns_friendly_banner() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let (status, body) = get_json(&router, "/api/reports/impact-analysis").await;
    assert_eq!(status, 200);
    assert!(body["missingSeedReason"].as_str().is_some());
    assert_eq!(body["totalImpacted"], 0);
}

#[tokio::test]
async fn impact_analysis_bad_direction_returns_400() {
    let (router, _state, _temp) = build_app(fixture_sample_project).await;
    let uri = format!("/api/reports/impact-analysis?seed={UUID_REQ_A}&direction=sideways");
    let (status, _) = get_json(&router, &uri).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn conflicts_report_deduplicates_bidirectional_edges() {
    let (router, _state, _temp) = build_app(|root| {
        write_collection(root, "requirements", "REQ");
        write_content(
            root,
            "requirements",
            "REQ-a",
            UUID_REQ_A,
            "A",
            json!([
                {
                    "targetUuid": UUID_REQ_B,
                    "type": "conflicts-with",
                    "hint": hint("sample", "REQ", "REQ-b"),
                }
            ]),
            None,
        );
        write_content(
            root,
            "requirements",
            "REQ-b",
            UUID_REQ_B,
            "B",
            json!([
                {
                    "targetUuid": UUID_REQ_A,
                    "type": "conflicts-with",
                    "hint": hint("sample", "REQ", "REQ-a"),
                }
            ]),
            None,
        );
    })
    .await;

    let (status, body) = get_json(&router, "/api/reports/conflicts").await;
    assert_eq!(status, 200);
    assert_eq!(body["kind"], "conflicts");
    assert_eq!(body["totalPairs"], 1);
    assert_eq!(body["pairs"][0]["bidirectional"], true);
}

#[tokio::test]
async fn saved_config_delete_is_idempotent() {
    let workspace = tempfile::tempdir().unwrap();
    let (router, _state, _temp) =
        build_app_with_workspace(fixture_sample_project, Some(workspace.path().to_path_buf()))
            .await;

    // Delete with no prior state: still 204.
    let (r1, _) = delete_json(&router, "/api/reports/unresolved-links/config").await;
    assert_eq!(r1, 204);

    // Delete after a write: still 204, and subsequent GET is empty.
    let _ = put_json(
        &router,
        "/api/reports/unresolved-links/config",
        &json!({"x":1}),
    )
    .await;
    let (r2, _) = delete_json(&router, "/api/reports/unresolved-links/config").await;
    assert_eq!(r2, 204);
    let (_, body) = get_json(&router, "/api/reports/unresolved-links/config").await;
    assert_eq!(body, json!({}));
}
