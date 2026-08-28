//! Browse-by-type view (Phase 7d).
//!
//! A single `GET /api/browse` endpoint groups every in-scope
//! artifact by its Collection `prefix`, returning one pane per
//! distinct prefix with artifacts sorted by title. Complements
//! the Phase 7c full-text search with a scannable overview of
//! the corpus — per `UX-browseByType`.
//!
//! The spec's "artifact type" is Collection: per
//! `ART-collectionGrouping`, a Collection is "a named, typed
//! grouping of related artifacts". Two mounted projects whose
//! Collections share a prefix land in one pane even when the
//! Collection `name` differs; any name inconsistency surfaces
//! in the pane's `nameVariants` field so the UI can warn
//! rather than silently picking a winner.

pub mod compute;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::matrix::ReviewStateTag;
use crate::reports::{Scope, ScopeDto};
use crate::world::World;

/// Query-string parameters accepted by `GET /api/browse`. The
/// vocabulary mirrors the Phase 7c search handler so operators
/// don't relearn the filter knobs across views.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseQuery {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub review_state: Option<String>,
    #[serde(default)]
    pub include_inactive: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseArtifact {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: crate::schema::ArtifactShape,
    pub active: bool,
    pub review_state: ReviewStateTag,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePane {
    /// Collection prefix — the pane's identity.
    pub prefix: String,
    /// Display label. Lexicographically first distinct Collection
    /// `name` observed for this prefix, so the choice is stable
    /// across mount-order churn.
    pub name: String,
    /// Alternate Collection names seen for this prefix across
    /// projects. Present only when ≥ 1 additional name exists
    /// beyond the chosen display label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_variants: Option<Vec<String>>,
    pub total_artifacts: usize,
    pub artifacts: Vec<BrowseArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResponse {
    pub scope: ScopeDto,
    pub total_panes: usize,
    pub total_artifacts: usize,
    pub panes: Vec<BrowsePane>,
}

#[derive(Debug, thiserror::Error)]
pub enum BrowseError {
    #[error("project '{0}' is not currently mounted")]
    ProjectNotMounted(String),
    #[error("collection '{prefix}' not found in project '{slug}'")]
    CollectionNotFound { slug: String, prefix: String },
    #[error("unknown review state(s): {0}")]
    UnknownReviewStates(String),
}

pub fn run(
    scope: Scope,
    query: &BrowseQuery,
    world: &World,
) -> Result<BrowseResponse, BrowseError> {
    compute::build_browse(scope, query, world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_defaults_are_permissive() {
        let q = BrowseQuery::default();
        assert!(q.scope.is_none());
        assert!(q.tags.is_none());
        assert!(q.review_state.is_none());
        assert!(q.include_inactive.is_none());
    }
}
