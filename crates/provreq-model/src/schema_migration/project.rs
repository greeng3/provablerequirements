//! Project-config migration chain.
//!
//! Current version is `1`; the chain is empty. Add a step here
//! the first time the project-config schema bumps.

use super::registry::{FileType, MigrationStep, Registry};

pub const PROJECT_STEPS: &[MigrationStep] = &[];

pub fn build() -> Registry {
    Registry::new(FileType::Project, 1, PROJECT_STEPS.to_vec())
}
