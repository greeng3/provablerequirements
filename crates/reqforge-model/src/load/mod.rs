//! Reading Projects, Collections, and content-hosted artifacts off
//! disk into in-memory schema types.
//!
//! Design: one hard error per loader entry point (used when the load
//! can't meaningfully proceed — for example, a missing
//! `reqforge.json`), plus a vector of per-artifact / per-collection
//! [`LoadDiagnostic`] values for soft failures. Per
//! `FORMAT-fieldTolerance`, a broken artifact must not poison the
//! Project or its siblings.

pub mod artifact;
pub mod blob;
pub mod blob_allowlist;
pub mod project;
pub mod url;

pub use artifact::LoadedArtifact;
pub use blob::{BlobFacts, BlobLoadError, load_blob_artifact, load_blob_artifact_by_sidecar};
pub use blob_allowlist::{is_allowed_blob_extension, media_type_for_extension};
pub use project::{LoadedCollection, LoadedProject, ProjectLoadError, load_project};
pub use url::{UrlLoadError, load_url_artifact};

use std::path::PathBuf;

/// A soft failure surfaced alongside a successful Project load.
///
/// The loader accumulates these rather than aborting so the UI can
/// show a list of problems without losing access to the rest of the
/// Project.
#[derive(Debug)]
pub enum LoadDiagnostic {
    /// A Collection directory has no `.collection.json`.
    CollectionConfigMissing { dir_name: String },

    /// A Collection's `.collection.json` could not be read or parsed.
    CollectionConfigInvalid {
        dir_name: String,
        path: PathBuf,
        reason: String,
    },

    /// An artifact file could not be loaded. `name` is the filename
    /// stem (for a `.md` file) so the UI can link back to it.
    ArtifactFailed {
        name: String,
        path: PathBuf,
        reason: String,
    },

    /// A binary file in a collection directory has no
    /// `.reqforge.json` sidecar. Surfaces as a soft failure so a
    /// half-uploaded blob doesn't poison the collection; the UI
    /// can surface the stray file.
    OrphanBinary { path: PathBuf },

    /// A file's `schemaVersion` is newer than this build of
    /// ReqForge knows how to read, per `STOR-schemaNewerFilesRefused`.
    /// The containing project stays usable; the specific file is
    /// refused until the operator upgrades.
    SchemaTooNew {
        path: PathBuf,
        file_type: crate::schema_migration::FileType,
        found_version: u32,
        current_version: u32,
    },
}
