//! Doorstop importer (Phase 8).
//!
//! Implements the one-way doorstop → ReqForge translation
//! captured in the `INTEROP-doorstop*` items:
//!
//! - **Discovery** walks a source tree for `.doorstop.yml`
//!   marker files; each marks a doorstop document whose items
//!   become a ReqForge Collection (per
//!   INTEROP-doorstopDiscovery).
//! - **Parsing** reads the marker + per-item YAML files via
//!   [`parse`].
//! - **ID normalisation** preserves numeric padding and
//!   replaces dashes inside NANUs with underscores, per
//!   INTEROP-doorstopIdNormalization ([`ids`]).
//! - **Ref classification** splits URL-shaped refs from
//!   anything else via an explicit prefix whitelist, per
//!   INTEROP-doorstopRefHandling ([`refs`]).
//! - **Planning** combines the parsed tree with the target
//!   project's existing Collections to produce an
//!   [`ImportPlan`] — the data the preview endpoint returns
//!   and the execute step consumes. Planning never writes
//!   files.
//!
//! Writes are explicitly out of scope for 8.1; they land in
//! 8.2 on top of this plan.

pub mod execute;
pub mod ids;
pub mod parse;
pub mod plan;
pub mod refs;
pub mod report;

pub use execute::{ExecuteError, ExecuteTarget, execute};
pub use ids::{normalize_item_name, parse_doorstop_uid};
pub use parse::{DoorstopDocument, DoorstopItem, DoorstopSettings, ParseError};
pub use plan::{
    ImportPlan, PlanArtifact, PlanCollection, PlanError, PlanRefDisposition, PrefixCollision,
    UnresolvedLink, build_plan,
};
pub use refs::{RefClass, classify_ref};
pub use report::{ImportReport, ReportCollection, ReportRefDisposition, ReportTotals};
