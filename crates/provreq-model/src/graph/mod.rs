//! Graph-canvas data (Phase 7a).
//!
//! A single `GET /api/graph` endpoint backs the React-Flow
//! canvas in the UI. The handler takes a scope + filter set
//! and returns a capped sample of nodes + edges plus a
//! `truncated` flag so the UI can render a "showing 500 of N"
//! banner over the sample when the full set overflows.
//!
//! Linked authoring (drag-to-link) reuses the existing
//! `PUT /api/artifacts/:uuid` write path; Phase 7a introduces
//! no new write endpoint.

pub mod compute;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::reports::{Scope, ScopeDto};
use crate::world::World;

/// Soft cap on canvas-visible nodes per
/// `UX-linkCreationGraph`. React Flow stays responsive up to
/// this bound on mid-range hardware; beyond it, the UI nags
/// for additional filters.
pub const GRAPH_NODE_CAP: usize = 500;

/// Query-string parameters accepted by `GET /api/graph`. The
/// scope + includeInactive fields mirror Phase 6a reports for
/// consistency; `link_types` and `tags` are the two new graph-
/// specific filters.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQuery {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub include_inactive: Option<bool>,
    /// Comma-separated list of link-type names. When absent
    /// (the default), edges from every catalog link type are
    /// returned.
    #[serde(default)]
    pub link_types: Option<String>,
    /// Comma-separated list of tag strings. When absent, the
    /// filter is a no-op; when present, an artifact must
    /// carry at least one of the listed tags to appear.
    #[serde(default)]
    pub tags: Option<String>,
}

impl GraphQuery {
    pub fn include_inactive(&self) -> bool {
        self.include_inactive.unwrap_or(false)
    }

    pub fn link_type_list(&self) -> Option<Vec<String>> {
        csv_field(self.link_types.as_deref())
    }

    pub fn tag_list(&self) -> Option<Vec<String>> {
        csv_field(self.tags.as_deref())
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

/// Wire response for `GET /api/graph`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResponse {
    pub scope: ScopeDto,
    /// Total in-scope artifacts before the 500-cap trim.
    pub total_nodes: usize,
    /// `true` when the sample had to be trimmed to
    /// `GRAPH_NODE_CAP`. The frontend banners the overflow in
    /// that case.
    pub truncated: bool,
    /// `true` when every edge in the returned set uses a link
    /// type declared acyclic in the effective catalog. The
    /// frontend defaults to the hierarchical layout when this
    /// is set — the user can still flip back to force-directed.
    pub hint_all_edges_acyclic: bool,
    /// Catalog of link types referenced by the returned edges.
    /// Lets the UI show type metadata (directed / acyclic) in
    /// tooltips without a second round-trip.
    pub referenced_link_types: Vec<GraphLinkType>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub uuid: Uuid,
    pub project_slug: String,
    pub collection_prefix: String,
    pub artifact_name: String,
    pub title: String,
    pub shape: crate::schema::ArtifactShape,
    pub active: bool,
    pub derived: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source_uuid: Uuid,
    pub target_uuid: Uuid,
    pub link_type: String,
    /// `true` iff the link type is declared acyclic + directed
    /// in the effective catalog (derives-from, supersedes). The
    /// frontend uses this both to drive the hierarchical-layout
    /// hint and to pick an arrow style per edge.
    pub acyclic: bool,
    pub directed: bool,
}

/// Compact link-type metadata piggy-backed on the response so
/// the UI can style tooltips without a separate fetch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphLinkType {
    pub name: String,
    pub inverse_name: String,
    pub directed: bool,
    pub acyclic: bool,
}

impl From<&crate::links::LinkType> for GraphLinkType {
    fn from(lt: &crate::links::LinkType) -> Self {
        Self {
            name: lt.name.to_string(),
            inverse_name: lt.inverse_name.to_string(),
            directed: lt.directed,
            acyclic: lt.acyclic,
        }
    }
}

/// Error surface. `ProjectNotMounted` / `CollectionNotFound`
/// mirror the Phase 6a reports wrap so the HTTP handler can
/// translate both kinds into 404s with consistent wording.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("project '{0}' is not currently mounted")]
    ProjectNotMounted(String),
    #[error("collection '{prefix}' not found in project '{slug}'")]
    CollectionNotFound { slug: String, prefix: String },
}

pub fn run(scope: Scope, query: &GraphQuery, world: &World) -> Result<GraphResponse, GraphError> {
    compute::build_graph(scope, query, world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_field_parses_and_dedupes() {
        assert_eq!(csv_field(None), None);
        assert_eq!(csv_field(Some("")), None);
        assert_eq!(csv_field(Some(" , ")), None);
        assert_eq!(
            csv_field(Some("a, b ,c,a")),
            Some(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])
        );
    }

    #[test]
    fn graph_query_defaults_are_permissive() {
        let q = GraphQuery::default();
        assert!(!q.include_inactive());
        assert!(q.link_type_list().is_none());
        assert!(q.tag_list().is_none());
    }
}
