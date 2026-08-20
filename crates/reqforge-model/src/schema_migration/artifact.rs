//! Artifact-frontmatter migration chain.
//!
//! Current version is `1`; the chain is empty. Add a step here
//! the first time the artifact schema bumps, and update the
//! `current_version` constant in [`super::CURRENT_ARTIFACT_VERSION`].

use super::registry::{FileType, MigrationStep, Registry};

/// Migration steps for the artifact-frontmatter file type, in
/// ascending `from_version` order. Empty today.
pub const ARTIFACT_STEPS: &[MigrationStep] = &[];

pub fn build() -> Registry {
    Registry::new(FileType::Artifact, 1, ARTIFACT_STEPS.to_vec())
}
