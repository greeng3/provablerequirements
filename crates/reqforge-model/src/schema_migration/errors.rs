//! Error type for the schema-migration engine.
//!
//! Surfaces at load time (wrapped by each loader's error enum) and
//! at bulk-migrate time (returned directly by the HTTP handler).
//! Kept narrow — the registry only produces a small set of failure
//! modes, and the caller doesn't need to branch further.

use thiserror::Error;

use super::registry::FileType;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchemaMigrationError {
    /// The raw value didn't carry a `schemaVersion` field, or it
    /// wasn't a positive integer. Per the on-disk format, every
    /// ReqForge-authored file must carry one; its absence points
    /// at a hand-edited or externally-produced file.
    #[error("{file_type} file is missing a valid `schemaVersion` (found `{found}`)")]
    InvalidSchemaVersion {
        file_type: FileType,
        /// Display of what we saw, truncated. Useful for the
        /// diagnostic surface; never fed back into any typed path.
        found: String,
    },

    /// The file's `schemaVersion` is newer than this build knows
    /// about. Per `STOR-schemaNewerFilesRefused`, we refuse to load
    /// rather than guess.
    #[error(
        "{file_type} file has schemaVersion {found}, which is newer than the \
         current version ({current}); upgrade ReqForge to read this file"
    )]
    NewerThanCurrent {
        file_type: FileType,
        found: u32,
        current: u32,
    },

    /// A registered migration step returned an error. Names the
    /// step (from → to) so operators can trace which bump is
    /// unhappy with the file.
    #[error("{file_type} migration {from_version} → {to_version} failed: {detail}")]
    StepFailed {
        file_type: FileType,
        from_version: u32,
        to_version: u32,
        detail: String,
    },
}
