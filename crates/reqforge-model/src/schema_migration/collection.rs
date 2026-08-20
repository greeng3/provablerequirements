//! Collection-config migration chain.
//!
//! Current version is `1`; the chain is empty. Add a step here
//! the first time the collection-config schema bumps.

use super::registry::{FileType, MigrationStep, Registry};

pub const COLLECTION_STEPS: &[MigrationStep] = &[];

pub fn build() -> Registry {
    Registry::new(FileType::Collection, 1, COLLECTION_STEPS.to_vec())
}
