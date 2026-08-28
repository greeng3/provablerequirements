//! Error type for the suggestions persistence layer.

use crate::write::AtomicWriteError;

#[derive(Debug, thiserror::Error)]
pub enum SuggestionError {
    #[error("I/O error reading suggestions sidecar: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error parsing suggestions sidecar: {0}")]
    Json(#[from] serde_json::Error),

    #[error("atomic write error writing suggestions sidecar: {0}")]
    Write(#[from] AtomicWriteError),

    #[error("schema version {found} in suggestions sidecar is newer than supported ({supported})")]
    SchemaTooNew { found: u32, supported: u32 },
}
