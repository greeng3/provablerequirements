//! URL artifact loader (Phase 5a).
//!
//! A URL artifact is a single `.reqforge.json` on disk with
//! `shape == "url"` and a `url` field that must be an HTTP / HTTPS
//! URL. There's no binary peer — the artifact is a reference to
//! externally-hosted content, and `UX-urlArtifactChecking` records
//! `checkedAt` / `checkStatus` fields the Phase 5b check action
//! populates.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::schema::sidecar::artifact_name_from_sidecar;
use crate::schema::{Artifact, ArtifactShape};
use crate::schema_migration::{FileType, SchemaMigrationError, migrate_value};

use super::artifact::LoadedArtifact;

/// Load a URL artifact from its sidecar path.
pub fn load_url_artifact(sidecar_path: &Path) -> Result<LoadedArtifact, UrlLoadError> {
    let text = fs::read_to_string(sidecar_path).map_err(|source| UrlLoadError::Io {
        path: sidecar_path.to_path_buf(),
        source,
    })?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| UrlLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let (migrated, _) =
        migrate_value(FileType::Artifact, raw).map_err(|source| UrlLoadError::Schema {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let metadata: Artifact =
        serde_json::from_value(migrated).map_err(|source| UrlLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    if metadata.shape != ArtifactShape::Url {
        return Err(UrlLoadError::ShapeMismatch {
            path: sidecar_path.to_path_buf(),
            shape: metadata.shape,
        });
    }
    let url = metadata
        .url
        .as_deref()
        .ok_or_else(|| UrlLoadError::MissingUrl {
            path: sidecar_path.to_path_buf(),
        })?;
    if !is_http_or_https(url) {
        return Err(UrlLoadError::UnsupportedScheme {
            path: sidecar_path.to_path_buf(),
            url: url.to_owned(),
        });
    }
    let name = artifact_name_from_sidecar(sidecar_path).ok_or_else(|| {
        UrlLoadError::InvalidSidecarName {
            path: sidecar_path.to_path_buf(),
        }
    })?;
    Ok(LoadedArtifact {
        name,
        source_path: sidecar_path.to_path_buf(),
        metadata,
        body: None,
        blob: None,
    })
}

/// Cheap scheme check — full URL parsing arrives in Phase 5b when
/// the `url` crate gets pulled in for the write path. For now this
/// rejects `file://`, `javascript:`, `data:`, and similar, which is
/// enough for `UX-urlArtifactChecking`'s contract.
fn is_http_or_https(url: &str) -> bool {
    let lower = url.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[derive(Debug, Error)]
pub enum UrlLoadError {
    #[error("i/o error reading {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid URL sidecar JSON at {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("sidecar at {} has shape '{shape:?}', expected 'url'", path.display())]
    ShapeMismatch { path: PathBuf, shape: ArtifactShape },

    #[error("URL sidecar at {} does not declare a url field", path.display())]
    MissingUrl { path: PathBuf },

    #[error(
        "URL sidecar at {} declared URL '{url}' which is not http(s)",
        path.display()
    )]
    UnsupportedScheme { path: PathBuf, url: String },

    #[error("sidecar filename at {} is not parseable as an artifact name", path.display())]
    InvalidSidecarName { path: PathBuf },

    #[error("schema migration failed for {}: {source}", path.display())]
    Schema {
        path: PathBuf,
        #[source]
        source: SchemaMigrationError,
    },
}

impl UrlLoadError {
    pub fn short_reason(&self) -> String {
        match self {
            Self::Io { source, .. } => format!("i/o error: {source}"),
            Self::Json { source, .. } => format!("JSON: {source}"),
            Self::ShapeMismatch { shape, .. } => {
                format!("sidecar declares shape {shape:?}, not url")
            }
            Self::MissingUrl { .. } => "sidecar has no url field".to_owned(),
            Self::UnsupportedScheme { url, .. } => {
                format!("URL '{url}' is not http or https")
            }
            Self::InvalidSidecarName { .. } => "sidecar filename is not parseable".to_owned(),
            Self::Schema { source, .. } => format!("schema: {source}"),
        }
    }

    /// If the failure is a too-new schema version, return the
    /// `(found, current)` pair so the project loader can emit a
    /// dedicated `SchemaTooNew` diagnostic.
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn url_metadata(url: Option<&str>) -> Artifact {
        Artifact {
            schema_version: 1,
            uuid: Uuid::now_v7(),
            title: "RFC 9110".to_owned(),
            shape: ArtifactShape::Url,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            links: Vec::new(),
            review_log: Vec::new(),
            description: None,
            expects_code_trace: None,
            active: None,
            derived: None,
            tags: None,
            outline_level: None,
            legacy: None,
            blob_path: None,
            url: url.map(String::from),
            checked_at: None,
            check_status: None,
            overflow: BTreeMap::new(),
        }
    }

    fn write_url_sidecar(root: &Path, name: &str, meta: &Artifact) -> PathBuf {
        let path = root.join(format!("{name}.reqforge.json"));
        fs::write(&path, serde_json::to_vec_pretty(meta).unwrap()).unwrap();
        path
    }

    #[test]
    fn happy_path_loads_url_artifact() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let meta = url_metadata(Some("https://www.rfc-editor.org/rfc/rfc9110"));
        let sidecar = write_url_sidecar(root, "RFC-9110", &meta);
        let loaded = load_url_artifact(&sidecar).unwrap();
        assert_eq!(loaded.name, "RFC-9110");
        assert_eq!(
            loaded.metadata.url.as_deref(),
            Some("https://www.rfc-editor.org/rfc/rfc9110"),
        );
        assert!(loaded.body.is_none());
        assert!(loaded.blob.is_none());
    }

    #[test]
    fn rejects_shape_content() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let mut meta = url_metadata(Some("https://example.com"));
        meta.shape = ArtifactShape::Content;
        let sidecar = write_url_sidecar(root, "example", &meta);
        let err = load_url_artifact(&sidecar).unwrap_err();
        assert!(matches!(err, UrlLoadError::ShapeMismatch { .. }));
    }

    #[test]
    fn rejects_missing_url_field() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let meta = url_metadata(None);
        let sidecar = write_url_sidecar(root, "empty", &meta);
        let err = load_url_artifact(&sidecar).unwrap_err();
        assert!(matches!(err, UrlLoadError::MissingUrl { .. }));
    }

    #[test]
    fn rejects_non_http_scheme() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let meta = url_metadata(Some("file:///etc/passwd"));
        let sidecar = write_url_sidecar(root, "scary", &meta);
        let err = load_url_artifact(&sidecar).unwrap_err();
        assert!(matches!(err, UrlLoadError::UnsupportedScheme { .. }));
    }

    #[test]
    fn accepts_both_http_and_https() {
        for url in ["http://example.com", "https://example.com"] {
            let temp = tempdir().unwrap();
            let root = temp.path();
            let meta = url_metadata(Some(url));
            let sidecar = write_url_sidecar(root, "example", &meta);
            let loaded = load_url_artifact(&sidecar).unwrap();
            assert_eq!(loaded.metadata.url.as_deref(), Some(url));
        }
    }
}
