//! Content-hosted artifact loader.
//!
//! Blob and URL artifacts have their own loader modules
//! (`crate::load::blob` and `crate::load::url`); this module is
//! the content-hosted path only.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::frontmatter::{FrontmatterError, split_frontmatter};
use crate::schema::Artifact;
use crate::schema_migration::{FileType, SchemaMigrationError, migrate_value};

use super::blob::BlobFacts;

/// One artifact as loaded from disk.
#[derive(Debug, Clone)]
pub struct LoadedArtifact {
    /// The filename stem. For content-hosted artifacts this comes
    /// from the `.md` file's stem; for blob/URL artifacts it comes
    /// from the sidecar stem with any inner extension stripped
    /// (`DES-spec.pdf.reqforge.json` → `DES-spec`).
    pub name: String,
    /// Absolute path to the file the artifact was read from. For
    /// blob / URL shapes this is the sidecar, not the binary peer.
    pub source_path: PathBuf,
    /// Parsed frontmatter / sidecar metadata.
    pub metadata: Artifact,
    /// Markdown body, for content-hosted artifacts. `None` for blob
    /// and URL artifacts.
    pub body: Option<String>,
    /// Blob stat facts (size, sha256, media type, absolute binary
    /// path). `Some` only when `metadata.shape == Blob`.
    pub blob: Option<BlobFacts>,
}

/// Load a single content-hosted `.md` artifact.
///
/// The filename stem becomes [`LoadedArtifact::name`]; the caller is
/// responsible for directory walking.
pub fn load_content_artifact(path: &Path) -> Result<LoadedArtifact, ArtifactLoadError> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ArtifactLoadError::InvalidName {
            path: path.to_path_buf(),
        })?
        .to_owned();

    let text = std::fs::read_to_string(path).map_err(|source| ArtifactLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let (json, body) =
        split_frontmatter(&text).map_err(|source| ArtifactLoadError::Frontmatter {
            path: path.to_path_buf(),
            source,
        })?;

    let raw: serde_json::Value =
        serde_json::from_str(json).map_err(|source| ArtifactLoadError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    let (migrated, _outcome) =
        migrate_value(FileType::Artifact, raw).map_err(|source| ArtifactLoadError::Schema {
            path: path.to_path_buf(),
            source,
        })?;
    let metadata: Artifact =
        serde_json::from_value(migrated).map_err(|source| ArtifactLoadError::Json {
            path: path.to_path_buf(),
            source,
        })?;

    Ok(LoadedArtifact {
        name,
        source_path: path.to_path_buf(),
        metadata,
        body: Some(body.to_owned()),
        blob: None,
    })
}

#[derive(Debug, Error)]
pub enum ArtifactLoadError {
    #[error("artifact filename is not valid UTF-8: {}", path.display())]
    InvalidName { path: PathBuf },

    #[error("i/o error reading {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("frontmatter error in {}: {source}", path.display())]
    Frontmatter {
        path: PathBuf,
        #[source]
        source: FrontmatterError,
    },

    #[error("JSON parse error in {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("schema migration failed for {}: {source}", path.display())]
    Schema {
        path: PathBuf,
        #[source]
        source: SchemaMigrationError,
    },
}

impl ArtifactLoadError {
    /// Short human-readable reason, for diagnostic aggregation.
    pub fn short_reason(&self) -> String {
        match self {
            Self::InvalidName { .. } => "filename is not valid UTF-8".to_owned(),
            Self::Io { source, .. } => format!("i/o error: {source}"),
            Self::Frontmatter { source, .. } => format!("frontmatter: {source}"),
            Self::Json { source, .. } => format!("JSON: {source}"),
            Self::Schema { source, .. } => format!("schema: {source}"),
        }
    }

    /// Typed accessor — if the underlying failure was a
    /// schema-version mismatch, return the found / current
    /// versions so the project loader can emit a dedicated
    /// `SchemaTooNew` diagnostic rather than a generic
    /// `ArtifactFailed`.
    pub fn schema_too_new(&self) -> Option<(u32, u32)> {
        match self {
            Self::Schema {
                source: SchemaMigrationError::NewerThanCurrent { found, current, .. },
                ..
            } => Some((*found, *current)),
            _ => None,
        }
    }
}
