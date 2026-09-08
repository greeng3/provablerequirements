//! Filesystem write primitives shared by every CRUD path.
//!
//! Atomic temp-file-then-rename writes, per `STOR-atomicWrites`,
//! plus ownership reconciliation against the target repository's
//! `.git` entry, per `DEPLOY-chownFromDotGit`. Phase 2 layers
//! `write_artifact` (frontmatter + body) and similar helpers on
//! top of these primitives.

pub mod artifact_file;
pub mod atomic;
pub mod ownership;
pub mod sidecar;

pub use artifact_file::{
    RenderError, WriteArtifactError, render_artifact_file, write_artifact_file,
};
pub use atomic::{AtomicWriteError, atomic_write, atomic_write_with_mode};
pub use ownership::{OwnershipError, OwnershipOverrides, reconcile_ownership};
pub use sidecar::{
    ShapeWriteError, render_sidecar_json, write_blob_and_sidecar, write_sidecar_only,
};
