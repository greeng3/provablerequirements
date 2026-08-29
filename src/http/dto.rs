//! JSON response types for the read-only HTTP API.
//!
//! These are distinct from the on-disk schema types in `reqforge_model::schema`
//! so the wire format can evolve independently of the file format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use reqforge_model::links::{LinkType, LinkTypeSource};
use reqforge_model::load::{LoadedArtifact, LoadedCollection, LoadedProject};
use reqforge_model::mount::{MountInfo, MountState};
use reqforge_model::reviews::{DerivedReviewState, OpenTodo, ReviewState, derive_review_state};
use reqforge_model::schema::{ArtifactShape, Link, LinkHint, ReviewLogEntry};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessResponse {
    pub ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub collection_count: usize,
    pub artifact_count: usize,
}

impl From<&LoadedProject> for ProjectSummary {
    fn from(p: &LoadedProject) -> Self {
        Self {
            slug: p.config.slug.clone(),
            name: p.config.name.clone(),
            description: p.config.description.clone(),
            collection_count: p.collections.len(),
            artifact_count: p.collections.iter().map(|c| c.artifacts.len()).sum(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub artifacts_path: String,
    pub collections: Vec<CollectionSummary>,
    /// Phase 11a: files inside this project whose `schemaVersion`
    /// is newer than this build of ReqForge knows how to read.
    /// The frontend shows a banner prompting an upgrade. Omitted
    /// from the wire when empty so v1-clean projects don't see a
    /// new field.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schema_diagnostics: Vec<SchemaDiagnostic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiagnostic {
    pub path: String,
    pub file_type: reqforge_model::schema_migration::FileType,
    pub found_version: u32,
    pub current_version: u32,
}

impl From<&LoadedProject> for ProjectDetail {
    fn from(p: &LoadedProject) -> Self {
        let schema_diagnostics = p
            .diagnostics
            .iter()
            .filter_map(|d| match d {
                reqforge_model::load::LoadDiagnostic::SchemaTooNew {
                    path,
                    file_type,
                    found_version,
                    current_version,
                } => Some(SchemaDiagnostic {
                    path: path.display().to_string(),
                    file_type: *file_type,
                    found_version: *found_version,
                    current_version: *current_version,
                }),
                _ => None,
            })
            .collect();
        Self {
            slug: p.config.slug.clone(),
            name: p.config.name.clone(),
            description: p.config.description.clone(),
            artifacts_path: p.config.effective_artifacts_path().to_owned(),
            collections: p.collections.iter().map(CollectionSummary::from).collect(),
            schema_diagnostics,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub prefix: String,
    pub name: String,
    pub description: Option<String>,
    pub artifact_count: usize,
    pub expects_code_trace: bool,
}

impl From<&LoadedCollection> for CollectionSummary {
    fn from(c: &LoadedCollection) -> Self {
        Self {
            prefix: c.config.prefix.clone(),
            name: c.config.name.clone(),
            description: c.config.description.clone(),
            artifact_count: c.artifacts.len(),
            expects_code_trace: c.config.effective_expects_code_trace(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactListing {
    pub name: String,
    pub uuid: Uuid,
    pub title: String,
    pub shape: ArtifactShape,
    pub active: bool,
    /// At-a-glance review state so collection views can badge each
    /// row without hitting the per-artifact endpoint.
    pub review_state: ReviewStateTag,
}

impl From<&LoadedArtifact> for ArtifactListing {
    fn from(a: &LoadedArtifact) -> Self {
        Self {
            name: a.name.clone(),
            uuid: a.metadata.uuid,
            title: a.metadata.title.clone(),
            shape: a.metadata.shape,
            active: a.metadata.is_active(),
            review_state: derive_review_state(&a.metadata.review_log).state.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDetail {
    pub name: String,
    pub project_slug: String,
    pub collection_prefix: String,
    pub uuid: Uuid,
    pub title: String,
    pub shape: ArtifactShape,
    pub description: Option<String>,
    pub active: bool,
    pub derived: bool,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub tags: Vec<String>,
    /// Outline level (e.g. "1.2.3") — surfaced when the underlying
    /// artifact declared one (`ART-outlineLevelField`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<String>,
    /// Server-resolved view of each outgoing link. The client
    /// renders straight from this — resolution, type metadata, and
    /// target summary are all precomputed.
    pub links: Vec<LinkView>,
    pub review_log: Vec<ReviewLogEntry>,
    /// Server-computed derived view of the review log. Preserves the
    /// raw `review_log` alongside for rendering timestamps,
    /// explanations, and added-TODO history.
    pub review_state: DerivedReviewStateDto,
    pub body: Option<String>,
    /// Present when `shape == "blob"` — stat + hash facts about the
    /// binary peer plus the client-side URLs for downloading and
    /// thumbnailing it. `None` for content and URL shapes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<BlobDetailDto>,
    /// Present when `shape == "url"` — the URL string itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Present when `shape == "url"` and the URL has been checked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<DateTime<Utc>>,
    /// Present when `shape == "url"` and the URL has been checked;
    /// one of `"ok"`, `"not-found"`, `"server-error"`, etc. See
    /// `src/urls/check.rs` (Phase 5b) for the stable set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check_status: Option<String>,
}

/// Wire shape for a blob artifact's binary-peer facts.
/// `downloadUrl` is always relative (no leading host) so the same
/// DTO works for dev proxy and containerised serving.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobDetailDto {
    pub byte_size: u64,
    pub content_hash: String,
    pub media_type: String,
    pub download_url: String,
    pub thumbnail_url: String,
}

/// Phase 6a.4: adopt an on-disk binary that lacks a sidecar as a
/// blob artifact. The caller supplies `binaryRelativePath`
/// verbatim from the filesystem-orphans report; the handler
/// validates it lives inside the target collection's dir and
/// passes the extension allowlist.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptOrphanBlobRequest {
    pub name: String,
    pub title: String,
    pub binary_relative_path: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub derived: Option<bool>,
    #[serde(default)]
    pub outline_level: Option<String>,
}

/// Request body for `POST /api/projects/:slug/collections/:prefix/artifacts/url`.
/// A URL artifact has no binary peer; all the metadata comes through
/// JSON up front.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUrlArtifactRequest {
    pub name: String,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub derived: Option<bool>,
    #[serde(default)]
    pub outline_level: Option<String>,
}

/// Response for `POST /api/artifacts/:uuid/check-url`. The wire
/// shape matches the `checkStatus` string set from
/// `UX-urlArtifactChecking`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUrlResponse {
    pub uuid: Uuid,
    pub checked_at: DateTime<Utc>,
    pub check_status: String,
}

/// Optional body for the bulk-check endpoint. When `uuids` is
/// present, only those URL artifacts are checked; when absent,
/// every URL artifact in the targeted collection is checked.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BulkCheckUrlsRequest {
    #[serde(default)]
    pub uuids: Option<Vec<Uuid>>,
}

/// Response for the bulk-check endpoint: one per-artifact result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkCheckUrlsResponse {
    pub checked: Vec<CheckUrlResponse>,
}

impl ArtifactDetail {
    /// Build a detail view, resolving every outgoing link against
    /// the supplied World snapshot (UUID index + effective catalog).
    pub fn from_loaded(
        artifact: &LoadedArtifact,
        project_slug: &str,
        collection_prefix: &str,
        world: &crate::app::World,
    ) -> Self {
        let meta = &artifact.metadata;
        let links = meta
            .links
            .iter()
            .map(|link| resolve_link(link, world))
            .collect();
        let review_state = derive_review_state(&meta.review_log).into();
        let blob = artifact.blob.as_ref().map(|facts| BlobDetailDto {
            byte_size: facts.byte_size,
            content_hash: facts.content_hash.clone(),
            media_type: facts.media_type.to_owned(),
            download_url: format!("/api/artifacts/{}/blob", meta.uuid),
            thumbnail_url: format!("/api/artifacts/{}/thumbnail", meta.uuid),
        });
        let (url, checked_at, check_status) = match meta.shape {
            ArtifactShape::Url => (meta.url.clone(), meta.checked_at, meta.check_status.clone()),
            _ => (None, None, None),
        };
        Self {
            name: artifact.name.clone(),
            project_slug: project_slug.to_owned(),
            collection_prefix: collection_prefix.to_owned(),
            uuid: meta.uuid,
            title: meta.title.clone(),
            shape: meta.shape,
            description: meta.description.clone(),
            active: meta.is_active(),
            derived: meta.is_derived(),
            created_at: meta.created_at,
            modified_at: meta.modified_at,
            tags: meta.tags.clone().unwrap_or_default(),
            outline_level: meta.outline_level.clone(),
            links,
            review_log: meta.review_log.clone(),
            review_state,
            body: artifact.body.clone(),
            blob,
            url,
            checked_at,
            check_status,
        }
    }
}

/// Compute a `LinkView` for one raw `Link` against the World. The
/// order of the checks matters — we prefer reporting
/// "unknown type" over "unresolved" so the operator sees the more
/// actionable problem first (typically an authoring-level fix vs.
/// a mount-level fix).
fn resolve_link(link: &Link, world: &crate::app::World) -> LinkView {
    let type_metadata = world
        .link_catalog
        .iter()
        .find(|t| t.name == link.type_name)
        .map(LinkTypeDto::from);
    let target_location = world.index.get(&link.target_uuid);

    let resolution = match (type_metadata.is_some(), target_location.is_some()) {
        (false, _) => LinkResolution::UnknownType,
        (true, true) => LinkResolution::Resolved,
        (true, false) => LinkResolution::Unresolved,
    };

    let target_summary = target_location.and_then(|loc| target_summary_for(loc, world));

    LinkView {
        target_uuid: link.target_uuid,
        type_name: link.type_name.clone(),
        hint: link.hint.clone(),
        resolution,
        type_metadata,
        target_summary,
    }
}

fn target_summary_for(
    location: &reqforge_model::index::ArtifactLocation,
    world: &crate::app::World,
) -> Option<LinkTargetSummary> {
    for mount in &world.mounts {
        let MountState::Project(project) = &mount.state else {
            continue;
        };
        if project.config.slug != location.project_slug {
            continue;
        }
        for collection in &project.collections {
            if collection.config.prefix != location.collection_prefix {
                continue;
            }
            for artifact in &collection.artifacts {
                if artifact.name == location.artifact_name {
                    return Some(LinkTargetSummary {
                        project_slug: location.project_slug.clone(),
                        collection_prefix: location.collection_prefix.clone(),
                        artifact_name: location.artifact_name.clone(),
                        title: artifact.metadata.title.clone(),
                    });
                }
            }
        }
    }
    None
}

/// A mount entry surfaced to the UI. The `state` discriminator
/// lets the System Home view render the four validity badges
/// described in `DEPLOY-mountValidityStates` without needing to
/// reason about the Rust enum shape.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountEntry {
    /// The mounted directory's absolute path.
    pub path: String,
    /// Directory basename — a stable label the UI can show.
    pub dir_name: String,
    /// One of "project", "needsInit", "noGit", "loadFailed".
    pub state: MountStateTag,
    /// Present when state == "project".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectSummary>,
    /// Present when state == "loadFailed"; human-readable reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MountStateTag {
    Project,
    NeedsInit,
    NoGit,
    LoadFailed,
}

impl From<&MountInfo> for MountEntry {
    fn from(m: &MountInfo) -> Self {
        let path = m.path.display().to_string();
        let dir_name = m
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        match &m.state {
            MountState::Project(p) => Self {
                path,
                dir_name,
                state: MountStateTag::Project,
                project: Some(ProjectSummary::from(p)),
                error: None,
            },
            MountState::NeedsInit => Self {
                path,
                dir_name,
                state: MountStateTag::NeedsInit,
                project: None,
                error: None,
            },
            MountState::NoGit => Self {
                path,
                dir_name,
                state: MountStateTag::NoGit,
                project: None,
                error: None,
            },
            MountState::LoadFailed(err) => Self {
                path,
                dir_name,
                state: MountStateTag::LoadFailed,
                project: None,
                error: Some(err.to_string()),
            },
        }
    }
}

/// Request body for POST /api/projects/:slug/collections.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    /// On-disk directory name under `<project>/<artifactsPath>/`.
    pub dir_name: String,
    pub prefix: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub expects_code_trace: Option<bool>,
}

/// Request body for POST /api/mounts/:dirName/init — converts a
/// NeedsInit mount into a fully-loaded Project by writing a
/// `reqforge.json` at the mount root.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitProjectRequest {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Optional override for the Collections-root path. When unset,
    /// ReqForge uses the default `artifacts/` (per
    /// FORMAT-collectionsRootPath).
    #[serde(default)]
    pub artifacts_path: Option<String>,
}

/// Request body for PATCH /api/artifacts/:uuid — rename an artifact
/// within its current collection. Cross-collection moves are a
/// separate operation (not yet wired in Phase 2).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameArtifactRequest {
    pub name: String,
}

/// Request body for POST /api/projects/:slug/collections/:prefix/artifacts.
/// Creates a new content-hosted artifact.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArtifactRequest {
    /// Filename stem (e.g. "REQ-hello" for REQ-hello.md). Must be
    /// unique within the collection; validated server-side.
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub derived: Option<bool>,
    #[serde(default)]
    pub outline_level: Option<String>,
}

/// Outgoing-link summary for a single source artifact that links
/// into a target artifact, used by GET incoming-links so the UI
/// can warn users before deleting something that's still linked.
/// Wire shape for the server-computed review state of an artifact.
/// Phase 4a; mirrors the `LinkView` pattern (server computes once per
/// read so the client doesn't have to fold the log itself).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedReviewStateDto {
    pub state: ReviewStateTag,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_approval_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reviewer: Option<String>,
    pub blocking_todos: Vec<OpenTodoDto>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStateTag {
    NeverReviewed,
    Approved,
    Rejected,
    ReRequested,
}

impl From<ReviewState> for ReviewStateTag {
    fn from(s: ReviewState) -> Self {
        match s {
            ReviewState::NeverReviewed => Self::NeverReviewed,
            ReviewState::Approved => Self::Approved,
            ReviewState::Rejected => Self::Rejected,
            ReviewState::ReRequested => Self::ReRequested,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTodoDto {
    pub id: String,
    pub text: String,
    pub added_at: DateTime<Utc>,
    pub added_by: String,
}

impl From<&OpenTodo> for OpenTodoDto {
    fn from(t: &OpenTodo) -> Self {
        Self {
            id: t.id.clone(),
            text: t.text.clone(),
            added_at: t.added_at,
            added_by: t.added_by.clone(),
        }
    }
}

impl From<DerivedReviewState> for DerivedReviewStateDto {
    fn from(d: DerivedReviewState) -> Self {
        Self {
            state: d.state.into(),
            last_approval_at: d.last_approval_at,
            last_event_at: d.last_event_at,
            last_reviewer: d.last_reviewer,
            blocking_todos: d.blocking_todos.iter().map(OpenTodoDto::from).collect(),
        }
    }
}

/// Wire shape for a link-type catalog entry. `source` lets the UI
/// distinguish built-ins from System-declared extras so the picker
/// can group / label them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTypeDto {
    pub name: String,
    pub inverse_name: String,
    pub directed: bool,
    pub acyclic: bool,
    pub source: LinkTypeSourceTag,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkTypeSourceTag {
    Builtin,
    System,
}

impl From<&LinkType> for LinkTypeDto {
    fn from(t: &LinkType) -> Self {
        Self {
            name: t.name.to_owned(),
            inverse_name: t.inverse_name.to_owned(),
            directed: t.directed,
            acyclic: t.acyclic,
            source: match t.source {
                LinkTypeSource::Builtin => LinkTypeSourceTag::Builtin,
                LinkTypeSource::System => LinkTypeSourceTag::System,
            },
        }
    }
}

/// Server-resolved view of an artifact's outgoing link.
///
/// Replaces the raw `Link` in `ArtifactDetail.links` so the client
/// never has to resolve target UUIDs or link-type names itself
/// (per `TRACE-unresolvedLinks`). The original on-disk `Link`
/// serde type is unchanged.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    pub target_uuid: Uuid,
    #[serde(rename = "type")]
    pub type_name: String,
    pub hint: LinkHint,
    pub resolution: LinkResolution,
    /// Populated when the link's type is in the effective catalog;
    /// absent for `"unknownType"` links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_metadata: Option<LinkTypeDto>,
    /// Populated when the target UUID resolves to a loaded
    /// artifact; absent for `"unresolved"` links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_summary: Option<LinkTargetSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LinkResolution {
    Resolved,
    Unresolved,
    UnknownType,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkTargetSummary {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
}

/// Wire shape for `POST /api/artifacts/:uuid/reviews`. Clients
/// submit one action per call; the server validates against the
/// artifact's current derived state and appends a single log
/// entry (per the Phase 4 action/entry 1:1 contract).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReviewRequest {
    pub reviewer: String,
    #[serde(flatten)]
    pub action: ReviewActionBody,
    #[serde(default)]
    pub explanation: Option<String>,
}

/// Internally tagged action union matching the five review actions
/// from `UX-reviewActions`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum ReviewActionBody {
    Approve,
    #[serde(rename = "reject-with-todo")]
    RejectWithTodo {
        todo: AddedTodoRequest,
    },
    #[serde(rename = "add-todo")]
    AddTodo {
        todo: AddedTodoRequest,
    },
    #[serde(rename = "resolve-todo")]
    ResolveTodo {
        #[serde(rename = "todoId")]
        todo_id: String,
    },
    #[serde(rename = "re-request-review")]
    ReRequestReview,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedTodoRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub text: String,
}

impl ReviewActionBody {
    /// Convert the wire-side tagged enum into the validator-facing
    /// input shape.
    pub fn into_action_input(self) -> reqforge_model::reviews::ReviewAction {
        use reqforge_model::reviews::{AddedTodoInput, ReviewAction};
        match self {
            Self::Approve => ReviewAction::Approve,
            Self::RejectWithTodo { todo } => ReviewAction::RejectWithTodo(AddedTodoInput {
                id: todo.id,
                text: todo.text,
            }),
            Self::AddTodo { todo } => ReviewAction::AddTodo(AddedTodoInput {
                id: todo.id,
                text: todo.text,
            }),
            Self::ResolveTodo { todo_id } => ReviewAction::ResolveTodo { id: todo_id },
            Self::ReRequestReview => ReviewAction::ReRequestReview,
        }
    }
}

/// Response for `GET /api/reviewers`. See `REVIEW-reviewerIdentity`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewerIdentityResponse {
    /// The mount's `.git/config` `[user] name = …` when
    /// `?projectSlug=` matched a loaded mount; otherwise the
    /// workspace-level git config's name, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_default: Option<String>,
    pub persisted: Vec<String>,
    pub session: Vec<String>,
}

/// Response for `GET /api/artifacts/:uuid/reviews/last-approval-snapshot`.
/// Carries the body and parsed metadata captured at the time of the
/// most recent approval, which the UI diffs against the current
/// artifact state per `UX-reviewPane`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastApprovalSnapshotResponse {
    pub approved_at: DateTime<Utc>,
    pub body: String,
    pub metadata: serde_json::Value,
}

/// Response for `GET /api/reviews/queue`. Two sections per
/// `UX-reviewQueue`: artifacts awaiting review (no approval
/// covering the current body), and artifacts with one or more open
/// blocking TODOs from a prior rejection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueResponse {
    pub awaiting_review: Vec<ReviewQueueEntry>,
    pub blocking_todos: Vec<ReviewQueueEntry>,
}

/// One item in a review-queue section.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewQueueEntry {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: ArtifactShape,
    pub state: ReviewStateTag,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<DateTime<Utc>>,
    pub modified_at: DateTime<Utc>,
    pub blocking_todo_count: usize,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reviewer: Option<String>,
}

/// One hit from `GET /api/artifacts/search`. Kept small and
/// project-aware so the picker can show which project each match
/// belongs to without a second lookup.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSearchResult {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingLinkEntry {
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub source_uuid: Uuid,
    pub link_type: String,
}

/// Request body for PUT /api/artifacts/:uuid. Every field is
/// optional; missing fields are left unchanged. `shape`, `uuid`,
/// `createdAt`, and `reviewLog` are immutable via this endpoint —
/// they're set at creation time or by Phase 4 review actions.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifactRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub active: Option<Option<bool>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub derived: Option<Option<bool>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub outline_level: Option<Option<String>>,
    /// Full-array replacement of the artifact's outgoing links.
    /// Absent = unchanged; empty array = clear all links. Each
    /// entry's `hint` is optional — the server auto-populates from
    /// the UUID index when the target is currently mounted.
    #[serde(default)]
    pub links: Option<Vec<LinkWriteRequest>>,
    /// New URL for a URL artifact, per `ART-urlArtifact`. Rejected
    /// on content and blob shapes. Absent = leave the URL alone;
    /// an empty string is not allowed. Changing the URL implicitly
    /// clears `checkedAt` / `checkStatus` so the stale check
    /// doesn't mislead the reviewer — the operator re-runs the
    /// check action.
    #[serde(default)]
    pub url: Option<String>,
}

/// One entry in `UpdateArtifactRequest.links`. Matches the on-disk
/// `Link` shape except `hint` is optional on input (the server
/// prefers the authoritative hint from the UUID index).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkWriteRequest {
    pub target_uuid: Uuid,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub hint: Option<LinkHint>,
}

impl From<LinkWriteRequest> for reqforge_model::links::LinkWriteInput {
    fn from(req: LinkWriteRequest) -> Self {
        Self {
            target_uuid: req.target_uuid,
            type_name: req.type_name,
            hint: req.hint,
        }
    }
}

/// Helper: accept `null` vs omission vs `value` as three distinct
/// states for an Option<Option<T>> field. Present-and-null clears
/// the field; present-with-value sets it; absent leaves it alone.
fn deserialize_optional_nullable<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Phase 5d: response shape for `GET /api/artifacts/:uuid/history`.
/// `fallbackReason` is `None` on a happy-path response; `Some` when
/// the server fell through because the mount isn't a git repo / has
/// a shallow clone / the history endpoint itself errored. The
/// frontend shows the standalone diff route unconditionally and
/// surfaces the reason in a banner when history is unavailable.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactHistoryResponse {
    pub commits: Vec<reqforge_model::git_history::CommitInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

/// Query args shared by `/artifact?at=<oid>` and
/// `/artifact/blob?at=<oid>`. When `at` is absent the handlers
/// behave exactly as Phase 5b did.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AtCommitQuery {
    #[serde(default)]
    pub at: Option<String>,
}

/// Query args for `/artifact/:uuid/diff`. `from` is required (the
/// base); `to` may be omitted and defaults to the working-tree
/// current state.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffQuery {
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
}

/// Response for `GET /api/artifacts/:uuid/diff`. Wraps the
/// shape-tagged payload with the two resolved-commit labels the UI
/// prints alongside the diff (`"working tree"` for `current`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDiffResponse {
    pub shape: ArtifactShape,
    pub from_label: String,
    pub to_label: String,
    pub diff: reqforge_model::diff::ShapeDiff,
    /// Present when the server had to fall back to the approval
    /// snapshot because history couldn't resolve a commit — the
    /// banner wording is defined in the Phase 5 locked decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase 10a + 13: LLM adapter layer DTOs.
//
// All responses are secret-safe: `apiKeyAvailable` reports whether
// a key is configured for the slot without revealing the value, and
// no endpoint returns the configured raw credentials.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderEntry {
    pub index: usize,
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub is_local: bool,
    pub requires_privacy_ack: bool,
    pub api_key_available: bool,
    /// Mirrors `ProviderConfig.is_enabled()` so the UI can show
    /// the toggle state without re-reading the System config.
    pub enabled: bool,
    pub health: reqforge_model::llm::HealthState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProvidersResponse {
    pub providers: Vec<LlmProviderEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmRetestResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub health: reqforge_model::llm::HealthState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmAcknowledgePrivacyResponse {
    pub acknowledged: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPromptRequest {
    pub prompt: String,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub temperature: Option<f32>,
}

fn default_max_tokens() -> u32 {
    512
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPromptResponse {
    pub served_by_index: usize,
    pub served_by: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<reqforge_model::llm::PromptUsage>,
}

// ---------------------------------------------------------------------------
// Phase 10b: LLM-assisted rename suggestion DTOs.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameSuggestionsRequest {
    /// Reserved for future hints — empty body is fine today.
    #[serde(default)]
    pub _hint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum RenameSuggestionsResponse {
    /// The happy path — chain ran, produced suggestions.
    Ok {
        suggestions: Vec<reqforge_model::rename_suggest::Suggestion>,
        served_by_index: usize,
        served_by: String,
    },
    /// One or more eligible providers still require a privacy
    /// acknowledgement. The UI uses this to route the operator
    /// to the right /llm/providers/{index}/acknowledge-privacy
    /// action.
    PrivacyAckRequired { indices: Vec<usize> },
    /// No LLM providers are configured — rename still works as
    /// plain rename; the UI just hides the Suggest button.
    NoProviders,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkRenameSuggestionsRequest {
    pub uuids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkRenameSuggestionsResponse {
    /// Per-UUID outcome. Dropping to a per-entry
    /// success/failure arm lets the wizard show a mixed table
    /// (some suggestions, some errors) rather than failing the
    /// whole bulk run because one artifact's chain failed.
    pub results: Vec<BulkRenameSuggestionEntry>,
}

#[derive(Debug, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum BulkRenameSuggestionEntry {
    Ok {
        uuid: Uuid,
        suggestions: Vec<reqforge_model::rename_suggest::Suggestion>,
        served_by_index: usize,
        served_by: String,
    },
    Error {
        uuid: Uuid,
        error: String,
    },
    PrivacyAckRequired {
        uuid: Uuid,
        indices: Vec<usize>,
    },
    NotFound {
        uuid: Uuid,
    },
}

// ---------------------------------------------------------------------------
// Phase 11a: schema-migration DTOs.

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MigrateSchemaRequest {
    /// Bypass the uncommitted-changes pre-flight. Defaults to
    /// `false` — the operator has to opt in.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateSchemaResponse {
    pub project_slug: String,
    pub result: reqforge_model::schema_migration::bulk::BulkMigrateResult,
}

// ---------------------------------------------------------------------------
// Phase 11b: sample-content onboarding.

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleContentResponse {
    pub project_slug: String,
    pub collections_created: usize,
    pub artifacts_created: usize,
    /// Per-collection summary so the UI can render a compact
    /// "we wrote these" panel after the run.
    pub collections: Vec<SampleContentCollectionSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleContentCollectionSummary {
    pub prefix: String,
    pub directory_name: String,
    pub artifact_count: usize,
    /// Filename stems of the artifacts written under this
    /// collection, in insertion order. Handy for the UI to jump
    /// to the first artifact after seeding.
    pub artifact_names: Vec<String>,
}

// ---------------------------------------------------------------------------
// Phase 11c: System state view.

/// Read-only view over the loaded System config + mount summary.
/// The System Home view reads this to decide whether to surface
/// the `UX-systemConfigBanner` prompt.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStateResponse {
    /// Whether a `SystemConfig` was successfully loaded this
    /// process lifetime. `false` for the unnamed-System case
    /// (no `REQFORGE_SYSTEM_CONFIG` env var, missing file, etc).
    pub loaded: bool,
    /// `Some(name)` when `loaded == true` — the System's human
    /// display name. Omitted from the wire when absent so the UI
    /// doesn't have to branch on `name === null` vs missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Count of mounts currently in the `Project` state. Feeds
    /// the banner condition (`projectCount >= 2 && !loaded`).
    pub project_count: usize,
}

// ---------------------------------------------------------------------------
// Phase 12a: LLM-assisted link suggestion.

/// Response shape for `POST /api/projects/{slug}/suggestions/links/analyze`.
/// Mirrors the Phase 10b RenameSuggestionsResponse arms so the frontend
/// can branch on `kind` for the privacy-ack and no-providers paths.
#[derive(Debug, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum AnalyzeSuggestionsResponse {
    Ok {
        suggestions: Vec<reqforge_model::suggestions::Suggestion>,
        served_by_index: usize,
        served_by: String,
    },
    /// One or more eligible providers still require a privacy
    /// acknowledgement. The UI routes the operator to
    /// /llm/providers/{index}/acknowledge-privacy.
    PrivacyAckRequired { indices: Vec<usize> },
    /// No LLM providers are configured. The "Analyze" button on
    /// the project page already hides itself in this case; the
    /// arm exists so a stale frontend gets a clean error.
    NoProviders,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSuggestionsResponse {
    pub suggestions: Vec<reqforge_model::suggestions::Suggestion>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDeclinedSuggestionsResponse {
    pub declined: Vec<reqforge_model::suggestions::DeclineRecord>,
}

// ---------------------------------------------------------------------------
// Phase 13: in-app LLM provider CRUD.

/// Body for `POST /api/llm/providers` (append a new entry) and
/// `PUT /api/llm/providers/{index}` (replace the entry at index).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCrudRequest {
    /// Wire form: "openai-compatible" / "anthropic" / "gemini".
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// `POST` only: insert at this position rather than the end.
    /// `PUT` ignores this field — use `PATCH` to reorder.
    #[serde(default)]
    pub position: Option<usize>,
}

/// Body for `PATCH /api/llm/providers/{index}`. Either field may
/// be `None`; both `None` is a no-op.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPatchRequest {
    /// Toggle the provider's `enabled` flag.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Move the entry to a new index in the array.
    #[serde(default)]
    pub position: Option<usize>,
}
