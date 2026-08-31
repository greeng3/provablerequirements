//! Read-only HTTP handlers for Phase 1a.

// Several handlers return `Result<_, axum::response::Response>` — the deliberate reqforge pattern of
// carrying an early-return Response as the Err so a helper can short-circuit a request. clippy 1.98's
// `result_large_err` flags the axum Response as a large Err variant, but boxing an axum Response is
// unidiomatic and would ripple through every `?` site; the pattern is intentional, so allow it here.
#![allow(clippy::result_large_err)]

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_core::Stream;
use uuid::Uuid;

use crate::app::AppState;
use reqforge_model::load::{LoadedCollection, LoadedProject};
use reqforge_model::mount::MountState;
use reqforge_model::schema::{Artifact, ArtifactShape, CollectionConfig, ProjectConfig};
use reqforge_model::write::{atomic_write, reconcile_ownership, write_artifact_file};

use super::dto::{
    AdoptOrphanBlobRequest, ArtifactDetail, ArtifactDiffResponse, ArtifactHistoryResponse,
    ArtifactListing, ArtifactSearchResult, AtCommitQuery, BulkCheckUrlsRequest,
    BulkCheckUrlsResponse, BulkRenameSuggestionEntry, BulkRenameSuggestionsRequest,
    BulkRenameSuggestionsResponse, CheckUrlResponse, CollectionSummary, CreateArtifactRequest,
    CreateCollectionRequest, CreateReviewRequest, CreateUrlArtifactRequest, DiffQuery,
    ErrorResponse, HealthResponse, IncomingLinkEntry, InitProjectRequest,
    LastApprovalSnapshotResponse, LinkTypeDto, LlmAcknowledgePrivacyResponse, LlmPromptRequest,
    LlmPromptResponse, LlmProviderEntry, LlmProvidersResponse, LlmRetestResponse,
    MigrateSchemaRequest, MigrateSchemaResponse, MountEntry, ProjectDetail, ProjectSummary,
    ProviderCrudRequest, ProviderPatchRequest, ReadinessResponse, RenameArtifactRequest,
    RenameSuggestionsResponse, ReviewQueueEntry, ReviewQueueResponse, ReviewStateTag,
    ReviewerIdentityResponse, SampleContentCollectionSummary, SampleContentResponse,
    SystemStateResponse, UpdateArtifactRequest,
};

/// GET /healthz — always 200.
pub async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// GET /readyz — 200 once discovery has published; 503 otherwise.
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    if state.is_ready() {
        (StatusCode::OK, Json(ReadinessResponse { ready: true })).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadinessResponse { ready: false }),
        )
            .into_response()
    }
}

/// GET /api/projects — list every Project currently mounted and loaded.
pub async fn list_projects(State(state): State<Arc<AppState>>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let projects: Vec<ProjectSummary> = world
        .mounts
        .iter()
        .filter_map(|m| match &m.state {
            MountState::Project(p) => Some(ProjectSummary::from(p)),
            _ => None,
        })
        .collect();
    Json(projects).into_response()
}

/// GET /api/system — summary of the loaded `SystemConfig` + mount
/// project count, per `UX-systemConfigBanner`. The System Home
/// view uses this to decide whether to surface the
/// "create a System config" banner: when `loaded == false` and
/// `project_count >= 2`, the operator has multi-project mounts
/// but hasn't chosen to group them into a named System yet.
///
/// Never writes the System config — per INTENTIONS.md, ReqForge
/// only reads an operator-supplied `REQFORGE_SYSTEM_CONFIG`.
pub async fn get_system(State(state): State<Arc<AppState>>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let project_count = world
        .mounts
        .iter()
        .filter(|m| matches!(m.state, MountState::Project(_)))
        .count();
    let (loaded, name) = match world.system.config() {
        Some(cfg) => (true, Some(cfg.name.clone())),
        None => (false, None),
    };
    Json(SystemStateResponse {
        loaded,
        name,
        project_count,
    })
    .into_response()
}

/// GET /api/mounts — list every mount with its validity-state tag.
/// The System Home view uses this to render Project / Needs-init /
/// No-git / Load-failed badges per DEPLOY-mountValidityStates.
pub async fn list_mounts(State(state): State<Arc<AppState>>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let entries: Vec<MountEntry> = world.mounts.iter().map(MountEntry::from).collect();
    Json(entries).into_response()
}

/// GET /api/projects/:slug — one Project detail.
pub async fn get_project(State(state): State<Arc<AppState>>, Path(slug): Path<String>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    match find_project(&world, &slug) {
        Some(p) => Json(ProjectDetail::from(p)).into_response(),
        None => not_found(format!("project '{slug}' not found")),
    }
}

/// GET /api/projects/:slug/collections — all Collections in one Project.
pub async fn list_collections(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let collections: Vec<CollectionSummary> = project
        .collections
        .iter()
        .map(CollectionSummary::from)
        .collect();
    Json(collections).into_response()
}

/// GET /api/projects/:slug/collections/:prefix — one Collection.
pub async fn get_collection(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    match find_collection(project, &prefix) {
        Some(c) => Json(CollectionSummary::from(c)).into_response(),
        None => not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        )),
    }
}

/// GET /api/projects/:slug/collections/:prefix/artifacts — all artifacts
/// in one Collection.
pub async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    let listings: Vec<ArtifactListing> = collection
        .artifacts
        .iter()
        .map(ArtifactListing::from)
        .collect();
    Json(listings).into_response()
}

/// GET /api/artifacts/:uuid — one artifact resolved via the UUID index.
/// Phase 5d adds an optional `?at=<oid>` query param; when present
/// the handler composes the detail from the tree entry at that
/// commit so the UI can show the historical state alongside the
/// diff view. Links on the historical payload resolve against the
/// *current* world — stale `unresolved` flags are expected and
/// acceptable for the read-only history view.
pub async fn get_artifact(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Query(q): Query<AtCommitQuery>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };

    if let Some(oid) = q.at.as_deref() {
        return match historical_artifact_detail(&state, &world, project, collection, artifact, oid)
        {
            Ok(detail) => Json(detail).into_response(),
            Err(resp) => resp,
        };
    }

    Json(ArtifactDetail::from_loaded(
        artifact,
        &project.config.slug,
        &collection.config.prefix,
        &world,
    ))
    .into_response()
}

/// GET /api/link-types — effective catalog (built-ins + System
/// extras) for the link picker (per `TRACE-linkCatalog` and
/// `TRACE-linkExtensibility`).
pub async fn list_link_types(State(state): State<Arc<AppState>>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let out: Vec<LinkTypeDto> = world.link_catalog.iter().map(LinkTypeDto::from).collect();
    Json(out).into_response()
}

/// GET /api/artifacts/search — type-ahead picker backend.
///
/// Case-insensitive substring match over artifact name and title
/// across every loaded project. Exact-prefix matches on either
/// field sort ahead of plain substring hits. Linear scan is
/// adequate for Phase 3 workloads — Tantivy lands in Phase 7c
/// (per `UX-search`).
pub async fn search_artifacts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ArtifactSearchParams>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let query = params.q.trim().to_lowercase();
    if query.is_empty() {
        return Json(Vec::<ArtifactSearchResult>::new()).into_response();
    }
    let limit = params.limit.unwrap_or(25).clamp(1, 100);

    let mut scored: Vec<(u8, ArtifactSearchResult)> = Vec::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                if Some(artifact.metadata.uuid) == params.exclude {
                    continue;
                }
                let name_l = artifact.name.to_lowercase();
                let title_l = artifact.metadata.title.to_lowercase();
                let prefix_hit = name_l.starts_with(&query) || title_l.starts_with(&query);
                let substring_hit = name_l.contains(&query) || title_l.contains(&query);
                if !substring_hit {
                    continue;
                }
                // Lower rank number = higher priority. Prefix match
                // beats plain substring match.
                let rank: u8 = if prefix_hit { 0 } else { 1 };
                scored.push((
                    rank,
                    ArtifactSearchResult {
                        uuid: artifact.metadata.uuid,
                        project_slug: project.config.slug.clone(),
                        collection_prefix: collection.config.prefix.clone(),
                        artifact_name: artifact.name.clone(),
                        title: artifact.metadata.title.clone(),
                        active: artifact.metadata.is_active(),
                    },
                ));
            }
        }
    }
    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.project_slug.cmp(&b.1.project_slug))
            .then_with(|| a.1.artifact_name.cmp(&b.1.artifact_name))
    });
    let out: Vec<ArtifactSearchResult> = scored.into_iter().map(|(_, r)| r).take(limit).collect();
    Json(out).into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSearchParams {
    pub q: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub exclude: Option<Uuid>,
}

/// GET /api/reviewers — aggregate identity options for the review
/// dropdown (per `REVIEW-reviewerIdentity`).
///
/// `?projectSlug=<slug>` scopes the git default to that mount's
/// `.git/config` so per-project identity (the common case) wins
/// over the workspace-level config. Persisted identities come from
/// `<workspace>/reviewers.json`; session identities come from the
/// in-memory cache on `AppState` populated by every successful
/// review write.
pub async fn list_reviewers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReviewerIdentityParams>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };

    let git_default = params
        .project_slug
        .as_deref()
        .and_then(|slug| find_project(&world, slug))
        .and_then(|project| project.git_user_name());

    let persisted = match state.config().workspace_dir.as_ref() {
        Some(dir) => {
            match reqforge_model::reviews::load_reviewers_json(&dir.join("reviewers.json")) {
                Ok(file) => file.reviewers,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "reviewers.json load failed; returning empty list",
                    );
                    Vec::new()
                }
            }
        }
        None => Vec::new(),
    };

    let session = state.session_identities_snapshot().await;

    Json(ReviewerIdentityResponse {
        git_default,
        persisted,
        session,
    })
    .into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerIdentityParams {
    #[serde(default)]
    pub project_slug: Option<String>,
}

/// GET /api/reviews/queue — system-wide review queue (per
/// `UX-reviewQueue`).
///
/// Walks every mounted project once and partitions each artifact
/// into the "awaiting review" section (no approval covering the
/// current body) and the "blocking TODOs" section (one or more open
/// TODOs from a prior rejection). Applies optional server-side
/// filters (`?projectSlug=`, `?collectionPrefix=`, `?shape=`,
/// `?tag=`, `?reviewer=`) and an ordering toggle
/// (`?order=oldest-first|newest-first`, default oldest-first for
/// awaiting-review, newest-first for blocking-TODOs).
pub async fn review_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ReviewQueueParams>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };

    let mut awaiting: Vec<ReviewQueueEntry> = Vec::new();
    let mut blocking: Vec<ReviewQueueEntry> = Vec::new();

    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        if let Some(slug) = &params.project_slug
            && &project.config.slug != slug
        {
            continue;
        }
        for collection in &project.collections {
            if let Some(prefix) = &params.collection_prefix
                && &collection.config.prefix != prefix
            {
                continue;
            }
            for artifact in &collection.artifacts {
                if let Some(shape_filter) = params.shape
                    && artifact.metadata.shape != shape_filter
                {
                    continue;
                }
                if let Some(tag_filter) = &params.tag {
                    let tags = artifact.metadata.tags.as_deref().unwrap_or(&[]);
                    if !tags.iter().any(|t| t == tag_filter) {
                        continue;
                    }
                }

                let derived =
                    reqforge_model::reviews::derive_review_state(&artifact.metadata.review_log);

                if let Some(reviewer_filter) = &params.reviewer {
                    let matches = derived.last_reviewer.as_deref() == Some(reviewer_filter)
                        || derived
                            .blocking_todos
                            .iter()
                            .any(|t| t.added_by == *reviewer_filter);
                    if !matches {
                        continue;
                    }
                }

                let entry = ReviewQueueEntry {
                    uuid: artifact.metadata.uuid,
                    project_slug: project.config.slug.clone(),
                    collection_prefix: collection.config.prefix.clone(),
                    artifact_name: artifact.name.clone(),
                    title: artifact.metadata.title.clone(),
                    shape: artifact.metadata.shape,
                    state: ReviewStateTag::from(derived.state),
                    last_event_at: derived.last_event_at,
                    modified_at: artifact.metadata.modified_at,
                    blocking_todo_count: derived.blocking_todos.len(),
                    tags: artifact.metadata.tags.clone().unwrap_or_default(),
                    last_reviewer: derived.last_reviewer.clone(),
                };

                match derived.state {
                    reqforge_model::reviews::ReviewState::NeverReviewed
                    | reqforge_model::reviews::ReviewState::Rejected
                    | reqforge_model::reviews::ReviewState::ReRequested => {
                        if entry.blocking_todo_count > 0 {
                            blocking.push(entry);
                        } else {
                            awaiting.push(entry);
                        }
                    }
                    reqforge_model::reviews::ReviewState::Approved => {
                        // Approved artifacts only land in the queue
                        // when they have open TODOs *post-approval*
                        // — in practice that's never, because
                        // approval wipes them — so approved
                        // artifacts drop out of both sections.
                    }
                }
            }
        }
    }

    let order = params.order.unwrap_or(ReviewQueueOrder::OldestFirst);
    sort_queue_section(&mut awaiting, order, QueueSortKey::Awaiting);
    sort_queue_section(&mut blocking, order, QueueSortKey::Blocking);

    Json(ReviewQueueResponse {
        awaiting_review: awaiting,
        blocking_todos: blocking,
    })
    .into_response()
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueParams {
    #[serde(default)]
    pub project_slug: Option<String>,
    #[serde(default)]
    pub collection_prefix: Option<String>,
    #[serde(default)]
    pub shape: Option<reqforge_model::schema::ArtifactShape>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub reviewer: Option<String>,
    #[serde(default)]
    pub order: Option<ReviewQueueOrder>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewQueueOrder {
    OldestFirst,
    NewestFirst,
}

#[derive(Clone, Copy)]
enum QueueSortKey {
    Awaiting,
    Blocking,
}

fn sort_queue_section(
    entries: &mut [ReviewQueueEntry],
    order: ReviewQueueOrder,
    key: QueueSortKey,
) {
    // Awaiting-review defaults to oldest-first by modifiedAt per
    // UX-reviewQueue; blocking-TODOs defaults to
    // newest-rejection-first (by lastEventAt) so the most recent
    // rejections sit at the top.
    entries.sort_by(|a, b| {
        let primary = match key {
            QueueSortKey::Awaiting => a.modified_at.cmp(&b.modified_at),
            QueueSortKey::Blocking => b.last_event_at.cmp(&a.last_event_at),
        };
        let primary = match order {
            ReviewQueueOrder::OldestFirst => primary,
            ReviewQueueOrder::NewestFirst => primary.reverse(),
        };
        primary
            .then_with(|| a.project_slug.cmp(&b.project_slug))
            .then_with(|| a.collection_prefix.cmp(&b.collection_prefix))
            .then_with(|| a.artifact_name.cmp(&b.artifact_name))
    });
}

/// GET /api/events — SSE stream of world-change notifications.
///
/// Each subscriber gets a dedicated broadcast receiver; when
/// ReqForge's world is replaced (CRUD write, polling-watcher
/// refresh) a `change` event with a JSON payload fires. If the
/// subscriber lags too far behind the broadcast buffer, the
/// channel returns `Lagged` — we rebroadcast it as a single
/// `WorldChanged` event so the client refetches from scratch.
pub async fn events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".into());
                    yield Ok::<Event, Infallible>(Event::default().event("change").data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let data = serde_json::to_string(&crate::app::ChangeEvent::WorldChanged)
                        .unwrap_or_else(|_| "{}".into());
                    yield Ok(Event::default().event("change").data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

// ---- helpers ----

fn find_project<'a>(world: &'a crate::app::World, slug: &str) -> Option<&'a LoadedProject> {
    world.mounts.iter().find_map(|m| match &m.state {
        MountState::Project(p) if p.config.slug == slug => Some(p),
        _ => None,
    })
}

fn find_collection<'a>(project: &'a LoadedProject, prefix: &str) -> Option<&'a LoadedCollection> {
    project
        .collections
        .iter()
        .find(|c| c.config.prefix == prefix)
}

fn service_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "discovery in progress".to_owned(),
        }),
    )
        .into_response()
}

fn not_found(msg: impl Into<String>) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse { error: msg.into() }),
    )
        .into_response()
}

fn internal_error(msg: impl Into<String>) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: msg.into() }),
    )
        .into_response()
}

/// PUT /api/artifacts/:uuid — update an artifact's mutable fields
/// and (optionally) replace its body. Immutable fields — uuid,
/// shape, createdAt, reviewLog — are left alone; modifiedAt gets
/// bumped to Utc::now().
pub async fn update_artifact(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Json(req): Json<UpdateArtifactRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };

    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(current) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };

    let mut metadata = current.metadata.clone();
    if let Some(title) = req.title {
        metadata.title = title;
    }
    if let Some(description) = req.description {
        metadata.description = description;
    }
    if let Some(active) = req.active {
        metadata.active = active;
    }
    if let Some(derived) = req.derived {
        metadata.derived = derived;
    }
    if let Some(tags) = req.tags {
        metadata.tags = tags;
    }
    if let Some(outline_level) = req.outline_level {
        metadata.outline_level = outline_level;
    }
    if let Some(link_reqs) = req.links {
        let inputs: Vec<reqforge_model::links::LinkWriteInput> =
            link_reqs.into_iter().map(Into::into).collect();
        match reqforge_model::links::validate_links(
            uuid,
            &inputs,
            &world.link_catalog,
            &world.index,
        ) {
            Ok(validated) => {
                metadata.links = validated.0;
            }
            Err(err) => {
                return bad_request(format!("{err}"));
            }
        }
    }
    if let Some(new_url) = req.url {
        if metadata.shape != ArtifactShape::Url {
            return bad_request(
                "`url` is only editable on URL-shape artifacts; \
                 use PUT /api/artifacts/:uuid/blob to replace a blob",
            );
        }
        if let Err(msg) = validate_http_url(&new_url) {
            return bad_request(msg);
        }
        metadata.url = Some(new_url);
        // Editing the URL invalidates the last check; clear the
        // stale check-status so the UI doesn't leave a misleading
        // green pill on a URL that's never been verified.
        metadata.checked_at = None;
        metadata.check_status = None;
    }
    metadata.modified_at = chrono::Utc::now();

    let body = req
        .body
        .unwrap_or_else(|| current.body.clone().unwrap_or_default());
    let source_path = current.source_path.clone();
    let project_root = project.root.clone();
    let project_slug = project.config.slug.clone();
    let collection_prefix = collection.config.prefix.clone();
    let name = current.name.clone();
    let overrides = state.overrides();

    // Drop the world snapshot before running the blocking write so
    // we don't hold the Arc across the spawn_blocking.
    drop(world);

    let metadata_for_write = metadata.clone();
    let body_for_write = body.clone();
    let shape = metadata.shape;
    let write_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        match shape {
            ArtifactShape::Content => write_artifact_file(
                &source_path,
                &project_root,
                &metadata_for_write,
                &body_for_write,
                overrides,
            )
            .map_err(|err| format!("{err}")),
            ArtifactShape::Url => reqforge_model::write::write_sidecar_only(
                &source_path,
                &project_root,
                &metadata_for_write,
                overrides,
            )
            .map_err(|err| format!("{err}")),
            ArtifactShape::Blob => {
                // Blob metadata updates share the sidecar path —
                // the binary is unaffected, only the sidecar JSON
                // is rewritten.
                reqforge_model::write::write_sidecar_only(
                    &source_path,
                    &project_root,
                    &metadata_for_write,
                    overrides,
                )
                .map_err(|err| format!("{err}"))
            }
        }
    })
    .await;

    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write failed: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after write failed: {err}"));
    }

    // Re-fetch the artifact from the new world to return an
    // authoritative ArtifactDetail.
    let Some(fresh) = state.snapshot().await else {
        return internal_error("world missing after refresh");
    };
    let Some(location) = fresh.index.get(&uuid) else {
        return internal_error("artifact vanished after refresh");
    };
    let project = find_project(&fresh, &location.project_slug).unwrap();
    let collection = find_collection(project, &location.collection_prefix).unwrap();
    let artifact = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
        .unwrap();

    let _ = (project_slug, collection_prefix, name); // borrowed for write path

    (
        StatusCode::OK,
        Json(ArtifactDetail::from_loaded(
            artifact,
            &project.config.slug,
            &collection.config.prefix,
            &fresh,
        )),
    )
        .into_response()
}

/// POST /api/artifacts/:uuid/reviews — append one review-log
/// entry for the artifact.
///
/// One HTTP call = one log entry (per the Phase 4 action/entry 1:1
/// contract). The handler runs [`validate_and_build_entry`] against
/// the artifact's current `DerivedReviewState`, appends the entry
/// via the existing atomic artifact-file write path, records the
/// session identity on `AppState`, persists the reviewer into
/// `<workspace>/reviewers.json` on first sighting, and — when the
/// action is `approve` — writes a snapshot under
/// `<workspace>/review-snapshots/<uuid>/<ts>/artifact.md` so
/// Phase 4c's "since last approval" diff has a before-image.
pub async fn create_review(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Json(req): Json<CreateReviewRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(current) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };

    let derived = reqforge_model::reviews::derive_review_state(&current.metadata.review_log);
    let action_input = reqforge_model::reviews::ReviewActionInput {
        reviewer: req.reviewer.clone(),
        action: req.action.into_action_input(),
        explanation: req.explanation,
    };
    let now = chrono::Utc::now();
    let validated =
        match reqforge_model::reviews::validate_and_build_entry(&derived, action_input, now) {
            Ok(v) => v,
            Err(err) => {
                return review_validation_response(err);
            }
        };

    let mut metadata = current.metadata.clone();
    metadata.review_log.push(validated.entry.clone());
    metadata.modified_at = now;

    let body = current.body.clone().unwrap_or_default();
    let source_path = current.source_path.clone();
    let project_root = project.root.clone();
    let overrides = state.overrides();
    let workspace_dir = state.config().workspace_dir.clone();
    let reviewer_for_session = validated.entry.reviewer.clone();
    let is_approval = validated.is_approval;

    drop(world);

    let metadata_for_write = metadata.clone();
    let body_for_write = body.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        write_artifact_file(
            &source_path,
            &project_root,
            &metadata_for_write,
            &body_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("review write failed: {err}")),
        Err(join_err) => {
            return internal_error(format!("review write task panicked: {join_err}"));
        }
    }

    // Best-effort side effects: warn on failure but don't roll the
    // review back; the event has already landed on disk.
    if let Some(workspace) = &workspace_dir {
        let workspace_for_persist = workspace.clone();
        let reviewer_for_persist = reviewer_for_session.clone();
        let persist = tokio::task::spawn_blocking(move || {
            reqforge_model::reviews::append_reviewer_if_missing(
                &workspace_for_persist,
                &reviewer_for_persist,
            )
        })
        .await;
        match persist {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                tracing::warn!(error = %err, "reviewers.json append failed");
            }
            Err(join_err) => {
                tracing::warn!(error = %join_err, "reviewers.json append task panicked");
            }
        }

        if is_approval {
            let workspace_for_snapshot = workspace.clone();
            let frontmatter_json = match serde_json::to_string_pretty(&metadata) {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!(error = %err, "approval snapshot serialise failed");
                    String::new()
                }
            };
            let body_for_snapshot = body.clone();
            if !frontmatter_json.is_empty() {
                let snapshot = tokio::task::spawn_blocking(move || {
                    reqforge_model::reviews::write_approval_snapshot(
                        &workspace_for_snapshot,
                        uuid,
                        now,
                        &frontmatter_json,
                        &body_for_snapshot,
                    )
                })
                .await;
                match snapshot {
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        tracing::warn!(error = %err, "approval snapshot write failed");
                    }
                    Err(join_err) => {
                        tracing::warn!(error = %join_err, "approval snapshot task panicked");
                    }
                }
            }
        }
    }

    state.record_session_identity(&reviewer_for_session).await;

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after review failed: {err}"));
    }

    let Some(fresh) = state.snapshot().await else {
        return internal_error("world missing after refresh");
    };
    let Some(location) = fresh.index.get(&uuid) else {
        return internal_error("artifact vanished after refresh");
    };
    let Some(project) = find_project(&fresh, &location.project_slug) else {
        return internal_error("project missing after refresh");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("collection missing after refresh");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("artifact missing after refresh");
    };
    (
        StatusCode::CREATED,
        Json(ArtifactDetail::from_loaded(
            artifact,
            &project.config.slug,
            &collection.config.prefix,
            &fresh,
        )),
    )
        .into_response()
}

/// GET /api/artifacts/:uuid/reviews/last-approval-snapshot —
/// return the most recent approval snapshot for an artifact.
///
/// 404s when no approval snapshot has been written yet (the
/// artifact has never been approved through ReqForge), so the UI
/// can suppress the diff panel per `UX-reviewPane`'s
/// "No prior approval" rule.
pub async fn last_approval_snapshot(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    if world.index.get(&uuid).is_none() {
        return not_found(format!("artifact {uuid} not found"));
    }
    let Some(workspace) = state.config().workspace_dir.clone() else {
        return not_found("no workspace configured; approval snapshots unavailable");
    };
    drop(world);

    let lookup = tokio::task::spawn_blocking(move || {
        reqforge_model::reviews::load_latest_approval_snapshot(&workspace, uuid)
    })
    .await;
    let snapshot = match lookup {
        Ok(Ok(Some(snapshot))) => snapshot,
        Ok(Ok(None)) => return not_found(format!("no approval snapshot for {uuid}")),
        Ok(Err(err)) => return internal_error(format!("snapshot load failed: {err}")),
        Err(join_err) => {
            return internal_error(format!("snapshot load task panicked: {join_err}"));
        }
    };
    let metadata = serde_json::from_str::<serde_json::Value>(&snapshot.frontmatter_json)
        .unwrap_or_else(|_| serde_json::Value::Null);
    Json(LastApprovalSnapshotResponse {
        approved_at: snapshot.approved_at,
        body: snapshot.body,
        metadata,
    })
    .into_response()
}

fn review_validation_response(err: reqforge_model::reviews::ReviewValidationError) -> Response {
    use reqforge_model::reviews::ReviewValidationError;
    let msg = err.to_string();
    let status = match err {
        ReviewValidationError::ApproveWithOpenTodos { .. } => StatusCode::CONFLICT,
        ReviewValidationError::ResolveTodoUnknown(_) => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(ErrorResponse { error: msg })).into_response()
}

/// POST /api/projects/:slug/collections/:prefix/artifacts —
/// create a new content-hosted artifact inside the named
/// collection.
pub async fn create_artifact(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
    Json(req): Json<CreateArtifactRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.name) {
        return bad_request(format!(
            "invalid artifact name '{}': must match [A-Za-z0-9._-]+ and not be empty",
            req.name
        ));
    }

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };

    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    if collection.artifacts.iter().any(|a| a.name == req.name) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "artifact '{}' already exists in collection '{}'",
                    req.name, prefix
                ),
            }),
        )
            .into_response();
    }

    let target = collection.dir_path.join(format!("{}.md", req.name));
    let project_root = project.root.clone();
    let overrides = state.overrides();
    let now = chrono::Utc::now();

    let metadata = Artifact {
        schema_version: 1,
        uuid: uuid::Uuid::now_v7(),
        title: req.title,
        shape: ArtifactShape::Content,
        created_at: now,
        modified_at: now,
        links: Vec::new(),
        review_log: Vec::new(),
        description: req.description,
        expects_code_trace: None,
        active: req.active,
        derived: req.derived,
        tags: req.tags,
        outline_level: req.outline_level,
        legacy: None,
        blob_path: None,
        url: None,
        checked_at: None,
        check_status: None,
        overflow: Default::default(),
    };
    let body = req.body.unwrap_or_default();
    let new_uuid = metadata.uuid;

    drop(world);

    let metadata_for_write = metadata.clone();
    let target_for_write = target.clone();
    let body_for_write = body.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        write_artifact_file(
            &target_for_write,
            &project_root,
            &metadata_for_write,
            &body_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write failed: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after create failed: {err}"));
    }

    // Return the freshly-loaded artifact.
    let fresh = state.snapshot().await.expect("world present after refresh");
    let location = fresh
        .index
        .get(&new_uuid)
        .expect("new artifact missing from index after refresh");
    let project = find_project(&fresh, &location.project_slug).unwrap();
    let collection = find_collection(project, &location.collection_prefix).unwrap();
    let artifact = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == new_uuid)
        .unwrap();

    (
        StatusCode::CREATED,
        Json(ArtifactDetail::from_loaded(
            artifact,
            &project.config.slug,
            &collection.config.prefix,
            &fresh,
        )),
    )
        .into_response()
}

/// GET /api/artifacts/:uuid/incoming-links — list every artifact
/// that links *to* the target. Used by the UI's delete-confirm
/// dialog so the user sees what will be left as unresolved links
/// if they proceed.
pub async fn list_incoming_links(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    if world.index.get(&uuid).is_none() {
        return not_found(format!("artifact {uuid} not found"));
    }
    let entries = collect_incoming_links(&world, uuid);
    Json(entries).into_response()
}

/// DELETE /api/artifacts/:uuid — remove an artifact's file from
/// disk and refresh state. Incoming links on other artifacts
/// survive as unresolved (per ART-deletionSemantics +
/// TRACE-unresolvedLinks) — ReqForge does not rewrite source-side
/// artifacts to scrub them.
pub async fn delete_artifact(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let source_path = location.source_path.clone();
    drop(world);

    let remove = tokio::task::spawn_blocking(move || std::fs::remove_file(&source_path)).await;
    match remove {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("remove failed: {err}")),
        Err(join_err) => return internal_error(format!("remove task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after delete failed: {err}"));
    }

    StatusCode::NO_CONTENT.into_response()
}

fn is_safe_filename_stem(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn bad_request(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: msg.into() }),
    )
        .into_response()
}

/// Walk every loaded project and collect outgoing links that point
/// at `target`. Cheap — Phase 1 workloads have at most hundreds of
/// artifacts.
fn collect_incoming_links(world: &crate::app::World, target: Uuid) -> Vec<IncomingLinkEntry> {
    let mut out = Vec::new();
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        for collection in &project.collections {
            for artifact in &collection.artifacts {
                for link in &artifact.metadata.links {
                    if link.target_uuid == target {
                        out.push(IncomingLinkEntry {
                            project_slug: project.config.slug.clone(),
                            collection_prefix: collection.config.prefix.clone(),
                            artifact_name: artifact.name.clone(),
                            source_uuid: artifact.metadata.uuid,
                            link_type: link.type_name.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// PATCH /api/artifacts/:uuid — rename an artifact within its
/// current collection. UUID is the authoritative identity, so
/// every incoming link keeps resolving after the rename; hints
/// update lazily on next read per ART-moveRename.
///
/// Cross-collection moves are a separate endpoint not yet wired
/// in Phase 2.
pub async fn rename_artifact(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Json(req): Json<RenameArtifactRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.name) {
        return bad_request(format!(
            "invalid artifact name '{}': must match [A-Za-z0-9._-]+ and not be empty",
            req.name
        ));
    }

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    if location.artifact_name == req.name {
        // No-op rename: return the current detail.
        let project = find_project(&world, &location.project_slug).unwrap();
        let collection = find_collection(project, &location.collection_prefix).unwrap();
        let artifact = collection
            .artifacts
            .iter()
            .find(|a| a.metadata.uuid == uuid)
            .unwrap();
        return Json(ArtifactDetail::from_loaded(
            artifact,
            &project.config.slug,
            &collection.config.prefix,
            &world,
        ))
        .into_response();
    }
    let project = find_project(&world, &location.project_slug).unwrap();
    let collection = find_collection(project, &location.collection_prefix).unwrap();
    if collection.artifacts.iter().any(|a| a.name == req.name) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "artifact '{}' already exists in collection '{}'",
                    req.name, collection.config.prefix
                ),
            }),
        )
            .into_response();
    }

    let old_path = location.source_path.clone();
    let new_path = collection.dir_path.join(format!("{}.md", req.name));
    drop(world);

    let rename_result =
        tokio::task::spawn_blocking(move || std::fs::rename(&old_path, &new_path)).await;
    match rename_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("rename failed: {err}")),
        Err(join_err) => return internal_error(format!("rename task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after rename failed: {err}"));
    }

    let fresh = state.snapshot().await.expect("world present after refresh");
    let location = fresh
        .index
        .get(&uuid)
        .expect("artifact missing after rename refresh");
    let project = find_project(&fresh, &location.project_slug).unwrap();
    let collection = find_collection(project, &location.collection_prefix).unwrap();
    let artifact = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
        .unwrap();
    Json(ArtifactDetail::from_loaded(
        artifact,
        &project.config.slug,
        &collection.config.prefix,
        &fresh,
    ))
    .into_response()
}

/// POST /api/projects/:slug/collections — create a new Collection.
pub async fn create_collection(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<CreateCollectionRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.dir_name) {
        return bad_request(format!(
            "invalid collection dirName '{}': must match [A-Za-z0-9._-]+ and not be empty",
            req.dir_name
        ));
    }
    if !is_safe_prefix(&req.prefix) {
        return bad_request(format!(
            "invalid prefix '{}': must be alphanumeric and not empty",
            req.prefix
        ));
    }

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    if project
        .collections
        .iter()
        .any(|c| c.config.prefix == req.prefix)
    {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "prefix '{}' is already in use by another collection",
                    req.prefix
                ),
            }),
        )
            .into_response();
    }
    if project
        .collections
        .iter()
        .any(|c| c.dir_name == req.dir_name)
    {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("directory '{}' already exists", req.dir_name),
            }),
        )
            .into_response();
    }

    let project_root = project.root.clone();
    let artifacts_root = project_root.join(project.config.effective_artifacts_path());
    let collection_dir = artifacts_root.join(&req.dir_name);
    let config_path = collection_dir.join(".collection.json");
    let overrides = state.overrides();

    let config = CollectionConfig {
        schema_version: 1,
        prefix: req.prefix.clone(),
        name: req.name,
        description: req.description,
        expects_code_trace: req.expects_code_trace,
        import_notes: None,
        overflow: Default::default(),
    };
    let bytes = match serde_json::to_vec_pretty(&config) {
        Ok(mut b) => {
            b.push(b'\n');
            b
        }
        Err(err) => return internal_error(format!("serialize collection config: {err}")),
    };

    drop(world);

    let write_result = tokio::task::spawn_blocking(move || {
        atomic_write(&config_path, &bytes)?;
        reconcile_ownership(&config_path, &project_root, overrides)?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write collection config: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after create failed: {err}"));
    }

    let fresh = state.snapshot().await.expect("world present after refresh");
    let project = find_project(&fresh, &slug).expect("project missing after refresh");
    let collection =
        find_collection(project, &req.prefix).expect("new collection missing after refresh");
    (
        StatusCode::CREATED,
        Json(CollectionSummary::from(collection)),
    )
        .into_response()
}

/// DELETE /api/projects/:slug/collections/:prefix — remove a
/// Collection's directory. Refuses if any artifacts remain.
pub async fn delete_collection(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    if !collection.artifacts.is_empty() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "collection '{}' still has {} artifact(s); move or delete them first",
                    prefix,
                    collection.artifacts.len()
                ),
            }),
        )
            .into_response();
    }
    let dir_path = collection.dir_path.clone();
    drop(world);

    let remove = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&dir_path)).await;
    match remove {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("remove collection dir: {err}")),
        Err(join_err) => return internal_error(format!("remove task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after delete failed: {err}"));
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Query parameters for the project wipe endpoint.
#[derive(Debug, Default, serde::Deserialize)]
pub struct WipeProjectQuery {
    /// When true, also remove `reqforge.json` and the `artifacts/`
    /// directory itself, reverting the mount to a NeedsInit state.
    /// When false (default), only the contents of `artifacts/` are
    /// removed and the project continues to load.
    #[serde(default)]
    pub deinit: bool,
}

/// DELETE /api/projects/:slug/artifacts — scorched-earth wipe.
///
/// Default: removes every immediate subdirectory of the project's
/// artifacts root, leaving reqforge.json and the artifacts/ root
/// itself in place so the project continues to load. Walks the
/// filesystem rather than the in-memory snapshot so a stray
/// collection directory the watcher hasn't caught up to gets
/// nuked too — re-imports must start from a clean slate.
///
/// With `?deinit=true`: additionally removes the artifacts/
/// directory itself and reqforge.json so the mount reverts to a
/// NeedsInit state, as if ReqForge had never touched the repo.
pub async fn wipe_project_artifacts(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Query(q): Query<WipeProjectQuery>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    let artifacts_root = project.root.join(project.config.effective_artifacts_path());
    drop(world);

    let deinit = q.deinit;
    let remove = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let entries = match std::fs::read_dir(&artifacts_root) {
            Ok(it) => Some(it),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err),
        };
        if let Some(entries) = entries {
            for entry in entries {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                }
            }
        }
        if deinit {
            match std::fs::remove_dir_all(&artifacts_root) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
            match std::fs::remove_file(project_root.join("reqforge.json")) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    })
    .await;
    match remove {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("wipe collection dirs: {err}")),
        Err(join_err) => return internal_error(format!("wipe task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after wipe failed: {err}"));
    }
    StatusCode::NO_CONTENT.into_response()
}

/// POST /api/mounts/:dirName/init — promote a NeedsInit mount to
/// a fully-loaded Project by writing a reqforge.json at its root.
pub async fn init_project(
    State(state): State<Arc<AppState>>,
    Path(dir_name): Path<String>,
    Json(req): Json<InitProjectRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.slug) {
        return bad_request(format!(
            "invalid slug '{}': must match [A-Za-z0-9._-]+ and not be empty",
            req.slug
        ));
    }
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };

    let mount = world
        .mounts
        .iter()
        .find(|m| m.path.file_name().and_then(|s| s.to_str()) == Some(dir_name.as_str()));
    let Some(mount) = mount else {
        return not_found(format!("mount '{dir_name}' not found"));
    };
    let repo_root = match &mount.state {
        MountState::NeedsInit => mount.path.clone(),
        MountState::Project(_) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!("mount '{dir_name}' is already a Project"),
                }),
            )
                .into_response();
        }
        MountState::NoGit => {
            return bad_request(format!(
                "mount '{dir_name}' has no .git — initialise the repository first"
            ));
        }
        MountState::LoadFailed(err) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!(
                        "mount '{dir_name}' has a reqforge.json but failed to load: {err}"
                    ),
                }),
            )
                .into_response();
        }
    };

    if world
        .mounts
        .iter()
        .any(|m| matches!(&m.state, MountState::Project(p) if p.config.slug == req.slug))
    {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("slug '{}' is already in use by another Project", req.slug),
            }),
        )
            .into_response();
    }

    let config = ProjectConfig {
        schema_version: 1,
        slug: req.slug.clone(),
        name: req.name,
        description: req.description,
        artifacts_path: req.artifacts_path,
        scan_paths: None,
        overflow: Default::default(),
    };
    let bytes = match serde_json::to_vec_pretty(&config) {
        Ok(mut b) => {
            b.push(b'\n');
            b
        }
        Err(err) => return internal_error(format!("serialize reqforge.json: {err}")),
    };
    let target = repo_root.join("reqforge.json");
    let overrides = state.overrides();

    drop(world);

    let repo_root_for_write = repo_root.clone();
    let target_for_write = target.clone();
    let config_for_write = config.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        atomic_write(&target_for_write, &bytes)?;
        reconcile_ownership(&target_for_write, &repo_root_for_write, overrides)?;
        let artifacts_path = config_for_write
            .artifacts_path
            .as_deref()
            .unwrap_or("artifacts");
        let artifacts_root = repo_root_for_write.join(artifacts_path);
        std::fs::create_dir_all(&artifacts_root)?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write reqforge.json: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after init failed: {err}"));
    }

    let fresh = state.snapshot().await.expect("world present after refresh");
    let project = find_project(&fresh, &req.slug).expect("project missing after init");
    (StatusCode::CREATED, Json(ProjectDetail::from(project))).into_response()
}

fn is_safe_prefix(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric())
}

// =====================================================================
// Phase 5b — blob and URL artifact handlers.
// =====================================================================

/// Multipart-friendly extraction of the upload fields from an
/// `axum::Multipart` stream. Pulls a single file plus the common
/// metadata fields — `name`, `title`, plus the optional fields the
/// content-hosted create path already understands.
struct BlobUploadParts {
    filename: String,
    bytes: Vec<u8>,
    name: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    active: Option<bool>,
    derived: Option<bool>,
    outline_level: Option<String>,
}

async fn parse_blob_multipart(
    mut multipart: Multipart,
    max_bytes: u64,
) -> Result<BlobUploadParts, Response> {
    let mut filename: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;
    let mut name: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut tags: Option<Vec<String>> = None;
    let mut active: Option<bool> = None;
    let mut derived: Option<bool> = None;
    let mut outline_level: Option<String> = None;

    while let Some(field_res) = multipart
        .next_field()
        .await
        .map_err(|e| bad_request(format!("multipart error: {e}")))
        .transpose()
    {
        let field = field_res?;
        let field_name = field.name().unwrap_or("").to_owned();
        match field_name.as_str() {
            "file" => {
                filename = field
                    .file_name()
                    .map(|s| s.to_owned())
                    .or_else(|| Some("blob".to_owned()));
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| bad_request(format!("reading upload: {e}")))?;
                if data.len() as u64 > max_bytes {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(ErrorResponse {
                            error: format!(
                                "upload is {} bytes; cap is {} bytes (REQFORGE_MAX_BLOB_BYTES). \
                                 Consider a URL-reference artifact instead.",
                                data.len(),
                                max_bytes,
                            ),
                        }),
                    )
                        .into_response());
                }
                bytes = Some(data.to_vec());
            }
            "name" => {
                name = Some(field_text(field).await?);
            }
            "title" => {
                title = Some(field_text(field).await?);
            }
            "description" => {
                description = Some(field_text(field).await?);
            }
            "tags" => {
                // Accept a comma-separated string for easy curl use;
                // the frontend sends a JSON array encoded as text.
                let raw = field_text(field).await?;
                tags = Some(parse_tags_field(&raw));
            }
            "active" => {
                active = Some(parse_bool(&field_text(field).await?));
            }
            "derived" => {
                derived = Some(parse_bool(&field_text(field).await?));
            }
            "outlineLevel" => {
                outline_level = Some(field_text(field).await?);
            }
            _ => {
                // Ignore unknown fields rather than erroring so a
                // future field addition on the client side stays
                // forward-compatible.
                let _ = field.bytes().await;
            }
        }
    }

    let filename = filename.ok_or_else(|| bad_request("missing `file` field"))?;
    let bytes = bytes.ok_or_else(|| bad_request("missing `file` bytes"))?;

    Ok(BlobUploadParts {
        filename,
        bytes,
        name,
        title,
        description,
        tags,
        active,
        derived,
        outline_level,
    })
}

async fn field_text(field: axum::extract::multipart::Field<'_>) -> Result<String, Response> {
    field
        .text()
        .await
        .map_err(|e| bad_request(format!("multipart text field: {e}")))
}

fn parse_tags_field(raw: &str) -> Vec<String> {
    // Accept either a JSON array or a comma-separated plain string.
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(raw) {
        return parsed
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .collect();
    }
    raw.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

/// POST /api/projects/:slug/collections/:prefix/artifacts/blob —
/// create a new blob artifact from a multipart upload.
pub async fn create_blob_artifact(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
    multipart: Multipart,
) -> Response {
    let max = state.config().max_blob_bytes;
    let parts = match parse_blob_multipart(multipart, max).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(name) = parts.name.clone() else {
        return bad_request("missing `name` field");
    };
    let Some(title) = parts.title.clone() else {
        return bad_request("missing `title` field");
    };
    if !is_safe_filename_stem(&name) {
        return bad_request(format!(
            "invalid artifact name '{name}': must match [A-Za-z0-9._-]+",
        ));
    }
    // Derive and validate the extension from the uploaded filename.
    let extension = std::path::Path::new(&parts.filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !reqforge_model::load::is_allowed_blob_extension(&extension) {
        return bad_request(format!(
            "extension '{extension}' is not in the blob allowlist",
        ));
    }
    // Magic-byte sniff per the locked Phase 5 decision.
    if let Err(msg) = infer_matches_extension(&parts.bytes, &extension) {
        return bad_request(msg);
    }

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    let binary_relative = relative_blob_path(project, collection, &name, &extension);
    if artifact_name_collides(collection, &name) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("artifact '{name}' already exists in collection '{prefix}'",),
            }),
        )
            .into_response();
    }
    let now = chrono::Utc::now();
    let new_uuid = Uuid::now_v7();
    let metadata = Artifact {
        schema_version: 1,
        uuid: new_uuid,
        title,
        shape: ArtifactShape::Blob,
        created_at: now,
        modified_at: now,
        links: Vec::new(),
        review_log: Vec::new(),
        description: parts.description,
        expects_code_trace: None,
        active: parts.active,
        derived: parts.derived,
        tags: parts.tags,
        outline_level: parts.outline_level,
        legacy: None,
        blob_path: Some(binary_relative.clone()),
        url: None,
        checked_at: None,
        check_status: None,
        overflow: std::collections::BTreeMap::new(),
    };
    let binary_target = project.root.join(&binary_relative);
    let sidecar_target = reqforge_model::schema::sidecar::sidecar_path_for_blob(&binary_target);
    let project_root = project.root.clone();
    let overrides = state.overrides();
    drop(world);

    let metadata_for_write = metadata.clone();
    let bytes = parts.bytes;
    let write_result = tokio::task::spawn_blocking(move || {
        reqforge_model::write::write_blob_and_sidecar(
            &binary_target,
            &bytes,
            &sidecar_target,
            &project_root,
            &metadata_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("blob write failed: {err}")),
        Err(join) => return internal_error(format!("blob write task panicked: {join}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after upload failed: {err}"));
    }
    respond_with_current(&state, new_uuid, StatusCode::CREATED).await
}

/// PUT /api/artifacts/:uuid/blob — replace the binary on a blob
/// artifact. Preserves UUID / review log / links (per
/// `ART-uploadReplaceOnly`).
pub async fn replace_blob(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    multipart: Multipart,
) -> Response {
    let max = state.config().max_blob_bytes;
    let parts = match parse_blob_multipart(multipart, max).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    // The `name` / `title` fields on replace uploads are ignored;
    // replace is a binary swap, not a rename.
    let _ = (
        parts.name,
        parts.title,
        parts.description,
        parts.tags,
        parts.active,
        parts.derived,
        parts.outline_level,
    );

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(current) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };
    if current.metadata.shape != ArtifactShape::Blob {
        return bad_request("replace-blob only applies to blob-shape artifacts");
    }
    let Some(current_blob) = current.blob.as_ref() else {
        return internal_error("blob artifact has no blob facts");
    };

    let new_extension = std::path::Path::new(&parts.filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !reqforge_model::load::is_allowed_blob_extension(&new_extension) {
        return bad_request(format!(
            "extension '{new_extension}' is not in the blob allowlist",
        ));
    }
    if let Err(msg) = infer_matches_extension(&parts.bytes, &new_extension) {
        return bad_request(msg);
    }

    let old_binary = current_blob.binary_path.clone();
    let old_extension = old_binary
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let old_sidecar = current.source_path.clone();

    let new_binary = if new_extension == old_extension {
        old_binary.clone()
    } else {
        old_binary.with_extension(&new_extension)
    };
    let new_sidecar = reqforge_model::schema::sidecar::sidecar_path_for_blob(&new_binary);
    let project_root = project.root.clone();

    let mut metadata = current.metadata.clone();
    metadata.blob_path = Some(
        new_binary
            .strip_prefix(&project_root)
            .unwrap_or(&new_binary)
            .to_string_lossy()
            .into_owned(),
    );
    metadata.modified_at = chrono::Utc::now();
    let overrides = state.overrides();
    drop(world);

    let metadata_for_write = metadata.clone();
    let bytes = parts.bytes;
    let write_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        reqforge_model::write::write_blob_and_sidecar(
            &new_binary,
            &bytes,
            &new_sidecar,
            &project_root,
            &metadata_for_write,
            overrides,
        )
        .map_err(|err| format!("{err}"))?;
        if new_sidecar != old_sidecar && old_sidecar.exists() {
            let _ = std::fs::remove_file(&old_sidecar);
        }
        if new_binary != old_binary && old_binary.exists() {
            let _ = std::fs::remove_file(&old_binary);
        }
        Ok(())
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("replace-blob failed: {err}")),
        Err(join) => return internal_error(format!("replace-blob task panicked: {join}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after replace-blob failed: {err}"));
    }
    respond_with_current(&state, uuid, StatusCode::OK).await
}

/// POST /api/projects/:slug/collections/:prefix/artifacts/url —
/// create a URL artifact.
pub async fn create_url_artifact(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
    Json(req): Json<CreateUrlArtifactRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.name) {
        return bad_request(format!(
            "invalid artifact name '{}': must match [A-Za-z0-9._-]+",
            req.name
        ));
    }
    if let Err(msg) = validate_http_url(&req.url) {
        return bad_request(msg);
    }
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    if artifact_name_collides(collection, &req.name) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "artifact '{}' already exists in collection '{prefix}'",
                    req.name
                ),
            }),
        )
            .into_response();
    }
    let now = chrono::Utc::now();
    let new_uuid = Uuid::now_v7();
    let metadata = Artifact {
        schema_version: 1,
        uuid: new_uuid,
        title: req.title,
        shape: ArtifactShape::Url,
        created_at: now,
        modified_at: now,
        links: Vec::new(),
        review_log: Vec::new(),
        description: req.description,
        expects_code_trace: None,
        active: req.active,
        derived: req.derived,
        tags: req.tags,
        outline_level: req.outline_level,
        legacy: None,
        blob_path: None,
        url: Some(req.url),
        checked_at: None,
        check_status: None,
        overflow: std::collections::BTreeMap::new(),
    };
    let sidecar_target =
        collection
            .dir_path
            .join(reqforge_model::schema::sidecar::url_sidecar_filename(
                &req.name,
            ));
    let project_root = project.root.clone();
    let overrides = state.overrides();
    drop(world);

    let metadata_for_write = metadata.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        reqforge_model::write::write_sidecar_only(
            &sidecar_target,
            &project_root,
            &metadata_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("URL sidecar write failed: {err}")),
        Err(join) => return internal_error(format!("URL sidecar write panicked: {join}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after URL create failed: {err}"));
    }
    respond_with_current(&state, new_uuid, StatusCode::CREATED).await
}

/// GET /api/artifacts/:uuid/blob — stream the binary.
pub async fn download_blob(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Query(q): Query<AtCommitQuery>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };
    if artifact.metadata.shape != ArtifactShape::Blob {
        return not_found(format!("artifact {uuid} is not a blob"));
    }
    let Some(blob) = artifact.blob.as_ref() else {
        return internal_error("blob artifact missing blob facts");
    };
    let binary_path = blob.binary_path.clone();
    let media_type = blob.media_type.to_owned();
    let etag_live = format!("\"{}\"", blob.content_hash);
    let display_filename = binary_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("blob")
        .to_owned();

    // Phase 5d: `?at=<oid>` resolves the binary at a historical
    // commit via the gitoxide cache on AppState. No ETag is
    // emitted for historical reads because we don't recompute the
    // content hash on the fly (the cost would dwarf the request).
    if let Some(oid) = q.at.clone() {
        // Resolve the repo root (the `.git` ancestor of the project dir, #379) and make the blob
        // path relative to it, so provreq's split layout (`.git` at the repo root, artifacts in a
        // `requirements/` subdir) resolves history instead of failing to open `requirements/.git`.
        let git_root = project.git_root().to_path_buf();
        let repo_rel = match repo_relative_path(&git_root, &binary_path) {
            Some(p) => p,
            None => return internal_error("blob path outside project root"),
        };
        drop(world);
        let repo = match state.repo_cache().open(&git_root.join(".git")) {
            Ok(r) => r,
            Err(err) => {
                return history_unavailable(&format!("repo open failed: {err}"));
            }
        };
        let result = tokio::task::spawn_blocking(move || {
            reqforge_model::git_history::read_blob_at_commit(&repo, &oid, &repo_rel)
        })
        .await;
        let bytes = match result {
            Ok(Ok(b)) => b,
            Ok(Err(err)) => return history_unavailable(&err.to_string()),
            Err(join) => return internal_error(format!("at-commit task panicked: {join}")),
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, media_type)
            .header(
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{display_filename}\""),
            )
            .body(Body::from(bytes))
            .unwrap();
    }
    drop(world);

    let bytes = match tokio::task::spawn_blocking(move || std::fs::read(&binary_path)).await {
        Ok(Ok(b)) => b,
        Ok(Err(err)) => return internal_error(format!("reading blob: {err}")),
        Err(join) => return internal_error(format!("blob read task panicked: {join}")),
    };
    let etag = etag_live;
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, media_type)
        .header(header::ETAG, etag)
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{display_filename}\""),
        )
        .body(Body::from(bytes))
        .unwrap();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "private, max-age=0, must-revalidate".parse().unwrap(),
    );
    response
}

/// GET /api/artifacts/:uuid/thumbnail — stream the cached 512 px
/// PNG for a blob artifact. Cache hits serve the file directly;
/// cache misses shell out to the first provider that accepts the
/// media type under a global concurrency cap (see
/// `thumbnails::ThumbnailRegistry::get_or_generate`). Structured
/// 404 JSON indicates an absent provider or a missing workspace.
pub async fn get_thumbnail(State(state): State<Arc<AppState>>, Path(uuid): Path<Uuid>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };
    if artifact.metadata.shape != ArtifactShape::Blob {
        return not_found(format!("artifact {uuid} is not a blob"));
    }
    let Some(blob) = artifact.blob.as_ref() else {
        return internal_error("blob artifact missing blob facts");
    };

    let Some(registry) = state.thumbnail_registry() else {
        return thumbnail_not_found(
            "workspace-not-configured",
            "set REQFORGE_WORKSPACE_DIR to enable the thumbnail cache",
        );
    };

    let content_hash = blob.content_hash.clone();
    let media_type = blob.media_type.to_owned();
    let binary_path = blob.binary_path.clone();
    drop(world);

    let registry = registry.clone();
    let thumb_path = match registry
        .get_or_generate(&content_hash, &media_type, &binary_path)
        .await
    {
        Ok(path) => path,
        Err(reqforge_model::thumbnails::ThumbnailError::NoProviderForMediaType { media_type }) => {
            return thumbnail_not_found(
                "no-thumbnailer-for-format",
                &format!("no provider accepts media type {media_type}"),
            );
        }
        Err(err) => {
            tracing::warn!(%uuid, error = %err, "thumbnail generation failed");
            return internal_error(format!("thumbnail generation failed: {err}"));
        }
    };

    let bytes = match tokio::task::spawn_blocking(move || std::fs::read(&thumb_path)).await {
        Ok(Ok(b)) => b,
        Ok(Err(err)) => return internal_error(format!("reading thumbnail: {err}")),
        Err(join) => return internal_error(format!("thumbnail read task panicked: {join}")),
    };

    let etag = format!("\"{content_hash}\"");
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::ETAG, etag)
        .body(Body::from(bytes))
        .unwrap();
    // Thumbnails are content-hash keyed, so the cached response is
    // safe to hold for a long time. The client-side query cache
    // invalidates on artifact mutation anyway.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=86400, immutable".parse().unwrap(),
    );
    response
}

fn thumbnail_not_found(reason: &str, detail: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": detail,
            "reason": reason,
        })),
    )
        .into_response()
}

/// POST /api/artifacts/:uuid/check-url — run a single URL check
/// and persist the outcome to the sidecar.
pub async fn check_url(State(state): State<Arc<AppState>>, Path(uuid): Path<Uuid>) -> Response {
    let outcome = match run_url_check(&state, uuid).await {
        Ok(outcome) => outcome,
        Err(resp) => return resp,
    };
    (
        StatusCode::OK,
        Json(CheckUrlResponse {
            uuid: outcome.uuid,
            checked_at: outcome.checked_at,
            check_status: outcome.check_status,
        }),
    )
        .into_response()
}

/// POST /api/projects/:slug/collections/:prefix/check-urls —
/// bulk-check every URL artifact in the collection (or the subset
/// named in the request body).
pub async fn bulk_check_urls(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
    body: Option<axum::Json<serde_json::Value>>,
) -> Response {
    // Accept an absent body, a null body, or a well-formed JSON
    // body. Only the `uuids` field is meaningful; everything else
    // is ignored for forward-compat.
    let filter = body.and_then(|b| {
        if b.0.is_null() {
            return None;
        }
        serde_json::from_value::<BulkCheckUrlsRequest>(b.0)
            .ok()
            .and_then(|req| req.uuids)
    });
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    let mut targets: Vec<Uuid> = Vec::new();
    for artifact in &collection.artifacts {
        if artifact.metadata.shape != ArtifactShape::Url {
            continue;
        }
        match &filter {
            Some(selected) if !selected.contains(&artifact.metadata.uuid) => continue,
            _ => {}
        }
        targets.push(artifact.metadata.uuid);
    }
    drop(world);

    let mut checked: Vec<CheckUrlResponse> = Vec::with_capacity(targets.len());
    for uuid in targets {
        match run_url_check(&state, uuid).await {
            Ok(outcome) => checked.push(CheckUrlResponse {
                uuid: outcome.uuid,
                checked_at: outcome.checked_at,
                check_status: outcome.check_status,
            }),
            Err(_) => {
                // Failures within the batch don't abort per
                // UX-urlArtifactChecking. The UI surfaces per-entry
                // results via the individual artifact detail after
                // the batch refresh.
            }
        }
    }
    Json(BulkCheckUrlsResponse { checked }).into_response()
}

struct UrlCheckOutcomeRaw {
    uuid: Uuid,
    checked_at: chrono::DateTime<chrono::Utc>,
    check_status: String,
}

async fn run_url_check(state: &Arc<AppState>, uuid: Uuid) -> Result<UrlCheckOutcomeRaw, Response> {
    let world = state.snapshot().await.ok_or_else(service_unavailable)?;
    let location = world
        .index
        .get(&uuid)
        .ok_or_else(|| not_found(format!("artifact {uuid} not found")))?;
    let project = find_project(&world, &location.project_slug)
        .ok_or_else(|| internal_error("index references a project that isn't loaded"))?;
    let collection = find_collection(project, &location.collection_prefix)
        .ok_or_else(|| internal_error("index references a collection that isn't loaded"))?;
    let current = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
        .ok_or_else(|| internal_error("index references an artifact that isn't loaded"))?;
    if current.metadata.shape != ArtifactShape::Url {
        return Err(bad_request("check-url only applies to URL-shape artifacts"));
    }
    let url = current
        .metadata
        .url
        .clone()
        .ok_or_else(|| internal_error("URL artifact missing url field"))?;
    let sidecar_path = current.source_path.clone();
    let project_root = project.root.clone();
    let mut metadata = current.metadata.clone();
    let overrides = state.overrides();
    drop(world);

    let outcome = state.url_check_client().check(&url).await;
    let now = chrono::Utc::now();
    metadata.checked_at = Some(now);
    metadata.check_status = Some(outcome.as_wire().to_owned());
    metadata.modified_at = now;

    let metadata_for_write = metadata.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        reqforge_model::write::write_sidecar_only(
            &sidecar_path,
            &project_root,
            &metadata_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            return Err(internal_error(format!("persist check-url outcome: {err}")));
        }
        Err(join) => {
            return Err(internal_error(format!("check-url write panicked: {join}")));
        }
    }
    if let Err(err) = state.refresh().await {
        return Err(internal_error(format!(
            "refresh after check-url failed: {err}"
        )));
    }
    Ok(UrlCheckOutcomeRaw {
        uuid,
        checked_at: now,
        check_status: outcome.as_wire().to_owned(),
    })
}

fn validate_http_url(raw: &str) -> Result<(), &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("url must not be empty");
    }
    let parsed = url::Url::parse(trimmed).map_err(|_| "url is not valid")?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("url scheme must be http or https"),
    }
}

fn relative_blob_path(
    project: &LoadedProject,
    collection: &LoadedCollection,
    name: &str,
    extension: &str,
) -> String {
    // Store paths with forward slashes regardless of host OS so
    // sidecar JSON round-trips identically across platforms.
    let artifacts_root = project.config.effective_artifacts_path();
    format!(
        "{}/{}/{}.{}",
        artifacts_root, collection.dir_name, name, extension,
    )
}

fn artifact_name_collides(collection: &LoadedCollection, name: &str) -> bool {
    collection.artifacts.iter().any(|a| a.name == name)
}

fn infer_matches_extension(bytes: &[u8], declared_extension: &str) -> Result<(), String> {
    // `infer` returns None on short / unknown inputs; for formats
    // we can't sniff we trust the claimed extension (the allowlist
    // has already filtered the set of acceptable values).
    let Some(sniffed) = infer::get(bytes) else {
        return Ok(());
    };
    let sniffed_ext = sniffed.extension();
    let matches = match declared_extension {
        "jpg" | "jpeg" => sniffed_ext == "jpg" || sniffed_ext == "jpeg",
        "svg" => sniffed_ext == "svg" || sniffed_ext == "xml",
        other => sniffed_ext == other,
    };
    if matches {
        Ok(())
    } else {
        Err(format!(
            "uploaded bytes look like '{sniffed_ext}', not '{declared_extension}'",
        ))
    }
}

async fn respond_with_current(state: &Arc<AppState>, uuid: Uuid, status: StatusCode) -> Response {
    let Some(fresh) = state.snapshot().await else {
        return internal_error("world missing after refresh");
    };
    let Some(location) = fresh.index.get(&uuid) else {
        return internal_error("artifact vanished after refresh");
    };
    let Some(project) = find_project(&fresh, &location.project_slug) else {
        return internal_error("project missing after refresh");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("collection missing after refresh");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("artifact missing after refresh");
    };
    (
        status,
        Json(ArtifactDetail::from_loaded(
            artifact,
            &project.config.slug,
            &collection.config.prefix,
            &fresh,
        )),
    )
        .into_response()
}

// ---- Phase 5d: history, at-commit, and diff handlers ----

/// GET /api/artifacts/:uuid/history — commits that touched the
/// artifact's source file. Walks `HEAD` through
/// [`reqforge_model::git_history::list_artifact_commits`] up to
/// `HISTORY_COMMIT_CAP`. Returns a 200 with an empty commits
/// list + a `fallbackReason` when history is unavailable.
pub async fn get_history(State(state): State<Arc<AppState>>, Path(uuid): Path<Uuid>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };

    // Repo root = the `.git` ancestor of the project dir (#379); the tracked path is made relative
    // to it so blob lookups key off the repo root, which is what provreq's split layout needs.
    let git_root = project.git_root().to_path_buf();
    let tracked_path = history_tracked_path(artifact);
    let Some(repo_rel) = repo_relative_path(&git_root, &tracked_path) else {
        return Json(ArtifactHistoryResponse {
            commits: Vec::new(),
            fallback_reason: Some("artifact source path is not inside the project root".to_owned()),
        })
        .into_response();
    };
    drop(world);

    let repo = match state.repo_cache().open(&git_root.join(".git")) {
        Ok(r) => r,
        Err(err) => {
            return Json(ArtifactHistoryResponse {
                commits: Vec::new(),
                fallback_reason: Some(format!("git history unavailable (repo open failed: {err})")),
            })
            .into_response();
        }
    };

    let result = tokio::task::spawn_blocking(move || {
        reqforge_model::git_history::list_artifact_commits(&repo, &repo_rel)
    })
    .await;
    match result {
        Ok(Ok(commits)) => Json(ArtifactHistoryResponse {
            commits,
            fallback_reason: None,
        })
        .into_response(),
        Ok(Err(err)) => Json(ArtifactHistoryResponse {
            commits: Vec::new(),
            fallback_reason: Some(format!("git history unavailable: {err}")),
        })
        .into_response(),
        Err(join) => internal_error(format!("history task panicked: {join}")),
    }
}

/// GET /api/artifacts/:uuid/diff?from=<oid>&to=<oid|current> —
/// shape-aware structured diff. Content bodies use
/// [`reqforge_model::diff::diff_content`]; blob and URL artifacts report
/// side-by-side metadata deltas. Falls back to the Phase 4b
/// approval snapshot with a banner when git history can't resolve
/// `from` / `to`.
pub async fn get_diff(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
    Query(q): Query<DiffQuery>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let Some(project) = find_project(&world, &location.project_slug) else {
        return internal_error("index references a project that isn't loaded");
    };
    let Some(collection) = find_collection(project, &location.collection_prefix) else {
        return internal_error("index references a collection that isn't loaded");
    };
    let Some(artifact) = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
    else {
        return internal_error("index references an artifact that isn't loaded");
    };

    let shape = artifact.metadata.shape;
    let from_label = q.from.chars().take(10).collect::<String>();
    let to_label = match q.to.as_deref() {
        None | Some("current") => "working tree".to_owned(),
        Some(oid) => oid.chars().take(10).collect::<String>(),
    };
    // Repo root = the `.git` ancestor of the project dir (#379); repo_rel keys off it.
    let git_root = project.git_root().to_path_buf();
    let tracked_path = history_tracked_path(artifact);
    let Some(repo_rel) = repo_relative_path(&git_root, &tracked_path) else {
        return internal_error("artifact source path is not inside the project root");
    };
    let current_bytes = if shape == ArtifactShape::Blob {
        // Blob diff is metadata-only; we don't need the binary
        // bytes for "current" — the in-memory blob facts cover it.
        None
    } else {
        match tokio::fs::read(&tracked_path).await {
            Ok(b) => Some(b),
            Err(err) => {
                return internal_error(format!("reading current source: {err}"));
            }
        }
    };
    let current_blob_facts = artifact
        .blob
        .as_ref()
        .map(|f| (f.byte_size, f.content_hash.clone(), f.media_type.to_owned()));
    drop(world);

    let repo_cache = state.repo_cache().clone();
    let git_dir = git_root.join(".git");
    let repo = match repo_cache.open(&git_dir) {
        Ok(r) => r,
        Err(err) => {
            return diff_fallback_response(
                shape,
                &from_label,
                &to_label,
                &format!("git history unavailable (repo open failed: {err})"),
            );
        }
    };

    let from_oid = q.from.clone();
    let to_oid = q.to.clone();
    let repo_rel_from = repo_rel.clone();
    let repo_rel_to = repo_rel.clone();
    let repo_for_from = repo.clone();
    let repo_for_to = repo.clone();
    let from_bytes = tokio::task::spawn_blocking(move || {
        reqforge_model::git_history::read_blob_at_commit(&repo_for_from, &from_oid, &repo_rel_from)
    })
    .await;

    let from_bytes = match from_bytes {
        Ok(Ok(b)) => b,
        Ok(Err(err)) => {
            return diff_fallback_response(shape, &from_label, &to_label, &err.to_string());
        }
        Err(join) => return internal_error(format!("from-commit task panicked: {join}")),
    };

    let to_bytes_opt = match to_oid.as_deref() {
        None | Some("current") => current_bytes,
        Some(oid) => {
            let oid = oid.to_owned();
            let join = tokio::task::spawn_blocking(move || {
                reqforge_model::git_history::read_blob_at_commit(&repo_for_to, &oid, &repo_rel_to)
            })
            .await;
            match join {
                Ok(Ok(b)) => Some(b),
                Ok(Err(err)) => {
                    return diff_fallback_response(shape, &from_label, &to_label, &err.to_string());
                }
                Err(je) => return internal_error(format!("to-commit task panicked: {je}")),
            }
        }
    };

    let diff = build_shape_diff(
        shape,
        uuid,
        &from_bytes,
        to_bytes_opt.as_deref(),
        current_blob_facts.as_ref(),
    );

    Json(ArtifactDiffResponse {
        shape,
        from_label,
        to_label,
        diff,
        fallback_reason: None,
    })
    .into_response()
}

/// Build a `ShapeDiff` from two byte slices. For content shapes we
/// decode as UTF-8 and run `similar` over the frontmatter-stripped
/// body. For blob shapes we derive size/hash/media-type from the
/// raw bytes rather than parsing a sidecar. For URL shapes we
/// parse the sidecar JSON and compare the `url` strings.
fn build_shape_diff(
    shape: ArtifactShape,
    uuid: Uuid,
    from_bytes: &[u8],
    to_bytes: Option<&[u8]>,
    current_blob_facts: Option<&(u64, String, String)>,
) -> reqforge_model::diff::ShapeDiff {
    use reqforge_model::diff::{BlobSide, diff_blob, diff_content, diff_url};
    match shape {
        ArtifactShape::Content => {
            let before = String::from_utf8_lossy(from_bytes).into_owned();
            let after = to_bytes
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let before_body = strip_frontmatter(&before);
            let after_body = strip_frontmatter(&after);
            reqforge_model::diff::ShapeDiff::Content(diff_content(
                Some(&before_body),
                Some(&after_body),
            ))
        }
        ArtifactShape::Blob => {
            let before = blob_side_from_bytes(uuid, from_bytes);
            let after = match to_bytes {
                Some(b) => Some(blob_side_from_bytes(uuid, b)),
                None => current_blob_facts.map(|(size, hash, media)| BlobSide {
                    byte_size: *size,
                    content_hash: hash.clone(),
                    media_type: media.clone(),
                    download_url: format!("/api/artifacts/{uuid}/blob"),
                }),
            };
            reqforge_model::diff::ShapeDiff::Blob(diff_blob(Some(before), after))
        }
        ArtifactShape::Url => {
            let before = parse_url_from_sidecar(from_bytes);
            let after = match to_bytes {
                Some(b) => parse_url_from_sidecar(b),
                None => None,
            };
            reqforge_model::diff::ShapeDiff::Url(diff_url(before, after))
        }
    }
}

fn strip_frontmatter(text: &str) -> String {
    // The same four-corners YAML-fenced JSON frontmatter the loader
    // accepts. Best-effort strip — we'd rather over-diff than
    // under-diff if a file lands in an odd state.
    if let Some(stripped) = text.strip_prefix("---\n")
        && let Some(end) = stripped.find("\n---\n")
    {
        return stripped[end + 5..].to_owned();
    }
    text.to_owned()
}

fn blob_side_from_bytes(uuid: Uuid, bytes: &[u8]) -> reqforge_model::diff::BlobSide {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hex_digest(&hasher.finalize());
    let media_type = infer::get(bytes)
        .map(|t| t.mime_type().to_owned())
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    reqforge_model::diff::BlobSide {
        byte_size: bytes.len() as u64,
        content_hash: hash,
        media_type,
        download_url: format!("/api/artifacts/{uuid}/blob"),
    }
}

fn parse_url_from_sidecar(bytes: &[u8]) -> Option<String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    v.get("url").and_then(|u| u.as_str()).map(|s| s.to_owned())
}

fn hex_digest(bytes: &[u8]) -> String {
    static CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(CHARS[(byte >> 4) as usize] as char);
        out.push(CHARS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// On a history miss, return 200 + `fallbackReason` with an empty
/// shape-appropriate diff instead of a 4xx. The frontend renders
/// the banner and falls back to the Phase 4b approval snapshot for
/// the "since last approval" pane.
fn diff_fallback_response(
    shape: ArtifactShape,
    from_label: &str,
    to_label: &str,
    reason: &str,
) -> Response {
    let empty = match shape {
        ArtifactShape::Content => reqforge_model::diff::ShapeDiff::Content(
            reqforge_model::diff::diff_content(Some(""), Some("")),
        ),
        ArtifactShape::Blob => {
            reqforge_model::diff::ShapeDiff::Blob(reqforge_model::diff::diff_blob(None, None))
        }
        ArtifactShape::Url => {
            reqforge_model::diff::ShapeDiff::Url(reqforge_model::diff::diff_url(None, None))
        }
    };
    Json(ArtifactDiffResponse {
        shape,
        from_label: from_label.to_owned(),
        to_label: to_label.to_owned(),
        diff: empty,
        fallback_reason: Some(reason.to_owned()),
    })
    .into_response()
}

/// Historical `ArtifactDetail`. Returns an `Err(Box<Response>)`
/// so the caller can early-return a pre-built response without
/// inflating every caller's stack frame for the happy path
/// (clippy's `result_large_err`).
#[allow(clippy::result_large_err)]
fn historical_artifact_detail(
    state: &Arc<AppState>,
    world: &crate::app::World,
    project: &LoadedProject,
    collection: &LoadedCollection,
    artifact: &reqforge_model::load::LoadedArtifact,
    oid: &str,
) -> Result<ArtifactDetail, Response> {
    let tracked_path = history_tracked_path(artifact);
    // Repo root = the `.git` ancestor of the project dir (#379); repo_rel keys off it so the split
    // layout (artifacts in a `requirements/` subdir of the repo) reads blobs at the right path.
    let git_root = project.git_root();
    let repo_rel = repo_relative_path(git_root, &tracked_path)
        .ok_or_else(|| internal_error("artifact source path is not inside the project root"))?;
    let repo = state
        .repo_cache()
        .open(&git_root.join(".git"))
        .map_err(|err| history_unavailable(&format!("repo open failed: {err}")))?;
    let bytes = reqforge_model::git_history::read_blob_at_commit(&repo, oid, &repo_rel)
        .map_err(|err| history_unavailable(&err.to_string()))?;

    // Reuse the wire DTO's `from_loaded` by rebuilding a
    // LoadedArtifact-ish view for the historical payload. Links on
    // the historical view resolve against the *current* world on
    // purpose (per UX-diffView) — stale unresolved flags are
    // expected and acceptable on historical frames.
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let (historical_metadata, historical_body) = match artifact.metadata.shape {
        ArtifactShape::Content => {
            let (frontmatter, body) = reqforge_model::frontmatter::split_frontmatter(&text)
                .map_err(|err| history_unavailable(&format!("parse frontmatter: {err}")))?;
            let meta: reqforge_model::schema::Artifact = serde_json::from_str(frontmatter)
                .map_err(|err| history_unavailable(&format!("parse metadata: {err}")))?;
            (meta, Some(body.to_owned()))
        }
        ArtifactShape::Blob | ArtifactShape::Url => {
            let meta: reqforge_model::schema::Artifact = serde_json::from_slice(&bytes)
                .map_err(|err| history_unavailable(&format!("parse sidecar at commit: {err}")))?;
            (meta, None)
        }
    };

    let historical = reqforge_model::load::LoadedArtifact {
        name: artifact.name.clone(),
        source_path: artifact.source_path.clone(),
        metadata: historical_metadata,
        body: historical_body,
        blob: artifact.blob.clone(),
    };
    Ok(ArtifactDetail::from_loaded(
        &historical,
        &project.config.slug,
        &collection.config.prefix,
        world,
    ))
}

/// The file whose history we consult for a given artifact. Content
/// artifacts track the .md file; blob and URL artifacts track the
/// sidecar (`source_path` is the sidecar for non-content shapes).
fn history_tracked_path(artifact: &reqforge_model::load::LoadedArtifact) -> std::path::PathBuf {
    artifact.source_path.clone()
}

fn repo_relative_path(
    project_root: &std::path::Path,
    abs: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let rel = abs.strip_prefix(project_root).ok()?;
    Some(rel.to_path_buf())
}

fn history_unavailable(reason: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: format!("history unavailable: {reason}"),
        }),
    )
        .into_response()
}

// ---- Phase 6a: report catalog + saved-config endpoints ----

/// GET /api/reports/:kind — unified report endpoint. Dispatches
/// on the kind segment to the compute module; stubs the
/// not-yet-implemented kinds with a structured
/// `ReportResponse::Unimplemented` so the frontend wiring is
/// visible before every report class lights up.
pub async fn run_report(
    State(state): State<Arc<AppState>>,
    Path(kind_str): Path<String>,
    Query(query): Query<reqforge_model::reports::ReportQuery>,
) -> Response {
    let Some(kind) = reqforge_model::reports::ReportKind::from_kebab(&kind_str) else {
        return not_found(format!("unknown report kind '{kind_str}'"));
    };
    let scope = match reqforge_model::reports::Scope::parse(query.scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(err.to_string()),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    match reqforge_model::reports::run_report(kind, scope, &query, &world) {
        Ok(resp) => Json(resp).into_response(),
        Err(reqforge_model::reports::ReportError::ProjectNotMounted(slug)) => {
            not_found(format!("project '{slug}' is not currently mounted"))
        }
        Err(reqforge_model::reports::ReportError::CollectionNotFound { slug, prefix }) => {
            not_found(format!(
                "collection '{prefix}' not found in project '{slug}'"
            ))
        }
        Err(reqforge_model::reports::ReportError::InvalidDirection(dir)) => bad_request(format!(
            "invalid direction '{dir}'; expected 'dependents' or 'dependencies'"
        )),
    }
}

/// GET /api/reports/:kind/config — read a report's saved config.
pub async fn read_report_config(
    State(state): State<Arc<AppState>>,
    Path(kind_str): Path<String>,
) -> Response {
    let Some(kind) = reqforge_model::reports::ReportKind::from_kebab(&kind_str) else {
        return not_found(format!("unknown report kind '{kind_str}'"));
    };
    let cfg =
        reqforge_model::reports::saved_config::load(state.config().workspace_dir.as_deref(), kind);
    Json(cfg.inner).into_response()
}

/// PUT /api/reports/:kind/config — persist a report's saved
/// config. Body is the opaque JSON object the frontend wants
/// round-tripped; 409 when the workspace dir isn't configured.
pub async fn write_report_config(
    State(state): State<Arc<AppState>>,
    Path(kind_str): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(kind) = reqforge_model::reports::ReportKind::from_kebab(&kind_str) else {
        return not_found(format!("unknown report kind '{kind_str}'"));
    };
    let cfg = reqforge_model::reports::saved_config::SavedReportConfig::from_value(body);
    match reqforge_model::reports::saved_config::save(
        state.config().workspace_dir.as_deref(),
        kind,
        &cfg,
    ) {
        Ok(()) => (StatusCode::NO_CONTENT, Body::empty()).into_response(),
        Err(reqforge_model::reports::saved_config::SavedConfigError::NoWorkspace) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "workspace directory is not configured — saved report configs are disabled"
                    .to_owned(),
            }),
        )
            .into_response(),
        Err(err) => internal_error(format!("saving report config: {err}")),
    }
}

/// DELETE /api/reports/:kind/config — clear a report's saved
/// config ("reset to defaults"). Idempotent: missing file → 204.
pub async fn clear_report_config(
    State(state): State<Arc<AppState>>,
    Path(kind_str): Path<String>,
) -> Response {
    let Some(kind) = reqforge_model::reports::ReportKind::from_kebab(&kind_str) else {
        return not_found(format!("unknown report kind '{kind_str}'"));
    };
    match reqforge_model::reports::saved_config::clear(
        state.config().workspace_dir.as_deref(),
        kind,
    ) {
        Ok(()) => (StatusCode::NO_CONTENT, Body::empty()).into_response(),
        Err(err) => internal_error(format!("clearing report config: {err}")),
    }
}

// ---- Phase 9a: code-traceability scan (debug) ----

/// GET /api/projects/:slug/code-scan — raw scanner output
/// for one project. Exists in 9a so the subsystem is end-to-
/// end testable; 9b wraps it into the actual report.
pub async fn code_scan(State(state): State<Arc<AppState>>, Path(slug): Path<String>) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    if find_project(&world, &slug).is_none() {
        return not_found(format!("project '{slug}' not found"));
    }
    // Scanner walks the filesystem synchronously; hand it to
    // spawn_blocking so we don't stall the axum worker thread
    // on a large repository. The blocking closure takes
    // ownership of the slug so the scanner can look up the
    // project from the cloned World snapshot inside the
    // thread.
    let slug_owned = slug.clone();
    let output = match tokio::task::spawn_blocking(move || {
        let project = find_project(&world, &slug_owned)
            .expect("project presence verified before spawn_blocking");
        reqforge_model::scan::run_scan(project, &world)
    })
    .await
    {
        Ok(output) => output,
        Err(err) => return internal_error(format!("scan task panicked: {err}")),
    };
    Json(output).into_response()
}

// ---- Phase 8: doorstop import (preview) ----

/// Request body for both the preview and the import endpoints.
/// `source` is a project-root-relative path (may equal the
/// project root with `.` or `""`) to the directory to scan
/// for `.doorstop.yml` markers.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoorstopImportRequest {
    pub source: String,
}

/// POST /api/projects/:slug/doorstop/preview — parse the
/// doorstop source tree and return the full import plan
/// without writing anything. Operators use the response to
/// resolve prefix collisions and review the plan before
/// committing.
pub async fn doorstop_preview(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<DoorstopImportRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let source_root = match resolve_doorstop_source(project, &req.source) {
        Ok(p) => p,
        Err(msg) => return bad_request(msg),
    };
    let documents = match reqforge_model::doorstop::parse::discover(&source_root) {
        Ok(d) => d,
        Err(err) => return bad_request(format!("doorstop parse error: {err}")),
    };
    match reqforge_model::doorstop::plan::build_plan(project, documents, chrono::Utc::now()) {
        Ok(plan) => Json(plan).into_response(),
        Err(err) => internal_error(format!("doorstop plan error: {err}")),
    }
}

/// POST /api/projects/:slug/doorstop/import — run the preview
/// pipeline and then write the files. Refuses when any prefix
/// collision is present (per
/// INTEROP-doorstopPrefixCollision). On success the resulting
/// import report is returned and cached on AppState for the
/// later `GET /doorstop/report` + exports to re-serve.
pub async fn doorstop_import(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<DoorstopImportRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let source_root = match resolve_doorstop_source(project, &req.source) {
        Ok(p) => p,
        Err(msg) => return bad_request(msg),
    };
    let documents = match reqforge_model::doorstop::parse::discover(&source_root) {
        Ok(d) => d,
        Err(err) => return bad_request(format!("doorstop parse error: {err}")),
    };
    let plan =
        match reqforge_model::doorstop::plan::build_plan(project, documents, chrono::Utc::now()) {
            Ok(p) => p,
            Err(err) => return internal_error(format!("doorstop plan error: {err}")),
        };
    if !plan.prefix_collisions.is_empty() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "prefix collision — resolve and re-run",
                "collisions": plan.prefix_collisions,
            })),
        )
            .into_response();
    }

    let overrides = state.overrides();
    let target = reqforge_model::doorstop::ExecuteTarget::from_project(project);
    let source_label = req.source.clone();
    drop(world);
    let report_result = tokio::task::spawn_blocking(move || {
        reqforge_model::doorstop::execute(&target, &source_label, plan, overrides)
    })
    .await;
    let report = match report_result {
        Ok(Ok(r)) => r,
        Ok(Err(err)) => return internal_error(format!("doorstop execute: {err}")),
        Err(join) => return internal_error(format!("doorstop task panicked: {join}")),
    };

    state
        .set_doorstop_report(slug.clone(), report.clone())
        .await;
    // Rediscover so the newly-written Collections land in
    // World immediately — the frontend picks up the import
    // via the same SSE / cache-invalidation path CRUD writes
    // use.
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after import failed: {err}"));
    }
    Json(report).into_response()
}

/// GET /api/projects/:slug/doorstop/report — re-serve the
/// latest import report held in memory for the project, or
/// 404 if no import has run this process.
pub async fn doorstop_report(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    match state.get_doorstop_report(&slug).await {
        Some(report) => Json((*report).clone()).into_response(),
        None => not_found(format!(
            "no doorstop import report for project '{slug}' (none run this process)"
        )),
    }
}

/// GET /api/projects/:slug/doorstop/report/export/{ext} —
/// serve the cached doorstop import report as JSON, CSV, or
/// HTML via the Phase 6b export scaffolding.
pub async fn doorstop_report_export(
    State(state): State<Arc<AppState>>,
    Path((slug, ext)): Path<(String, String)>,
) -> Response {
    let Some(format) = reqforge_model::exports::ExportFormat::from_ext(&ext) else {
        return not_found(format!("unknown export format '{ext}'"));
    };
    let Some(report) = state.get_doorstop_report(&slug).await else {
        return not_found(format!(
            "no doorstop import report for project '{slug}' (none run this process)"
        ));
    };
    let filename = format!(
        "reqforge-doorstop-import-{}-{}.{}",
        slug,
        report.import_run_at.format("%Y%m%dT%H%M%SZ"),
        format.ext()
    );
    let (bytes, mime) = match format {
        reqforge_model::exports::ExportFormat::Json => {
            let body = match serde_json::to_vec_pretty(&*report) {
                Ok(b) => b,
                Err(err) => return internal_error(format!("serialize: {err}")),
            };
            (body, format.mime())
        }
        reqforge_model::exports::ExportFormat::Csv => (
            reqforge_model::exports::doorstop::render_csv(&report),
            format.mime(),
        ),
        reqforge_model::exports::ExportFormat::Html => (
            reqforge_model::exports::doorstop::render_html(&report),
            format.mime(),
        ),
    };
    (
        [
            ("content-type", mime.to_string()),
            (
                "content-disposition",
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// Resolve the project-root-relative `source` string into an
/// absolute path that's guaranteed to stay inside the project
/// root. Mirrors the Phase 6a orphan-adopt traversal check.
fn resolve_doorstop_source(
    project: &reqforge_model::load::LoadedProject,
    source: &str,
) -> Result<std::path::PathBuf, String> {
    let declared = source.replace('\\', "/");
    if declared.contains("..") || declared.starts_with('/') {
        return Err("source must be a forward-slash project-root-relative path".to_owned());
    }
    let target = if declared.is_empty() || declared == "." {
        project.root.clone()
    } else {
        project.root.join(&declared)
    };
    if !target.exists() {
        return Err(format!(
            "source path '{}' does not exist under project root",
            declared
        ));
    }
    if !target.is_dir() {
        return Err(format!("source path '{}' is not a directory", declared));
    }
    let canonical_project = project.root.canonicalize().ok();
    let canonical_target = target.canonicalize().ok();
    let inside = match (&canonical_project, &canonical_target) {
        (Some(p), Some(t)) => t.starts_with(p),
        _ => true,
    };
    if !inside {
        return Err("source path must stay inside the project root".to_owned());
    }
    Ok(target)
}

// ---- Phase 7d: browse-by-type endpoint ----

/// GET /api/browse — browse artifacts grouped by Collection
/// prefix (per UX-browseByType). Scope + tag + review-state +
/// include-inactive filters mirror the Phase 7c search
/// vocabulary so operators don't relearn the knobs.
pub async fn browse(
    State(state): State<Arc<AppState>>,
    Query(query): Query<reqforge_model::browse::BrowseQuery>,
) -> Response {
    let scope = match reqforge_model::reports::Scope::parse(query.scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(err.to_string()),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    match reqforge_model::browse::run(scope, &query, &world) {
        Ok(resp) => Json(resp).into_response(),
        Err(reqforge_model::browse::BrowseError::ProjectNotMounted(slug)) => {
            not_found(format!("project '{slug}' is not currently mounted"))
        }
        Err(reqforge_model::browse::BrowseError::CollectionNotFound { slug, prefix }) => not_found(
            format!("collection '{prefix}' not found in project '{slug}'"),
        ),
        Err(reqforge_model::browse::BrowseError::UnknownReviewStates(list)) => {
            bad_request(format!("unknown review state(s): {list}"))
        }
    }
}

// ---- Phase 7c: full-text search endpoint ----

/// GET /api/search — Tantivy full-text search over artifact
/// title, short name, body, description, and tags (per
/// UX-search). Structured filters (scope, shape, review state,
/// has-links, active) AND onto the text query. Empty `q` runs
/// a match-all so pure-filter searches work.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<reqforge_model::search::SearchQuery>,
) -> Response {
    let scope = match reqforge_model::reports::Scope::parse(query.scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(err.to_string()),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    // Scope validation for project / collection filters:
    // mirror the Phase 7a / 7b 404 mapping so operators see a
    // consistent "not mounted" message across views.
    if let Err(err) = ensure_search_scope_exists(&scope, &world) {
        return not_found(err);
    }
    let scope_filter = reqforge_model::search::query::ScopeFilter::from_reports_scope(&scope);
    match reqforge_model::search::run(&world.search_index, scope_filter, &query) {
        Ok(resp) => Json(resp).into_response(),
        Err(reqforge_model::search::SearchError::BadQuery(msg)) => {
            bad_request(format!("malformed query: {msg}"))
        }
        Err(reqforge_model::search::SearchError::UnknownReviewStates(list)) => {
            bad_request(format!("unknown review state(s): {list}"))
        }
        Err(reqforge_model::search::SearchError::UnknownShapes(list)) => {
            bad_request(format!("unknown shape(s): {list}"))
        }
        Err(reqforge_model::search::SearchError::Tantivy(err)) => {
            internal_error(format!("search error: {err}"))
        }
    }
}

fn ensure_search_scope_exists(
    scope: &reqforge_model::reports::Scope,
    world: &crate::app::World,
) -> Result<(), String> {
    use reqforge_model::mount::MountState;
    match scope {
        reqforge_model::reports::Scope::System => Ok(()),
        reqforge_model::reports::Scope::Project(slug) => {
            let present = world.mounts.iter().any(|m| match &m.state {
                MountState::Project(p) => p.config.slug == *slug,
                _ => false,
            });
            if present {
                Ok(())
            } else {
                Err(format!("project '{slug}' is not currently mounted"))
            }
        }
        reqforge_model::reports::Scope::Collection { slug, prefix } => {
            let project_present = world.mounts.iter().any(|m| match &m.state {
                MountState::Project(p) => p.config.slug == *slug,
                _ => false,
            });
            if !project_present {
                return Err(format!("project '{slug}' is not currently mounted"));
            }
            let found = world.mounts.iter().any(|m| {
                matches!(&m.state, MountState::Project(p)
                    if p.config.slug == *slug
                        && p.collections.iter().any(|c| c.config.prefix == *prefix))
            });
            if found {
                Ok(())
            } else {
                Err(format!(
                    "collection '{prefix}' not found in project '{slug}'"
                ))
            }
        }
    }
}

// ---- Phase 7b: matrix-link-view data endpoint ----

/// GET /api/matrix — backing feed for the TanStack-Virtual
/// matrix in the UI. Each axis has its own scope + tag +
/// review-state filters; `linkType` is required. Per-axis
/// 500-caps are enforced inside the compute module and, when
/// exceeded, propagate as `rowsTruncated` / `columnsTruncated`
/// flags on an otherwise-empty response so the UI can render a
/// blocking banner instead of a partial matrix.
pub async fn get_matrix(
    State(state): State<Arc<AppState>>,
    Query(query): Query<reqforge_model::matrix::MatrixQuery>,
) -> Response {
    let row_scope = match reqforge_model::reports::Scope::parse(query.row_scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(format!("row scope: {err}")),
    };
    let column_scope = match reqforge_model::reports::Scope::parse(query.column_scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(format!("column scope: {err}")),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    match reqforge_model::matrix::run(row_scope, column_scope, &query, &world) {
        Ok(resp) => Json(resp).into_response(),
        Err(reqforge_model::matrix::MatrixError::ProjectNotMounted(slug)) => {
            not_found(format!("project '{slug}' is not currently mounted"))
        }
        Err(reqforge_model::matrix::MatrixError::CollectionNotFound { slug, prefix }) => not_found(
            format!("collection '{prefix}' not found in project '{slug}'"),
        ),
        Err(reqforge_model::matrix::MatrixError::LinkTypeRequired) => {
            bad_request("linkType query parameter is required")
        }
        Err(reqforge_model::matrix::MatrixError::UnknownLinkType(name)) => {
            bad_request(format!("unknown link type '{name}'"))
        }
        Err(reqforge_model::matrix::MatrixError::UnknownReviewStates(list)) => {
            bad_request(format!("unknown review state(s): {list}"))
        }
    }
}

// ---- Phase 7a: graph-canvas data endpoint ----

/// GET /api/graph — backing feed for the React-Flow canvas in
/// the UI. Accepts scope + includeInactive + linkTypes + tags
/// query parameters and returns the capped node + edge sample
/// that drives the layout. The 500-node cap is enforced in the
/// compute layer; when exceeded the response's `truncated` flag
/// lets the UI banner the overflow.
pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Query(query): Query<reqforge_model::graph::GraphQuery>,
) -> Response {
    let scope = match reqforge_model::reports::Scope::parse(query.scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(err.to_string()),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    match reqforge_model::graph::run(scope, &query, &world) {
        Ok(resp) => Json(resp).into_response(),
        Err(reqforge_model::graph::GraphError::ProjectNotMounted(slug)) => {
            not_found(format!("project '{slug}' is not currently mounted"))
        }
        Err(reqforge_model::graph::GraphError::CollectionNotFound { slug, prefix }) => not_found(
            format!("collection '{prefix}' not found in project '{slug}'"),
        ),
    }
}

/// POST /api/projects/:slug/collections/:prefix/artifacts/blob/adopt —
/// adopt an existing on-disk binary (no sidecar) as a blob
/// artifact. The Phase 6a filesystem-orphans report surfaces the
/// candidates; the UI's Adopt wizard posts the relative path
/// straight back. We validate the path stays inside the target
/// collection's dir, extension is in the allowlist, and the file
/// already exists — then write the sidecar only (no copy).
pub async fn adopt_orphan_blob(
    State(state): State<Arc<AppState>>,
    Path((slug, prefix)): Path<(String, String)>,
    Json(req): Json<AdoptOrphanBlobRequest>,
) -> Response {
    if !is_safe_filename_stem(&req.name) {
        return bad_request(format!(
            "invalid artifact name '{}': must match [A-Za-z0-9._-]+",
            req.name,
        ));
    }
    // Normalise the declared path to forward slashes and refuse
    // absolute / parent-traversal forms — the adopt request
    // should always be a repo-relative file under the collection.
    let declared = req.binary_relative_path.replace('\\', "/");
    if declared.contains("..") || declared.starts_with('/') {
        return bad_request("binaryRelativePath must be a forward-slash repo-relative path");
    }

    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(collection) = find_collection(project, &prefix) else {
        return not_found(format!(
            "collection '{prefix}' not found in project '{slug}'"
        ));
    };
    if artifact_name_collides(collection, &req.name) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "artifact '{}' already exists in collection '{prefix}'",
                    req.name,
                ),
            }),
        )
            .into_response();
    }

    let binary_target = project.root.join(&declared);
    let canonical_collection = collection.dir_path.canonicalize().ok();
    let canonical_binary = binary_target.canonicalize().ok();
    let inside = match (&canonical_collection, &canonical_binary) {
        (Some(c), Some(b)) => b.starts_with(c),
        _ => binary_target
            .parent()
            .map(|p| p == collection.dir_path.as_path())
            .unwrap_or(false),
    };
    if !inside {
        return bad_request(
            "binaryRelativePath does not resolve to a file inside the target collection",
        );
    }
    if !binary_target.is_file() {
        return bad_request(format!(
            "binary file does not exist at '{}'",
            binary_target.display()
        ));
    }

    let extension = binary_target
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !reqforge_model::load::is_allowed_blob_extension(&extension) {
        return bad_request(format!(
            "extension '{extension}' is not in the blob allowlist",
        ));
    }
    let sidecar_target = reqforge_model::schema::sidecar::sidecar_path_for_blob(&binary_target);
    if sidecar_target.exists() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("sidecar already exists at '{}'", sidecar_target.display()),
            }),
        )
            .into_response();
    }

    let now = chrono::Utc::now();
    let new_uuid = Uuid::now_v7();
    let metadata = Artifact {
        schema_version: 1,
        uuid: new_uuid,
        title: req.title,
        shape: ArtifactShape::Blob,
        created_at: now,
        modified_at: now,
        links: Vec::new(),
        review_log: Vec::new(),
        description: req.description,
        expects_code_trace: None,
        active: req.active,
        derived: req.derived,
        tags: req.tags,
        outline_level: req.outline_level,
        legacy: None,
        blob_path: Some(declared),
        url: None,
        checked_at: None,
        check_status: None,
        overflow: std::collections::BTreeMap::new(),
    };
    let project_root = project.root.clone();
    let overrides = state.overrides();
    drop(world);

    let metadata_for_write = metadata.clone();
    let sidecar_target_cl = sidecar_target.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        reqforge_model::write::write_sidecar_only(
            &sidecar_target_cl,
            &project_root,
            &metadata_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("adopt write failed: {err}")),
        Err(join) => return internal_error(format!("adopt write task panicked: {join}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after adopt failed: {err}"));
    }
    respond_with_current(&state, new_uuid, StatusCode::CREATED).await
}

/// GET /api/reports/:kind/export/:ext — render a report in the
/// requested format (json / csv / html). Browsers get a proper
/// `Content-Disposition: attachment; filename=…` so a direct
/// click downloads the file with the locked-shape name
/// `reqforge-<kind>-<scope-slug>-<timestamp>.<ext>`. Cycles
/// declines CSV per the locked decision; unknown ext → 404.
pub async fn export_report(
    State(state): State<Arc<AppState>>,
    Path((kind_str, ext_str)): Path<(String, String)>,
    Query(query): Query<reqforge_model::reports::ReportQuery>,
) -> Response {
    let Some(kind) = reqforge_model::reports::ReportKind::from_kebab(&kind_str) else {
        return not_found(format!("unknown report kind '{kind_str}'"));
    };
    let Some(format) = reqforge_model::exports::ExportFormat::from_ext(&ext_str) else {
        return not_found(format!(
            "unknown export format '{ext_str}'; expected json / csv / html",
        ));
    };
    let scope = match reqforge_model::reports::Scope::parse(query.scope.as_deref()) {
        Ok(s) => s,
        Err(err) => return bad_request(err.to_string()),
    };
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let response = match reqforge_model::reports::run_report(kind, scope, &query, &world) {
        Ok(resp) => resp,
        Err(reqforge_model::reports::ReportError::ProjectNotMounted(slug)) => {
            return not_found(format!("project '{slug}' is not currently mounted"));
        }
        Err(reqforge_model::reports::ReportError::CollectionNotFound { slug, prefix }) => {
            return not_found(format!(
                "collection '{prefix}' not found in project '{slug}'"
            ));
        }
        Err(reqforge_model::reports::ReportError::InvalidDirection(dir)) => {
            return bad_request(format!(
                "invalid direction '{dir}'; expected 'dependents' or 'dependencies'"
            ));
        }
    };
    drop(world);

    // Reach into the response's scope for the filename builder.
    // Every variant carries a `ScopeDto`; we duplicate the small
    // dispatch rather than exposing a new public trait.
    let scope_dto = match &response {
        reqforge_model::reports::ReportResponse::UnresolvedLinks(r) => &r.scope,
        reqforge_model::reports::ReportResponse::LinkOrphans(r) => &r.scope,
        reqforge_model::reports::ReportResponse::Cycles(r) => &r.scope,
        reqforge_model::reports::ReportResponse::Conflicts(r) => &r.scope,
        reqforge_model::reports::ReportResponse::CoverageMatrix(r) => &r.scope,
        reqforge_model::reports::ReportResponse::ImpactAnalysis(r) => &r.scope,
        reqforge_model::reports::ReportResponse::ReviewStatus(r) => &r.scope,
        reqforge_model::reports::ReportResponse::FilesystemOrphans(r) => &r.scope,
        reqforge_model::reports::ReportResponse::CodeTraceability(r) => &r.scope,
    };
    let filename =
        reqforge_model::exports::filename::build(kind, scope_dto, format, chrono::Utc::now());

    let bytes = match format {
        reqforge_model::exports::ExportFormat::Json => match serde_json::to_vec_pretty(&response) {
            Ok(b) => b,
            Err(err) => return internal_error(format!("json encode: {err}")),
        },
        reqforge_model::exports::ExportFormat::Csv => {
            match reqforge_model::exports::render_csv(&response) {
                reqforge_model::exports::CsvOutcome::Bytes(b) => b,
                reqforge_model::exports::CsvOutcome::NotAcceptable {
                    reason,
                    alternatives,
                } => {
                    return (
                        StatusCode::NOT_ACCEPTABLE,
                        Json(serde_json::json!({
                            "error": reason,
                            "alternatives": alternatives,
                        })),
                    )
                        .into_response();
                }
            }
        }
        reqforge_model::exports::ExportFormat::Html => {
            reqforge_model::exports::render_html(&response, state.config().external_url.as_deref())
        }
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, format.mime())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(bytes))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Phase 10a: LLM adapter layer HTTP surface.

/// GET /api/llm/providers — priority-ordered list of the
/// configured providers with health + privacy-ack + env-var
/// availability. No secrets leak: the env-var *name* is
/// surfaced so operators can pinpoint the setting, but the
/// *value* never leaves the server.
pub async fn list_llm_providers(State(state): State<Arc<AppState>>) -> Response {
    let Some(runtime) = state.llm_runtime().await else {
        return Json(LlmProvidersResponse {
            providers: Vec::new(),
        })
        .into_response();
    };
    let providers = runtime
        .providers()
        .iter()
        .zip(runtime.adapters().iter())
        .enumerate()
        .map(|(index, (cfg, adapter))| {
            let endpoint = adapter.endpoint();
            let is_local = reqforge_model::llm::is_local_endpoint(endpoint);
            let requires_privacy_ack = runtime.privacy().requires_ack(index, endpoint);
            LlmProviderEntry {
                index,
                provider: cfg.provider.as_wire().to_owned(),
                model: adapter.model().to_owned(),
                endpoint: endpoint.to_owned(),
                is_local,
                requires_privacy_ack,
                api_key_available: adapter.api_key_available(),
                enabled: cfg.is_enabled(),
                health: runtime.health().state(index),
            }
        })
        .collect();
    Json(LlmProvidersResponse { providers }).into_response()
}

/// POST /api/llm/providers/{index}/retest — force-clear the
/// slot's health, fire a ping probe, return the post-probe
/// health snapshot. The probe call itself is privacy-gated
/// by the adapter (if the env var is missing, Auth error is
/// returned; the slot lands back in HardDisabled). Bypasses
/// the privacy-ack gate — retest is an explicit operator
/// action, not an implicit chain call.
pub async fn retest_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Response {
    let Some(runtime) = state.llm_runtime().await else {
        return not_found("llm runtime not configured");
    };
    if !runtime.valid_index(index) {
        return not_found(format!("llm provider index {index} out of range"));
    }
    let result = runtime.retest(index).await;
    let health = runtime.health().state(index);
    let body = match result {
        Ok(()) => LlmRetestResponse {
            ok: true,
            error: None,
            health,
        },
        Err(err) => LlmRetestResponse {
            ok: false,
            error: Some(err.to_string()),
            health,
        },
    };
    Json(body).into_response()
}

/// POST /api/llm/providers/{index}/acknowledge-privacy —
/// record the operator's acknowledgement that prompts to
/// this provider may leave the host. Idempotent; local
/// endpoints already bypass the gate so ack'ing them is a
/// no-op but still returns 200.
pub async fn acknowledge_llm_privacy(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Response {
    let Some(runtime) = state.llm_runtime().await else {
        return not_found("llm runtime not configured");
    };
    if !runtime.valid_index(index) {
        return not_found(format!("llm provider index {index} out of range"));
    }
    runtime.privacy().acknowledge(index);
    Json(LlmAcknowledgePrivacyResponse { acknowledged: true }).into_response()
}

/// POST /api/llm/prompt — debug endpoint that runs the full
/// fallback chain over the operator's prompt. Intended for
/// integration tests and manual troubleshooting; Phase 10b's
/// features hit the chain directly rather than going through
/// this endpoint.
pub async fn debug_llm_prompt(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LlmPromptRequest>,
) -> Response {
    let Some(runtime) = state.llm_runtime().await else {
        return not_found("llm runtime not configured");
    };
    if runtime.is_empty() {
        return not_found("no llm providers configured");
    }
    let prompt = reqforge_model::llm::PromptRequest {
        system: req.system,
        messages: vec![reqforge_model::llm::PromptMessage {
            role: reqforge_model::llm::PromptRole::User,
            content: req.prompt,
        }],
        max_tokens: req.max_tokens,
        temperature: req.temperature.unwrap_or(0.2),
        timeout_ms: None,
    };
    match runtime.run_prompt(&prompt).await {
        Ok((index, response)) => {
            let served_by = runtime
                .providers()
                .get(index)
                .map(|p| format!("{}/{}", p.provider.as_wire(), p.model))
                .unwrap_or_default();
            Json(LlmPromptResponse {
                served_by_index: index,
                served_by,
                text: response.text,
                usage: response.usage,
            })
            .into_response()
        }
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Phase 10b: rename-suggestions — single and bulk.

/// Walk the runtime's adapter list and return the indices that
/// would be skipped solely because they need a privacy ack (i.e.
/// not hard-disabled or in backoff). Used by the handlers to
/// distinguish the "every slot needs ack" case from "every slot
/// is dead" — the first is fixable by the operator.
fn providers_needing_ack(runtime: &reqforge_model::llm::LlmRuntime) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, adapter) in runtime.adapters().iter().enumerate() {
        if runtime.health().should_skip(i) {
            continue;
        }
        if runtime.privacy().requires_ack(i, adapter.endpoint()) {
            out.push(i);
        }
    }
    out
}

/// Whether any provider is eligible (not skipped for health,
/// not ack-pending). If none, the chain call is pointless.
fn has_eligible_provider(runtime: &reqforge_model::llm::LlmRuntime) -> bool {
    runtime.adapters().iter().enumerate().any(|(i, adapter)| {
        !runtime.health().should_skip(i) && !runtime.privacy().requires_ack(i, adapter.endpoint())
    })
}

/// POST /api/artifacts/:uuid/rename-suggestions — run the LLM
/// chain to propose up to three alternative filename stems.
/// Plain rename still works without LLM; this endpoint is
/// purely additive per LLM-renameWorkflow.
pub async fn rename_suggestions(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<Uuid>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(location) = world.index.get(&uuid) else {
        return not_found(format!("artifact {uuid} not found"));
    };
    let project = find_project(&world, &location.project_slug).unwrap();
    let collection = find_collection(project, &location.collection_prefix).unwrap();
    let artifact = collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == uuid)
        .unwrap();

    let Some(runtime) = state.llm_runtime().await else {
        return Json(RenameSuggestionsResponse::NoProviders).into_response();
    };
    if runtime.is_empty() {
        return Json(RenameSuggestionsResponse::NoProviders).into_response();
    }

    if !has_eligible_provider(&runtime) {
        let indices = providers_needing_ack(&runtime);
        if !indices.is_empty() {
            return Json(RenameSuggestionsResponse::PrivacyAckRequired { indices }).into_response();
        }
        // No eligible providers and no ack-pending ones either
        // → everything is hard-disabled / in backoff. Surface
        // that honestly.
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "no healthy LLM providers available".into(),
            }),
        )
            .into_response();
    }

    let siblings: Vec<String> = collection
        .artifacts
        .iter()
        .map(|a| a.name.clone())
        .collect();
    let input = reqforge_model::rename_suggest::PromptInput {
        collection_prefix: &collection.config.prefix,
        current_name: &artifact.name,
        current_title: &artifact.metadata.title,
        sibling_names: &siblings,
    };
    let prompt = reqforge_model::rename_suggest::build_prompt(&input);

    match runtime.run_prompt(&prompt).await {
        Ok((index, response)) => {
            match reqforge_model::rename_suggest::parse_suggestions(&response.text, &artifact.name)
            {
                Ok(suggestions) => {
                    let served_by = runtime
                        .providers()
                        .get(index)
                        .map(|p| format!("{}/{}", p.provider.as_wire(), p.model))
                        .unwrap_or_default();
                    Json(RenameSuggestionsResponse::Ok {
                        suggestions,
                        served_by_index: index,
                        served_by,
                    })
                    .into_response()
                }
                Err(err) => (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: err.to_string(),
                    }),
                )
                    .into_response(),
            }
        }
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        )
            .into_response(),
    }
}

/// POST /api/projects/:slug/rename-suggestions/bulk — fan out
/// rename-suggestions across many artifacts at parallelism 4
/// per LLM-postImportRenameSuggest. Each UUID gets its own
/// result entry so the post-import wizard can render a mixed
/// table (some suggestions, some errors) without the whole run
/// failing on one bad artifact.
pub async fn bulk_rename_suggestions(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<BulkRenameSuggestionsRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(_project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let Some(runtime) = state.llm_runtime().await else {
        return Json(BulkRenameSuggestionsResponse {
            results: Vec::new(),
        })
        .into_response();
    };
    if runtime.is_empty() {
        return Json(BulkRenameSuggestionsResponse {
            results: Vec::new(),
        })
        .into_response();
    }

    // Precompute per-UUID inputs. Any UUID that doesn't resolve
    // to an artifact within the named project gets a `NotFound`
    // entry — helps callers that submitted a stale wizard
    // session.
    struct TaskInput {
        uuid: Uuid,
        prompt: reqforge_model::llm::PromptRequest,
        current_name: String,
    }
    let mut inputs: Vec<TaskInput> = Vec::new();
    let mut not_found_entries: Vec<BulkRenameSuggestionEntry> = Vec::new();
    for uuid in &req.uuids {
        let Some(location) = world.index.get(uuid) else {
            not_found_entries.push(BulkRenameSuggestionEntry::NotFound { uuid: *uuid });
            continue;
        };
        if location.project_slug != slug {
            not_found_entries.push(BulkRenameSuggestionEntry::NotFound { uuid: *uuid });
            continue;
        }
        let project = find_project(&world, &location.project_slug).unwrap();
        let collection = find_collection(project, &location.collection_prefix).unwrap();
        let Some(artifact) = collection
            .artifacts
            .iter()
            .find(|a| a.metadata.uuid == *uuid)
        else {
            not_found_entries.push(BulkRenameSuggestionEntry::NotFound { uuid: *uuid });
            continue;
        };
        let siblings: Vec<String> = collection
            .artifacts
            .iter()
            .map(|a| a.name.clone())
            .collect();
        let input = reqforge_model::rename_suggest::PromptInput {
            collection_prefix: &collection.config.prefix,
            current_name: &artifact.name,
            current_title: &artifact.metadata.title,
            sibling_names: &siblings,
        };
        let prompt = reqforge_model::rename_suggest::build_prompt(&input);
        inputs.push(TaskInput {
            uuid: *uuid,
            prompt,
            current_name: artifact.name.clone(),
        });
    }
    // Release the world Arc before spawning — we've already
    // copied the data we need.
    drop(world);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let mut set = tokio::task::JoinSet::new();
    for task_input in inputs {
        let sem = semaphore.clone();
        let runtime = runtime.clone();
        set.spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore not closed");
            run_one_suggestion(
                &runtime,
                task_input.uuid,
                &task_input.prompt,
                &task_input.current_name,
            )
            .await
        });
    }

    let mut results = not_found_entries;
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(entry) => results.push(entry),
            Err(err) => {
                // A panicking task shouldn't fail the whole run
                // — capture it as a generic error on an unknown
                // UUID so the wizard surfaces something useful.
                results.push(BulkRenameSuggestionEntry::Error {
                    uuid: Uuid::nil(),
                    error: format!("task join failed: {err}"),
                });
            }
        }
    }
    Json(BulkRenameSuggestionsResponse { results }).into_response()
}

async fn run_one_suggestion(
    runtime: &reqforge_model::llm::LlmRuntime,
    uuid: Uuid,
    prompt: &reqforge_model::llm::PromptRequest,
    current_name: &str,
) -> BulkRenameSuggestionEntry {
    if !has_eligible_provider(runtime) {
        let indices = providers_needing_ack(runtime);
        if !indices.is_empty() {
            return BulkRenameSuggestionEntry::PrivacyAckRequired { uuid, indices };
        }
        return BulkRenameSuggestionEntry::Error {
            uuid,
            error: "no healthy LLM providers available".into(),
        };
    }
    match runtime.run_prompt(prompt).await {
        Ok((index, response)) => {
            match reqforge_model::rename_suggest::parse_suggestions(&response.text, current_name) {
                Ok(suggestions) => {
                    let served_by = runtime
                        .providers()
                        .get(index)
                        .map(|p| format!("{}/{}", p.provider.as_wire(), p.model))
                        .unwrap_or_default();
                    BulkRenameSuggestionEntry::Ok {
                        uuid,
                        suggestions,
                        served_by_index: index,
                        served_by,
                    }
                }
                Err(err) => BulkRenameSuggestionEntry::Error {
                    uuid,
                    error: err.to_string(),
                },
            }
        }
        Err(err) => BulkRenameSuggestionEntry::Error {
            uuid,
            error: err.to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// Phase 11a: bulk schema-migration endpoint.

/// POST /api/projects/{slug}/migrate-schema — walk every
/// ReqForge-authored file in the project, apply any registered
/// migrations, and atomic-rewrite the changed ones. Today every
/// registered chain is empty (schemaVersion=1 for every file
/// type), so the endpoint is usable as a dry-run smoke-test —
/// it reports every scanned file as up-to-date.
///
/// Body: `{ "force"?: bool }`. When `force=false` (default), a
/// dirty git worktree produces a 409; when `force=true`, the
/// run proceeds regardless. ReqForge never commits — the
/// operator does.
pub async fn migrate_project_schema(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<MigrateSchemaRequest>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    let overrides = state.overrides();
    let force = req.force;
    drop(world); // Release the world Arc before the blocking call.

    let migration_result = tokio::task::spawn_blocking(move || {
        reqforge_model::schema_migration::bulk::migrate_project(&project_root, overrides, force)
    })
    .await;
    match migration_result {
        Ok(Ok(result)) => {
            let body = MigrateSchemaResponse {
                project_slug: slug,
                result,
            };
            Json(body).into_response()
        }
        Ok(Err(reqforge_model::schema_migration::bulk::BulkMigrateError::DirtyWorktree)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error:
                    "project worktree has uncommitted changes; commit them or pass `force: true`"
                        .to_owned(),
            }),
        )
            .into_response(),
        Ok(Err(err)) => internal_error(err.to_string()),
        Err(join_err) => internal_error(format!("migration task panicked: {join_err}")),
    }
}

// ---------------------------------------------------------------------------
// Phase 11b: sample-content onboarding.

/// POST /api/projects/{slug}/sample-content — seed a just-
/// initialised project with the `UX-initSampleContent` demo set.
/// Refuses with 409 if the project already has any collection
/// (this is starter content, not a reset button); 404 for an
/// unknown slug. On success, writes every draft via the same
/// atomic-write path CRUD uses and refreshes discovery.
pub async fn create_sample_content(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    if !project.collections.is_empty() {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "project '{slug}' already has {} collection(s); sample content is starter content and won't overwrite existing work",
                    project.collections.len()
                ),
            }),
        )
            .into_response();
    }
    let project_root = project.root.clone();
    let artifacts_root = project_root.join(project.config.effective_artifacts_path());
    let overrides = state.overrides();
    drop(world);

    let drafts = reqforge_model::sample_content::generate(&slug);
    let collection_summaries: Vec<SampleContentCollectionSummary> = drafts
        .iter()
        .map(|c| SampleContentCollectionSummary {
            prefix: c.prefix.clone(),
            directory_name: c.directory_name.clone(),
            artifact_count: c.artifacts.len(),
            artifact_names: c.artifacts.iter().map(|a| a.name.clone()).collect(),
        })
        .collect();
    let total_artifacts: usize = drafts.iter().map(|c| c.artifacts.len()).sum();

    let project_root_for_write = project_root.clone();
    let artifacts_root_for_write = artifacts_root.clone();
    let write_result = tokio::task::spawn_blocking(move || {
        write_sample_content(
            &drafts,
            &artifacts_root_for_write,
            &project_root_for_write,
            overrides,
        )
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write sample content: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }
    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after sample-content failed: {err}"));
    }

    (
        StatusCode::CREATED,
        Json(SampleContentResponse {
            project_slug: slug,
            collections_created: collection_summaries.len(),
            artifacts_created: total_artifacts,
            collections: collection_summaries,
        }),
    )
        .into_response()
}

fn write_sample_content(
    drafts: &[reqforge_model::sample_content::CollectionDraft],
    artifacts_root: &std::path::Path,
    project_root: &std::path::Path,
    overrides: reqforge_model::write::OwnershipOverrides,
) -> Result<(), String> {
    use chrono::Utc;
    use reqforge_model::schema::{Artifact, ArtifactShape, CollectionConfig};

    std::fs::create_dir_all(artifacts_root).map_err(|e| format!("create artifacts root: {e}"))?;

    for draft in drafts {
        let collection_dir = artifacts_root.join(&draft.directory_name);
        std::fs::create_dir_all(&collection_dir)
            .map_err(|e| format!("create {}: {e}", collection_dir.display()))?;
        let config = CollectionConfig {
            schema_version: 1,
            prefix: draft.prefix.clone(),
            name: draft.name.clone(),
            description: draft.description.clone(),
            expects_code_trace: None,
            import_notes: None,
            overflow: Default::default(),
        };
        let config_path = collection_dir.join(".collection.json");
        let mut bytes = serde_json::to_vec_pretty(&config)
            .map_err(|e| format!("serialize collection config: {e}"))?;
        bytes.push(b'\n');
        atomic_write(&config_path, &bytes).map_err(|e| format!("write collection config: {e}"))?;
        reconcile_ownership(&config_path, project_root, overrides)
            .map_err(|e| format!("chown collection config: {e}"))?;

        for art in &draft.artifacts {
            let now = Utc::now();
            let metadata = Artifact {
                schema_version: 1,
                uuid: art.uuid,
                title: art.title.clone(),
                shape: ArtifactShape::Content,
                created_at: now,
                modified_at: now,
                links: art.links.clone(),
                review_log: Vec::new(),
                description: art.description.clone(),
                expects_code_trace: None,
                active: None,
                derived: None,
                tags: if art.tags.is_empty() {
                    None
                } else {
                    Some(art.tags.clone())
                },
                outline_level: None,
                legacy: None,
                blob_path: None,
                url: None,
                checked_at: None,
                check_status: None,
                overflow: Default::default(),
            };
            let target = collection_dir.join(format!("{}.md", art.name));
            write_artifact_file(&target, project_root, &metadata, &art.body, overrides)
                .map_err(|e| format!("write artifact {}: {e}", art.name))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 12a: LLM-assisted link suggestion handlers.

/// POST /api/projects/:slug/suggestions/links/analyze — run the
/// LLM-assisted link analysis, persist the result to the project's
/// pending sidecar, and return the new suggestion list.
pub async fn analyze_link_suggestions(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };

    // Provider gating, mirroring Phase 10b's rename surface.
    let Some(runtime) = state.llm_runtime().await else {
        return Json(crate::http::dto::AnalyzeSuggestionsResponse::NoProviders).into_response();
    };
    if runtime.is_empty() {
        return Json(crate::http::dto::AnalyzeSuggestionsResponse::NoProviders).into_response();
    }
    if !has_eligible_provider(&runtime) {
        let indices = providers_needing_ack(&runtime);
        if !indices.is_empty() {
            return Json(
                crate::http::dto::AnalyzeSuggestionsResponse::PrivacyAckRequired { indices },
            )
            .into_response();
        }
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "no healthy LLM providers available".into(),
            }),
        )
            .into_response();
    }

    let project_root = project.root.clone();

    // Load existing declined sidecar for the proposal filter.
    let declined = match reqforge_model::suggestions::declined::load(&project_root) {
        Ok(d) => d,
        Err(err) => {
            return internal_error(format!("read declined sidecar: {err}"));
        }
    };

    // Hold the world Arc snapshot across the LLM await — it's an
    // Arc clone, not a lock guard, so this is safe.
    let result = reqforge_model::suggestions::engine::propose_links(
        &runtime,
        project,
        &world.link_catalog,
        &declined,
    )
    .await;
    drop(world);

    match result {
        Ok(suggestions) => {
            if let Err(err) =
                reqforge_model::suggestions::pending::save(&project_root, &suggestions)
            {
                return internal_error(format!("write pending sidecar: {err}"));
            }
            let served_by_index = first_eligible_provider_index(&runtime).unwrap_or(0);
            let served_by = runtime
                .providers()
                .get(served_by_index)
                .map(|p| format!("{}/{}", p.provider.as_wire(), p.model))
                .unwrap_or_default();
            Json(crate::http::dto::AnalyzeSuggestionsResponse::Ok {
                suggestions,
                served_by_index,
                served_by,
            })
            .into_response()
        }
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("{err}"),
            }),
        )
            .into_response(),
    }
}

fn first_eligible_provider_index(runtime: &reqforge_model::llm::LlmRuntime) -> Option<usize> {
    (0..runtime.adapters().len()).find(|&i| !runtime.health().should_skip(i))
}

/// GET /api/projects/:slug/suggestions/links — list pending
/// suggestions for the project.
pub async fn list_pending_link_suggestions(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    drop(world);

    match reqforge_model::suggestions::pending::load(&project_root) {
        Ok(suggestions) => {
            Json(crate::http::dto::ListSuggestionsResponse { suggestions }).into_response()
        }
        Err(err) => internal_error(format!("read pending sidecar: {err}")),
    }
}

/// GET /api/projects/:slug/suggestions/links/declined — list
/// declined suggestions for the project.
pub async fn list_declined_link_suggestions(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    drop(world);

    match reqforge_model::suggestions::declined::load(&project_root) {
        Ok(declined) => {
            Json(crate::http::dto::ListDeclinedSuggestionsResponse { declined }).into_response()
        }
        Err(err) => internal_error(format!("read declined sidecar: {err}")),
    }
}

/// POST /api/projects/:slug/suggestions/links/:id/accept — apply
/// a pending suggestion as a real link, drop it from pending.
pub async fn accept_link_suggestion(
    State(state): State<Arc<AppState>>,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    drop(world);

    let Some(suggestion) = (match reqforge_model::suggestions::pending::remove(&project_root, id) {
        Ok(s) => s,
        Err(err) => {
            return internal_error(format!("remove from pending: {err}"));
        }
    }) else {
        return not_found(format!("pending suggestion {id} not found"));
    };

    apply_suggestion_as_link(&state, &suggestion).await
}

/// POST /api/projects/:slug/suggestions/links/:id/reject — move a
/// pending suggestion to the declined sidecar.
pub async fn reject_link_suggestion(
    State(state): State<Arc<AppState>>,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    drop(world);

    let Some(suggestion) = (match reqforge_model::suggestions::pending::remove(&project_root, id) {
        Ok(s) => s,
        Err(err) => return internal_error(format!("remove from pending: {err}")),
    }) else {
        return not_found(format!("pending suggestion {id} not found"));
    };

    let record = reqforge_model::suggestions::DeclineRecord {
        suggestion,
        declined_at: chrono::Utc::now(),
    };
    if let Err(err) = reqforge_model::suggestions::declined::append(&project_root, record) {
        return internal_error(format!("append to declined: {err}"));
    }
    StatusCode::NO_CONTENT.into_response()
}

/// POST /api/projects/:slug/suggestions/links/:id/reinstate —
/// take a declined suggestion, apply it as a real link, and
/// remove it from the declined sidecar.
pub async fn reinstate_link_suggestion(
    State(state): State<Arc<AppState>>,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    let Some(project) = find_project(&world, &slug) else {
        return not_found(format!("project '{slug}' not found"));
    };
    let project_root = project.root.clone();
    drop(world);

    let Some(record) = (match reqforge_model::suggestions::declined::remove(&project_root, id) {
        Ok(r) => r,
        Err(err) => return internal_error(format!("remove from declined: {err}")),
    }) else {
        return not_found(format!("declined suggestion {id} not found"));
    };

    apply_suggestion_as_link(&state, &record.suggestion).await
}

/// Shared accept / reinstate helper: builds a Link from a
/// Suggestion, appends it to the from artifact's links, validates
/// against the link catalog, and atomic-writes the updated
/// artifact.
async fn apply_suggestion_as_link(
    state: &AppState,
    suggestion: &reqforge_model::suggestions::Suggestion,
) -> Response {
    let Some(world) = state.snapshot().await else {
        return service_unavailable();
    };
    // Resolve the from artifact.
    let Some(from_loc) = world.index.get(&suggestion.from) else {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("source artifact {} no longer exists", suggestion.from),
            }),
        )
            .into_response();
    };
    let Some(from_project) = find_project(&world, &from_loc.project_slug) else {
        return internal_error("from-artifact project not loaded");
    };
    let Some(from_collection) = find_collection(from_project, &from_loc.collection_prefix) else {
        return internal_error("from-artifact collection not loaded");
    };
    let Some(from_artifact) = from_collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == suggestion.from)
    else {
        return internal_error("from-artifact not loaded");
    };
    // Resolve the to artifact (for LinkHint).
    let Some(to_loc) = world.index.get(&suggestion.to) else {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("target artifact {} no longer exists", suggestion.to),
            }),
        )
            .into_response();
    };
    let Some(to_project) = find_project(&world, &to_loc.project_slug) else {
        return internal_error("to-artifact project not loaded");
    };
    let Some(to_collection) = find_collection(to_project, &to_loc.collection_prefix) else {
        return internal_error("to-artifact collection not loaded");
    };
    let Some(to_artifact) = to_collection
        .artifacts
        .iter()
        .find(|a| a.metadata.uuid == suggestion.to)
    else {
        return internal_error("to-artifact not loaded");
    };

    // Build the new links list (existing + new) and validate.
    // `hint: None` lets validate_links repopulate from the UUID
    // index so we don't have to plumb the existing hints back.
    // The to_artifact lookup above is what proves the target
    // resolves; validate_links re-walks the index to produce the
    // canonical hint payload.
    let _ = to_artifact;
    let mut combined: Vec<reqforge_model::links::LinkWriteInput> = from_artifact
        .metadata
        .links
        .iter()
        .map(|l| reqforge_model::links::LinkWriteInput {
            target_uuid: l.target_uuid,
            type_name: l.type_name.clone(),
            hint: None,
        })
        .collect();
    combined.push(reqforge_model::links::LinkWriteInput {
        target_uuid: suggestion.to,
        type_name: suggestion.link_type.clone(),
        hint: None,
    });
    let validated = match reqforge_model::links::validate_links(
        suggestion.from,
        &combined,
        &world.link_catalog,
        &world.index,
    ) {
        Ok(v) => v,
        Err(err) => {
            return bad_request(format!("link validation failed: {err}"));
        }
    };

    let mut metadata = from_artifact.metadata.clone();
    metadata.links = validated.0;
    metadata.modified_at = chrono::Utc::now();

    let body = from_artifact.body.clone().unwrap_or_default();
    let source_path = from_artifact.source_path.clone();
    let project_root = from_project.root.clone();
    let overrides = state.overrides();
    let shape = metadata.shape;
    drop(world);

    let metadata_for_write = metadata.clone();
    let body_for_write = body.clone();
    let write_result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        match shape {
            ArtifactShape::Content => write_artifact_file(
                &source_path,
                &project_root,
                &metadata_for_write,
                &body_for_write,
                overrides,
            )
            .map_err(|err| format!("{err}")),
            ArtifactShape::Url | ArtifactShape::Blob => reqforge_model::write::write_sidecar_only(
                &source_path,
                &project_root,
                &metadata_for_write,
                overrides,
            )
            .map_err(|err| format!("{err}")),
        }
    })
    .await;
    match write_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return internal_error(format!("write failed: {err}")),
        Err(join_err) => return internal_error(format!("write task panicked: {join_err}")),
    }

    if let Err(err) = state.refresh().await {
        return internal_error(format!("refresh after write failed: {err}"));
    }
    StatusCode::NO_CONTENT.into_response()
}

// ---------------------------------------------------------------------------
// Phase 13: in-app LLM provider CRUD.

/// POST /api/llm/providers — append (or insert at `position`) a
/// new provider entry. Body: full ProviderCrudRequest.
pub async fn add_llm_provider(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ProviderCrudRequest>,
) -> Response {
    mutate_llm_array(&state, |arr| {
        let entry = build_provider_entry(&req, None)?;
        let position = req.position.unwrap_or(arr.len()).min(arr.len());
        arr.insert(position, entry);
        Ok(())
    })
    .await
}

/// PUT /api/llm/providers/{index} — replace the entry at index.
/// Merge semantics for `apiKey` and `enabled`: when those fields
/// are absent from the request body, the existing values are
/// preserved. The wire never returns the apiKey value, so the
/// frontend's Edit form can't re-supply it; preserving it on
/// merge means an Edit that doesn't touch the key keeps it intact.
pub async fn replace_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(req): Json<ProviderCrudRequest>,
) -> Response {
    mutate_llm_array(&state, move |arr| {
        if index >= arr.len() {
            return Err(ProviderCrudError::IndexOutOfRange { index });
        }
        let existing = arr[index].clone();
        let entry = build_provider_entry(&req, Some(&existing))?;
        arr[index] = entry;
        Ok(())
    })
    .await
}

/// DELETE /api/llm/providers/{index} — remove the entry at index.
pub async fn delete_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
) -> Response {
    mutate_llm_array(&state, move |arr| {
        if index >= arr.len() {
            return Err(ProviderCrudError::IndexOutOfRange { index });
        }
        arr.remove(index);
        Ok(())
    })
    .await
}

/// PATCH /api/llm/providers/{index} — toggle enabled and/or move
/// to a new position. Both fields optional; both unset is a no-op.
pub async fn patch_llm_provider(
    State(state): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(req): Json<ProviderPatchRequest>,
) -> Response {
    mutate_llm_array(&state, move |arr| {
        if index >= arr.len() {
            return Err(ProviderCrudError::IndexOutOfRange { index });
        }
        if let Some(enabled) = req.enabled
            && let Some(obj) = arr[index].as_object_mut()
        {
            obj.insert("enabled".into(), serde_json::Value::Bool(enabled));
        }
        if let Some(new_pos) = req.position {
            let new_pos = new_pos.min(arr.len() - 1);
            if new_pos != index {
                let entry = arr.remove(index);
                arr.insert(new_pos, entry);
            }
        }
        Ok(())
    })
    .await
}

/// Internal failure type for the CRUD helper. Never escapes —
/// each variant maps to a typed Response in `mutate_llm_array`.
#[derive(Debug)]
enum ProviderCrudError {
    NoSystemConfig,
    IndexOutOfRange { index: usize },
    Invalid(String),
    ReadOnly(std::path::PathBuf),
    Write(String),
    Refresh(String),
    PostMutateValidate(String),
}

/// Shared CRUD plumbing: snapshot the world, mutate the llm array
/// via `mutate`, validate via `parse_llm`, atomic-write the
/// updated SystemConfig, invalidate the LLM runtime, and refresh
/// discovery.
async fn mutate_llm_array<F>(state: &AppState, mutate: F) -> Response
where
    F: FnOnce(&mut Vec<serde_json::Value>) -> Result<(), ProviderCrudError>,
{
    let result = mutate_inner(state, mutate).await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(ProviderCrudError::NoSystemConfig) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "no system config is loaded — set REQFORGE_SYSTEM_CONFIG and restart"
                    .to_owned(),
            }),
        )
            .into_response(),
        Err(ProviderCrudError::IndexOutOfRange { index }) => not_found(format!(
            "provider index {index} out of range"
        )),
        Err(ProviderCrudError::Invalid(msg)) => bad_request(msg),
        Err(ProviderCrudError::PostMutateValidate(msg)) => bad_request(msg),
        Err(ProviderCrudError::ReadOnly(path)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "system config at {} is read-only — operators editing via /llm need a writable mount (drop the `:ro` flag)",
                    path.display()
                ),
            }),
        )
            .into_response(),
        Err(ProviderCrudError::Write(msg)) => internal_error(format!("write failed: {msg}")),
        Err(ProviderCrudError::Refresh(msg)) => internal_error(format!("refresh failed: {msg}")),
    }
}

async fn mutate_inner<F>(state: &AppState, mutate: F) -> Result<(), ProviderCrudError>
where
    F: FnOnce(&mut Vec<serde_json::Value>) -> Result<(), ProviderCrudError>,
{
    let world = state
        .snapshot()
        .await
        .ok_or_else(|| ProviderCrudError::Invalid("server not ready".into()))?;
    let (config, path) = match &world.system {
        reqforge_model::system::LoadedSystem::Named {
            config,
            source_path,
        } => ((**config).clone(), source_path.clone()),
        reqforge_model::system::LoadedSystem::Unnamed => {
            return Err(ProviderCrudError::NoSystemConfig);
        }
    };
    drop(world);

    let mut new_config = config;
    let mut llm_array: Vec<serde_json::Value> = match new_config.llm.take() {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(_) | None => Vec::new(),
    };
    mutate(&mut llm_array)?;

    let new_value = serde_json::Value::Array(llm_array);
    // Validate the new shape via parse_llm before writing — surface
    // schema problems as 400 rather than corrupting the file.
    reqforge_model::llm::parse_llm(Some(&new_value))
        .map_err(|err| ProviderCrudError::PostMutateValidate(err.to_string()))?;
    new_config.llm = Some(new_value);

    match reqforge_model::system::write_system_config(&path, &new_config) {
        Ok(()) => {}
        Err(err) if err.is_read_only() => {
            return Err(ProviderCrudError::ReadOnly(path));
        }
        Err(err) => return Err(ProviderCrudError::Write(err.to_string())),
    }

    state.invalidate_llm_runtime().await;
    state
        .refresh()
        .await
        .map_err(|e| ProviderCrudError::Refresh(e.to_string()))?;
    Ok(())
}

/// Build a provider JSON object from the request. When `existing`
/// is supplied (PUT path), missing-from-request fields fall back
/// to the corresponding value on the existing entry — so an Edit
/// that doesn't touch the apiKey preserves it, even though the
/// wire never round-trips the secret to the frontend.
fn build_provider_entry(
    req: &ProviderCrudRequest,
    existing: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ProviderCrudError> {
    if !matches!(
        req.provider.as_str(),
        "openai-compatible" | "anthropic" | "gemini"
    ) {
        return Err(ProviderCrudError::Invalid(format!(
            "unknown provider '{}' — expected one of openai-compatible, anthropic, gemini",
            req.provider
        )));
    }
    if req.model.trim().is_empty() {
        return Err(ProviderCrudError::Invalid(
            "model is required and must be non-empty".into(),
        ));
    }
    let existing_obj = existing.and_then(|v| v.as_object());
    let mut obj = serde_json::Map::new();
    obj.insert(
        "provider".into(),
        serde_json::Value::String(req.provider.clone()),
    );
    obj.insert("model".into(), serde_json::Value::String(req.model.clone()));
    if let Some(endpoint) = &req.endpoint {
        obj.insert(
            "endpoint".into(),
            serde_json::Value::String(endpoint.clone()),
        );
    } else if let Some(eo) = existing_obj
        && let Some(existing_endpoint) = eo.get("endpoint")
    {
        obj.insert("endpoint".into(), existing_endpoint.clone());
    }
    match &req.api_key {
        Some(val) if !val.is_empty() => {
            obj.insert("apiKey".into(), serde_json::Value::String(val.clone()));
        }
        Some(_) | None => {
            // Empty string and absent are both "no change" —
            // there's no UI path to deliberately scrub a key,
            // and a stray empty submission shouldn't silently
            // break auth. To clear the key, delete + re-add.
            if let Some(eo) = existing_obj
                && let Some(existing_key) = eo.get("apiKey")
            {
                obj.insert("apiKey".into(), existing_key.clone());
            }
        }
    }
    if let Some(enabled) = req.enabled {
        obj.insert("enabled".into(), serde_json::Value::Bool(enabled));
    } else if let Some(eo) = existing_obj
        && let Some(existing_enabled) = eo.get("enabled")
    {
        obj.insert("enabled".into(), existing_enabled.clone());
    }
    Ok(serde_json::Value::Object(obj))
}
