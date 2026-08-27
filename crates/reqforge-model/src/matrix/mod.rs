//! Matrix-link-view data (Phase 7b).
//!
//! A single `GET /api/matrix` endpoint backs the React/TanStack-
//! Virtual matrix in the UI. The handler takes two independent
//! axis scopes + per-axis filters + a required link-type name
//! and returns `{rows, columns, edges}` already narrowed to the
//! chosen link type. Both axes enforce their own 500-cap; when
//! either axis overflows, the response carries the `truncated`
//! flags + totals and the UI renders a blocking banner instead
//! of a partial matrix.
//!
//! Cell-level authoring (click to add or remove a link) reuses
//! the existing `PUT /api/artifacts/:uuid` write path; Phase 7b
//! introduces no new write endpoint.

pub mod compute;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::graph::GraphLinkType;
use crate::reports::{Scope, ScopeDto};
use crate::world::World;

/// Soft cap on per-axis visible artifacts per
/// `UX-linkCreationMatrix`. TanStack Virtual stays responsive
/// at 500×500 on mid-range hardware; beyond that, the UI blocks
/// render and nags for additional filters.
pub const MATRIX_AXIS_CAP: usize = 500;

/// Query-string parameters accepted by `GET /api/matrix`. Each
/// axis has its own scope + tag + review-state filters; a shared
/// `include_inactive` flag applies to both axes.
///
/// `link_type` is required — a matrix without a link type is
/// meaningless (the cells have nothing to encode). The handler
/// returns `400` if the parameter is missing or names a type
/// absent from the effective catalog.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixQuery {
    #[serde(default)]
    pub row_scope: Option<String>,
    #[serde(default)]
    pub column_scope: Option<String>,
    #[serde(default)]
    pub link_type: Option<String>,
    #[serde(default)]
    pub include_inactive: Option<bool>,

    /// CSV of tag strings. When absent, the tag filter is a
    /// no-op for the axis; when present, an artifact must carry
    /// at least one of the listed tags to appear.
    #[serde(default)]
    pub row_tags: Option<String>,
    #[serde(default)]
    pub column_tags: Option<String>,

    /// CSV of review-state names — `approved`, `rejected`,
    /// `re-requested`, `never-reviewed`. When absent, the
    /// review-state filter is a no-op for the axis.
    #[serde(default)]
    pub row_review_states: Option<String>,
    #[serde(default)]
    pub column_review_states: Option<String>,
}

impl MatrixQuery {
    pub fn include_inactive(&self) -> bool {
        self.include_inactive.unwrap_or(false)
    }

    pub fn row_tag_list(&self) -> Option<Vec<String>> {
        csv_field(self.row_tags.as_deref())
    }

    pub fn column_tag_list(&self) -> Option<Vec<String>> {
        csv_field(self.column_tags.as_deref())
    }

    pub fn row_review_state_list(&self) -> Option<Vec<String>> {
        csv_field(self.row_review_states.as_deref())
    }

    pub fn column_review_state_list(&self) -> Option<Vec<String>> {
        csv_field(self.column_review_states.as_deref())
    }
}

fn csv_field(raw: Option<&str>) -> Option<Vec<String>> {
    let raw = raw?;
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

/// Per-axis node shape. Shares the Phase 7a `GraphNodeDto`
/// fields plus a `reviewState` tag so the frontend can colour-
/// code rows/columns without a second lookup.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixNode {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: crate::schema::ArtifactShape,
    pub active: bool,
    pub derived: bool,
    pub tags: Vec<String>,
    pub review_state: ReviewStateTag,
}

/// Serialised review-state tag. Kept camelCase-hyphen-free to
/// ride cleanly through the CSV filter parsing on the query
/// side — the filter accepts kebab-case strings and the wire
/// format matches them exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewStateTag {
    NeverReviewed,
    Approved,
    Rejected,
    ReRequested,
}

impl ReviewStateTag {
    pub fn from_derived(state: &crate::reviews::DerivedReviewState) -> Self {
        match state.state {
            crate::reviews::ReviewState::NeverReviewed => ReviewStateTag::NeverReviewed,
            crate::reviews::ReviewState::Approved => ReviewStateTag::Approved,
            crate::reviews::ReviewState::Rejected => ReviewStateTag::Rejected,
            crate::reviews::ReviewState::ReRequested => ReviewStateTag::ReRequested,
        }
    }

    /// Parse the kebab-case filter string from the query
    /// parameter. Returns `None` for unknown tags so the handler
    /// can surface a typed 400 listing the invalid entries.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "never-reviewed" => Some(Self::NeverReviewed),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "re-requested" => Some(Self::ReRequested),
            _ => None,
        }
    }
}

/// One cell's edge encoding. The matrix only carries edges of
/// the chosen link type, already oriented row → column, so the
/// frontend can render cells with a simple `(row, column) ∈ edges`
/// check.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixEdge {
    pub row_uuid: Uuid,
    pub column_uuid: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixResponse {
    pub row_scope: ScopeDto,
    pub column_scope: ScopeDto,
    pub link_type: GraphLinkType,

    /// Total row-axis artifacts in scope before the 500-cap
    /// trim. `rowsTruncated` is true when this exceeds
    /// `MATRIX_AXIS_CAP`; in that case `rows`, `columns`, and
    /// `edges` are all empty so the frontend can banner without
    /// a partial draw.
    pub total_rows: usize,
    pub rows_truncated: bool,
    pub total_columns: usize,
    pub columns_truncated: bool,

    pub rows: Vec<MatrixNode>,
    pub columns: Vec<MatrixNode>,
    pub edges: Vec<MatrixEdge>,
}

#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error("project '{0}' is not currently mounted")]
    ProjectNotMounted(String),
    #[error("collection '{prefix}' not found in project '{slug}'")]
    CollectionNotFound { slug: String, prefix: String },
    #[error("link type is required")]
    LinkTypeRequired,
    #[error("unknown link type '{0}'")]
    UnknownLinkType(String),
    #[error("unknown review state(s): {0}")]
    UnknownReviewStates(String),
}

pub fn run(
    row_scope: Scope,
    column_scope: Scope,
    query: &MatrixQuery,
    world: &World,
) -> Result<MatrixResponse, MatrixError> {
    compute::build_matrix(row_scope, column_scope, query, world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_field_parses_and_dedupes() {
        assert_eq!(csv_field(None), None);
        assert_eq!(csv_field(Some("")), None);
        assert_eq!(
            csv_field(Some("a, b ,c,a")),
            Some(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])
        );
    }

    #[test]
    fn review_state_tag_parses_kebab_case() {
        assert_eq!(
            ReviewStateTag::parse("approved"),
            Some(ReviewStateTag::Approved)
        );
        assert_eq!(
            ReviewStateTag::parse("re-requested"),
            Some(ReviewStateTag::ReRequested)
        );
        assert_eq!(ReviewStateTag::parse("bogus"), None);
    }
}
