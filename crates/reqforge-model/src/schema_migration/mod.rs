//! Schema migration engine (Phase 11a).
//!
//! Per the `STOR-schema*` specs: every ReqForge-authored file
//! carries a `schemaVersion` integer. When ReqForge reads a file
//! whose version is below the current build's, it runs a chain
//! of registered migration functions in sequence to produce the
//! current in-memory representation. Files newer than the
//! current build's version refuse to load so we never guess.
//!
//! Today every chain is empty — the on-disk format started at
//! `v=1` and hasn't bumped — so `migrate_value` for a v=1 file is
//! a pass-through. The infrastructure is in place so the first
//! actual schema bump is a single-file change (register a step).
//!
//! Sub-modules:
//!
//! - [`registry`] — `FileType`, `Registry`, `MigrationStep`, and
//!   the chain engine.
//! - [`errors`] — [`SchemaMigrationError`].
//! - [`artifact`] / [`collection`] / [`project`] / [`system`] —
//!   per-file-type chain declarations. Add future steps here.

pub mod artifact;
pub mod bulk;
pub mod collection;
pub mod errors;
pub mod project;
pub mod registry;
pub mod system;

use serde_json::Value;

pub use errors::SchemaMigrationError;
pub use registry::{FileType, MigrationOutcome, Registry};

/// Current `schemaVersion` for each file type. Exposed as `const`
/// so the bulk-migrate handler + the frontend DTO can report the
/// target version without constructing a `Registry`.
pub const CURRENT_ARTIFACT_VERSION: u32 = 1;
pub const CURRENT_COLLECTION_VERSION: u32 = 1;
pub const CURRENT_PROJECT_VERSION: u32 = 1;
pub const CURRENT_SYSTEM_VERSION: u32 = 2;

/// Top-level entry point: dispatch to the right registry based
/// on `file_type`, then run its chain.
///
/// Callers that already hold a `&Registry` can call
/// [`Registry::migrate`] directly. This wrapper is for the common
/// case where the caller knows only "this is an artifact" etc.
pub fn migrate_value(
    file_type: FileType,
    value: Value,
) -> Result<(Value, MigrationOutcome), SchemaMigrationError> {
    registry_for(file_type).migrate(value)
}

/// Current version for a given file type — matches the build-
/// time constants above; kept as a function so handlers that
/// receive a `FileType` at runtime don't have to branch.
pub fn current_version(file_type: FileType) -> u32 {
    match file_type {
        FileType::Artifact => CURRENT_ARTIFACT_VERSION,
        FileType::Collection => CURRENT_COLLECTION_VERSION,
        FileType::Project => CURRENT_PROJECT_VERSION,
        FileType::System => CURRENT_SYSTEM_VERSION,
    }
}

fn registry_for(file_type: FileType) -> Registry {
    match file_type {
        FileType::Artifact => artifact::build(),
        FileType::Collection => collection::build(),
        FileType::Project => project::build(),
        FileType::System => system::build(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn current_version_matches_constants_for_every_file_type() {
        assert_eq!(
            current_version(FileType::Artifact),
            CURRENT_ARTIFACT_VERSION
        );
        assert_eq!(
            current_version(FileType::Collection),
            CURRENT_COLLECTION_VERSION
        );
        assert_eq!(current_version(FileType::Project), CURRENT_PROJECT_VERSION);
        assert_eq!(current_version(FileType::System), CURRENT_SYSTEM_VERSION);
    }

    #[test]
    fn empty_chains_are_identity_for_current_version_files() {
        for ft in [
            FileType::Artifact,
            FileType::Collection,
            FileType::Project,
            FileType::System,
        ] {
            let (out, outcome) =
                migrate_value(ft, json!({ "schemaVersion": current_version(ft) })).unwrap();
            assert_eq!(out["schemaVersion"], current_version(ft));
            assert!(!outcome.migrated);
            assert_eq!(outcome.from_version, current_version(ft));
            assert_eq!(outcome.to_version, current_version(ft));
        }
    }

    #[test]
    fn too_new_refused_for_every_file_type() {
        for ft in [
            FileType::Artifact,
            FileType::Collection,
            FileType::Project,
            FileType::System,
        ] {
            let err =
                migrate_value(ft, json!({ "schemaVersion": current_version(ft) + 1 })).unwrap_err();
            assert!(matches!(err, SchemaMigrationError::NewerThanCurrent { .. }));
        }
    }
}
