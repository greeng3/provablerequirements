//! Report catalog (Phase 6a).
//!
//! Every report kind follows the same shape: the handler takes a
//! [`Scope`] (System / Project / Collection), an
//! [`includeInactive`](ReportQuery) flag, and any kind-specific
//! options; the response is a serde-tagged [`ReportResponse`]
//! variant so the frontend dispatches on `kind` the same way it
//! dispatches on `shape` in Phase 5d's `ShapeDiff`. All eight
//! report kinds are implemented as of Phase 6a.4.

pub mod compute;
pub mod saved_config;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::ArtifactShape;
use crate::world::World;

/// Enumerates every report kind on the unified endpoint. Stable
/// kebab-case wire strings — the URL is `/api/reports/<kebab>`,
/// the saved-config store's filename is `<kebab>.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportKind {
    UnresolvedLinks,
    LinkOrphans,
    Cycles,
    Conflicts,
    CoverageMatrix,
    ImpactAnalysis,
    ReviewStatus,
    FilesystemOrphans,
    /// Phase 9b: per-artifact listing of code-side evidence
    /// plus orphan tags (REPORT-codeTraceability).
    CodeTraceability,
}

impl ReportKind {
    /// Stable identifier used in URLs + saved-config filenames.
    pub fn as_kebab(&self) -> &'static str {
        match self {
            Self::UnresolvedLinks => "unresolved-links",
            Self::LinkOrphans => "link-orphans",
            Self::Cycles => "cycles",
            Self::Conflicts => "conflicts",
            Self::CoverageMatrix => "coverage-matrix",
            Self::ImpactAnalysis => "impact-analysis",
            Self::ReviewStatus => "review-status",
            Self::FilesystemOrphans => "filesystem-orphans",
            Self::CodeTraceability => "code-traceability",
        }
    }

    pub fn from_kebab(s: &str) -> Option<Self> {
        match s {
            "unresolved-links" => Some(Self::UnresolvedLinks),
            "link-orphans" => Some(Self::LinkOrphans),
            "cycles" => Some(Self::Cycles),
            "conflicts" => Some(Self::Conflicts),
            "coverage-matrix" => Some(Self::CoverageMatrix),
            "impact-analysis" => Some(Self::ImpactAnalysis),
            "review-status" => Some(Self::ReviewStatus),
            "filesystem-orphans" => Some(Self::FilesystemOrphans),
            "code-traceability" => Some(Self::CodeTraceability),
            _ => None,
        }
    }
}

/// Scope restricts a report's input set to a slice of the System.
/// Parsed from the `?scope=` query param:
///
/// - `system` (or absent) → [`Scope::System`]
/// - `project:<slug>` → [`Scope::Project`]
/// - `collection:<slug>/<prefix>` → [`Scope::Collection`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    System,
    Project(String),
    Collection { slug: String, prefix: String },
}

impl Scope {
    pub fn parse(raw: Option<&str>) -> Result<Self, ScopeParseError> {
        let Some(raw) = raw else {
            return Ok(Self::System);
        };
        if raw == "system" || raw.is_empty() {
            return Ok(Self::System);
        }
        if let Some(slug) = raw.strip_prefix("project:") {
            if slug.is_empty() {
                return Err(ScopeParseError::Empty("project"));
            }
            return Ok(Self::Project(slug.to_owned()));
        }
        if let Some(rest) = raw.strip_prefix("collection:") {
            let (slug, prefix) = rest.split_once('/').ok_or(ScopeParseError::MissingPrefix)?;
            if slug.is_empty() {
                return Err(ScopeParseError::Empty("project"));
            }
            if prefix.is_empty() {
                return Err(ScopeParseError::Empty("collection"));
            }
            return Ok(Self::Collection {
                slug: slug.to_owned(),
                prefix: prefix.to_owned(),
            });
        }
        Err(ScopeParseError::UnknownForm(raw.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScopeParseError {
    #[error(
        "unknown scope form '{0}'; expected system | project:<slug> | collection:<slug>/<prefix>"
    )]
    UnknownForm(String),
    #[error("collection scope missing '/<prefix>'")]
    MissingPrefix,
    #[error("{0} name cannot be empty")]
    Empty(&'static str),
}

/// Query parameters that every report handler accepts. Kind-
/// specific options ride on their own optional fields so the
/// shared dispatcher doesn't need bespoke per-kind query types;
/// compute functions only touch the fields they care about.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportQuery {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub include_inactive: Option<bool>,

    // ---- coverage-matrix options ----
    /// Comma-separated list of link-type names to treat as
    /// "covering". When absent, the compute function applies the
    /// REPORT-coverageMatrix default set.
    #[serde(default)]
    pub covering_link_types: Option<String>,

    // ---- impact-analysis options ----
    /// Seed artifact UUID. Required by the impact-analysis
    /// report; ignored by every other kind.
    #[serde(default)]
    pub seed: Option<String>,
    /// `"dependents"` (default) walks incoming edges from the
    /// seed; `"dependencies"` walks outgoing. Any other value is
    /// rejected at the compute layer.
    #[serde(default)]
    pub direction: Option<String>,
}

impl ReportQuery {
    pub fn include_inactive(&self) -> bool {
        self.include_inactive.unwrap_or(false)
    }

    /// Parse the optional `coveringLinkTypes=a,b,c` param into a
    /// trimmed de-duplicated `Vec<String>`, dropping empty
    /// entries. Returns `None` when the param is absent — the
    /// compute function then applies the default set.
    pub fn covering_link_type_list(&self) -> Option<Vec<String>> {
        let raw = self.covering_link_types.as_deref()?;
        let mut out: Vec<String> = Vec::new();
        for part in raw.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !out.iter().any(|e| e == trimmed) {
                out.push(trimmed.to_owned());
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }
}

/// Tagged response union — one variant per report kind. Frontend
/// narrows on `kind`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReportResponse {
    UnresolvedLinks(UnresolvedLinksReport),
    LinkOrphans(LinkOrphansReport),
    Cycles(CyclesReport),
    Conflicts(ConflictsReport),
    CoverageMatrix(CoverageMatrixReport),
    ImpactAnalysis(ImpactAnalysisReport),
    ReviewStatus(ReviewStatusReport),
    FilesystemOrphans(FilesystemOrphansReport),
    CodeTraceability(CodeTraceabilityReport),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedLinksReport {
    pub scope: ScopeDto,
    pub total_unresolved: usize,
    pub entries: Vec<UnresolvedLinkEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedLinkEntry {
    pub source_uuid: Uuid,
    pub source_project_slug: String,
    pub source_collection_prefix: String,
    pub source_artifact_name: String,
    pub source_title: String,
    pub source_shape: ArtifactShape,
    pub link_type: String,
    pub target_uuid: Uuid,
    pub target_hint_project_slug: String,
    pub target_hint_collection_prefix: String,
    pub target_hint_artifact_name: String,
    /// Stable reason string — `"mount-missing"` when the hint's
    /// project isn't currently mounted, `"target-missing"` when
    /// the mount is present but the UUID isn't in the index.
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkOrphansReport {
    pub scope: ScopeDto,
    pub total_orphans: usize,
    pub entries: Vec<LinkOrphanEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkOrphanEntry {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: ArtifactShape,
    pub active: bool,
    pub derived: bool,
}

/// One cycle, as discovered in one acyclic link type's directed
/// edge set. `path` is an ordered loop: `path[0]` reappears at
/// `path[len]` conceptually. The UI renders each row as a chain
/// with the link-type name between the arrows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleEntry {
    pub link_type: String,
    pub nodes: Vec<CycleNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleNode {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: ArtifactShape,
    pub active: bool,
}

/// Top-level cycles report. `linkTypesChecked` mirrors the list
/// of acyclic link types the walk considered, so the UI can
/// explain why a type isn't represented (it isn't marked
/// acyclic).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CyclesReport {
    pub scope: ScopeDto,
    pub link_types_checked: Vec<String>,
    pub total_cycles: usize,
    /// `true` when any link type hit the per-type cycle cap
    /// (see [`compute::MAX_CYCLES_PER_LINK_TYPE`]). The UI
    /// surfaces a banner asking operators to clean up the first
    /// batch before looking for more.
    pub truncated: bool,
    pub cycles: Vec<CycleEntry>,
}

/// One pair in the conflicts report. Pairs are deduplicated by
/// sorting UUIDs, so a link from A→B + B→A yields a single row.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPair {
    pub first: CycleNode,
    pub second: CycleNode,
    /// `true` when the backing links exist in both directions
    /// (A→B and B→A). Either direction alone still produces a
    /// pair — the conflicts-with type is undirected, so either
    /// side is sufficient to surface the pair, but having both
    /// is a modelling nicety worth flagging.
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictsReport {
    pub scope: ScopeDto,
    pub total_pairs: usize,
    pub pairs: Vec<ConflictPair>,
}

/// Coverage-matrix report — one row per in-scope parent artifact
/// with the set of covering children and a `has_gap` flag that
/// fires when no children cover the parent via any of the
/// configured covering link types.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageMatrixReport {
    pub scope: ScopeDto,
    /// The effective covering-link-type set used for this run.
    /// Echoed back so the UI can render the header without
    /// re-reading its own saved config.
    pub covering_link_types: Vec<String>,
    /// Covering link types the request asked for but that aren't
    /// in the effective catalog. Rendered as an amber warning on
    /// the UI so operators notice a typo in their saved config.
    pub unknown_requested_types: Vec<String>,
    pub total_parents: usize,
    pub gap_count: usize,
    pub parents: Vec<CoverageParentEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageParentEntry {
    pub parent: CycleNode,
    pub has_gap: bool,
    pub covering_children: Vec<CoverageChildEntry>,
    /// Phase 9b: code-side evidence collected from the
    /// scanner (`scan::run_scan`). Each entry is one in-code
    /// tag whose verb is in the effective covering-link-type
    /// set. Empty when no tags cover the parent. Existing
    /// clients that ignore the field keep working; the
    /// coverage-matrix page renders a `(+N code)` badge when
    /// the list is non-empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covering_code_evidence: Vec<CoverageCodeEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageChildEntry {
    pub child: CycleNode,
    /// Which covering link type this child uses to cover the
    /// parent. Multiple-entry display groups by type.
    pub link_type: String,
}

/// Phase 9b code-side evidence row. The verb is the Phase 9a
/// canonical form (`"Satisfies"`, `"Verifies"`, ...).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageCodeEntry {
    pub file: PathBuf,
    pub line: usize,
    pub verb: String,
}

/// Impact-analysis report — reachability from a seed artifact
/// along traceability links. Default direction is `"dependents"`
/// (who transitively points AT the seed); flip to
/// `"dependencies"` via `?direction=dependencies` to walk the
/// seed's outgoing edges instead.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactAnalysisReport {
    pub scope: ScopeDto,
    /// The seed artifact itself. `None` when the requested seed
    /// UUID isn't resolvable — the UI then prompts the operator
    /// to pick a different seed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<CycleNode>,
    pub direction: String,
    pub total_impacted: usize,
    pub impacted: Vec<ImpactedArtifact>,
    /// Populated when no seed was supplied on the request. The
    /// UI renders this as an inline instruction rather than a
    /// generic empty-state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing_seed_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedArtifact {
    pub node: CycleNode,
    /// BFS distance from the seed. Depth 1 is a direct neighbour;
    /// the UI typically sorts the table by this so nearer nodes
    /// come first.
    pub depth: usize,
    /// Link types that arrived at this node across the traversal;
    /// deduped and sorted for a stable display.
    pub link_types: Vec<String>,
}

/// Buckets used by the review-status report. Mirrors the four
/// states in `ReviewState` with camelCase wire names so the
/// frontend narrows uniformly.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatusCounts {
    pub approved: usize,
    pub rejected: usize,
    pub re_requested: usize,
    pub never_reviewed: usize,
}

impl ReviewStatusCounts {
    pub fn total(&self) -> usize {
        self.approved + self.rejected + self.re_requested + self.never_reviewed
    }
}

/// Review-status report. `totals` is the overall count across
/// scope; `byProject`, `byCollection`, and `byShape` are three
/// facets the UI can switch between without re-fetching.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatusReport {
    pub scope: ScopeDto,
    pub totals: ReviewStatusCounts,
    pub by_project: Vec<ReviewStatusByProject>,
    pub by_collection: Vec<ReviewStatusByCollection>,
    pub by_shape: ReviewStatusByShape,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatusByProject {
    pub project_slug: String,
    pub counts: ReviewStatusCounts,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatusByCollection {
    pub project_slug: String,
    pub collection_prefix: String,
    pub counts: ReviewStatusCounts,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStatusByShape {
    pub content: ReviewStatusCounts,
    pub blob: ReviewStatusCounts,
    pub url: ReviewStatusCounts,
}

/// Filesystem-orphans report — mismatches in the on-disk blob /
/// sidecar pairing. Surfaces two categories per
/// REPORT-orphans:
///
/// - `missing_sidecar`: binary files in blob-holding
///   collection dirs that have no companion `.reqforge.json`;
///   the UI prompts an Adopt-as-artifact wizard.
/// - `missing_binary`: sidecars whose `blobPath` points at a
///   file that isn't on disk; the UI prompts a
///   restore-or-delete decision.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemOrphansReport {
    pub scope: ScopeDto,
    pub missing_sidecar: Vec<OrphanBinary>,
    pub missing_binary: Vec<OrphanSidecar>,
}

/// A binary file on disk that lacks a companion
/// `.reqforge.json`. The UI's "Adopt as artifact" wizard posts
/// back to `/api/projects/:slug/collections/:prefix/artifacts/
/// blob/adopt` with the `binary_relative_path` verbatim.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanBinary {
    pub project_slug: String,
    pub collection_prefix: String,
    pub filename: String,
    pub binary_relative_path: String,
    pub byte_size: u64,
    pub media_type: &'static str,
}

/// A sidecar whose declared `blobPath` doesn't resolve on disk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanSidecar {
    pub project_slug: String,
    pub collection_prefix: String,
    pub sidecar_filename: String,
    pub declared_blob_path: String,
}

/// Phase 9b code-traceability report (REPORT-codeTraceability).
/// Per-artifact listing of in-code tag locations grouped by
/// verb, plus separate lists of orphan tags and uncovered
/// artifacts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTraceabilityReport {
    pub scope: ScopeDto,
    pub total_artifacts: usize,
    pub uncovered_count: usize,
    pub orphan_tag_count: usize,
    pub entries: Vec<CodeTraceabilityEntry>,
    pub orphan_tags: Vec<CodeTraceabilityOrphan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTraceabilityEntry {
    pub artifact: CycleNode,
    pub expects_code_trace: bool,
    /// `true` when `expectsCodeTrace` resolves true and
    /// `locationsByVerb` is empty. False otherwise — even for
    /// artifacts that don't expect a trace, so the UI can
    /// filter on this flag without caring about the policy.
    pub has_gap: bool,
    /// Canonical-verb → locations. Verbs match the Phase 9a
    /// canonical forms (`"Satisfies"`, `"Verifies"`, etc.).
    pub locations_by_verb: std::collections::BTreeMap<String, Vec<CodeTraceabilityLocation>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTraceabilityLocation {
    pub file: PathBuf,
    pub line: usize,
}

/// An in-code tag whose `(prefix, name)` pair didn't resolve
/// to any mounted artifact. Typically a rename left the tag
/// pointing at an identifier that no longer exists.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTraceabilityOrphan {
    pub file: PathBuf,
    pub line: usize,
    pub verb: String,
    pub raw_id: String,
}

/// Scope as it's echoed back in the report body — lets the UI
/// render "for collection REQ" headers without re-parsing the
/// query string. One serde tag instead of a pair of `kind` +
/// `value` fields keeps the wire shape tight.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScopeDto {
    System,
    Project { slug: String },
    Collection { slug: String, prefix: String },
}

impl From<&Scope> for ScopeDto {
    fn from(scope: &Scope) -> Self {
        match scope {
            Scope::System => Self::System,
            Scope::Project(slug) => Self::Project { slug: slug.clone() },
            Scope::Collection { slug, prefix } => Self::Collection {
                slug: slug.clone(),
                prefix: prefix.clone(),
            },
        }
    }
}

/// Dispatch a ReportKind against a World snapshot. Returns the
/// tagged response; the handler wraps this in 200 JSON.
pub fn run_report(
    kind: ReportKind,
    scope: Scope,
    query: &ReportQuery,
    world: &World,
) -> Result<ReportResponse, ReportError> {
    match kind {
        ReportKind::UnresolvedLinks => Ok(ReportResponse::UnresolvedLinks(
            compute::unresolved_links(&scope, query.include_inactive(), world)?,
        )),
        ReportKind::LinkOrphans => Ok(ReportResponse::LinkOrphans(compute::link_orphans(
            &scope,
            query.include_inactive(),
            world,
        )?)),
        ReportKind::Cycles => Ok(ReportResponse::Cycles(compute::cycles(
            &scope,
            query.include_inactive(),
            world,
        )?)),
        ReportKind::Conflicts => Ok(ReportResponse::Conflicts(compute::conflicts(
            &scope,
            query.include_inactive(),
            world,
        )?)),
        ReportKind::CoverageMatrix => Ok(ReportResponse::CoverageMatrix(compute::coverage_matrix(
            &scope,
            query.include_inactive(),
            query,
            world,
        )?)),
        ReportKind::ImpactAnalysis => Ok(ReportResponse::ImpactAnalysis(compute::impact_analysis(
            &scope,
            query.include_inactive(),
            query,
            world,
        )?)),
        ReportKind::ReviewStatus => Ok(ReportResponse::ReviewStatus(compute::review_status(
            &scope,
            query.include_inactive(),
            world,
        )?)),
        ReportKind::FilesystemOrphans => Ok(ReportResponse::FilesystemOrphans(
            compute::filesystem_orphans(&scope, world)?,
        )),
        ReportKind::CodeTraceability => Ok(ReportResponse::CodeTraceability(
            compute::code_traceability(&scope, query.include_inactive(), world)?,
        )),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("project '{0}' is not currently mounted")]
    ProjectNotMounted(String),
    #[error("collection '{prefix}' not found in project '{slug}'")]
    CollectionNotFound { slug: String, prefix: String },
    #[error("invalid impact-analysis direction '{0}'; expected 'dependents' or 'dependencies'")]
    InvalidDirection(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_parses_system_variants() {
        assert_eq!(Scope::parse(None).unwrap(), Scope::System);
        assert_eq!(Scope::parse(Some("system")).unwrap(), Scope::System);
        assert_eq!(Scope::parse(Some("")).unwrap(), Scope::System);
    }

    #[test]
    fn scope_parses_project_and_collection_forms() {
        assert_eq!(
            Scope::parse(Some("project:sample")).unwrap(),
            Scope::Project("sample".to_owned())
        );
        assert_eq!(
            Scope::parse(Some("collection:sample/REQ")).unwrap(),
            Scope::Collection {
                slug: "sample".to_owned(),
                prefix: "REQ".to_owned()
            }
        );
    }

    #[test]
    fn scope_rejects_malformed_forms() {
        assert!(Scope::parse(Some("foo")).is_err());
        assert!(Scope::parse(Some("project:")).is_err());
        assert!(Scope::parse(Some("collection:sample")).is_err());
        assert!(Scope::parse(Some("collection:/REQ")).is_err());
    }

    #[test]
    fn report_kind_round_trips_through_kebab() {
        for kind in [
            ReportKind::UnresolvedLinks,
            ReportKind::LinkOrphans,
            ReportKind::Cycles,
            ReportKind::Conflicts,
            ReportKind::CoverageMatrix,
            ReportKind::ImpactAnalysis,
            ReportKind::ReviewStatus,
            ReportKind::FilesystemOrphans,
        ] {
            let s = kind.as_kebab();
            assert_eq!(ReportKind::from_kebab(s), Some(kind));
        }
        assert_eq!(ReportKind::from_kebab("unknown"), None);
    }

    #[test]
    fn report_query_defaults_include_inactive_to_false() {
        let q = ReportQuery::default();
        assert!(!q.include_inactive());
    }
}
