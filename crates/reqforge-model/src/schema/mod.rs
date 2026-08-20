//! On-disk schema types for every file ReqForge reads.
//!
//! Each type here mirrors a specific file format described under
//! `requirements-doorstop/format/`:
//!
//! - [`system::SystemConfig`]        ← `system.json`
//! - [`project::ProjectConfig`]      ← `reqforge.json`
//! - [`collection::CollectionConfig`] ← `.collection.json`
//! - [`artifact::Artifact`]          ← the JSON frontmatter of a
//!   content-hosted `.md`, or the `.reqforge.json` sidecar of a
//!   blob / URL artifact
//! - [`link::Link`]                  ← an entry in `Artifact.links`
//! - [`review::ReviewLogEntry`]      ← an entry in `Artifact.reviewLog`
//!
//! Every top-level type carries an `overflow` field that captures
//! unknown JSON keys per `FORMAT-fieldTolerance`, so files written
//! by newer / forked ReqForge versions round-trip without data loss.

pub mod artifact;
pub mod collection;
pub mod link;
pub mod project;
pub mod review;
pub mod sidecar;
pub mod system;

pub use artifact::{Artifact, ArtifactShape};
pub use collection::CollectionConfig;
pub use link::{Link, LinkHint};
pub use project::ProjectConfig;
pub use review::{AddedTodo, ReviewLogEntry};
pub use system::{SystemConfig, SystemLinkType, SystemProjectRef};

use std::collections::BTreeMap;

/// The unknown-field overflow bucket shared by every schema type.
///
/// `BTreeMap` is used (rather than `HashMap`) so unknown fields are
/// written back in a stable order.
pub type Overflow = BTreeMap<String, serde_json::Value>;
