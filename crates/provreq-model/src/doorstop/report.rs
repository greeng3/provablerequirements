//! Import-report DTOs (Phase 8.2).
//!
//! The execute step fills in an [`ImportReport`] alongside the
//! writes it performs. The report lives in memory on
//! `AppState` keyed by project slug so the frontend can fetch
//! it via the regular report-kind export surface (JSON / CSV /
//! HTML per REPORT-baselineExports).
//!
//! Per the locked decision, the report is not persisted to
//! disk — it survives the current process only.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::doorstop::plan::{PrefixCollision, UnresolvedLink};

/// Response + cached shape for a doorstop import run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub project_slug: String,
    pub source: String,
    pub import_run_at: DateTime<Utc>,

    /// Collection summaries in prefix-ascending order.
    pub collections: Vec<ReportCollection>,

    /// Totals across the whole run.
    pub totals: ReportTotals,

    /// Every doorstop `ref` row, grouped by disposition, so
    /// operators can scan how each external reference was
    /// handled (per INTEROP-doorstopImportReport).
    pub ref_dispositions: Vec<ReportRefDisposition>,

    /// Unresolved-link list (already populated by the plan;
    /// re-exposed here so the report is self-contained).
    pub unresolved_links: Vec<UnresolvedLink>,

    /// Prefix collisions. For a *successful* import this is
    /// always empty — the endpoint refuses the run when any
    /// collision exists. Carried here for diagnostic symmetry
    /// with the preview response shape.
    pub prefix_collisions: Vec<PrefixCollision>,

    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCollection {
    pub prefix: String,
    pub name: String,
    pub directory_name: String,
    pub artifact_count: usize,
    /// Count of artifacts that carry a synthetic initial
    /// review-log entry from a non-null reviewed hash.
    pub synthetic_review_count: usize,
    /// Count of artifacts that carry extension fields routed
    /// into the imported artifact's `legacy` object.
    pub legacy_preserved_count: usize,
    /// Count of derives-from links written in this
    /// collection's artifacts.
    pub derives_from_link_count: usize,
    /// URL-companion artifact count (per
    /// INTEROP-doorstopRefHandling URL branch).
    pub url_artifact_count: usize,
    pub source_marker_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTotals {
    pub collections_created: usize,
    pub artifacts_imported: usize,
    pub derives_from_links: usize,
    pub url_artifacts: usize,
    pub cites_links: usize,
    pub legacy_refs: usize,
    pub synthetic_review_entries: usize,
    pub legacy_preserved_fields: usize,
    pub unresolved_link_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReportRefDisposition {
    UrlArtifact {
        source_uid: String,
        url: String,
        url_artifact_name: String,
    },
    Legacy {
        source_uid: String,
        value: String,
    },
}
