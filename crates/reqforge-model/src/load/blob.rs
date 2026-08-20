//! Blob artifact loader (Phase 5a).
//!
//! A blob artifact on disk is two files: the binary (e.g.
//! `DES-spec.pdf`) and the sidecar (`DES-spec.pdf.reqforge.json`).
//! The sidecar's JSON mirrors the content-hosted `Artifact` type
//! but `shape == "blob"` and the binary peer lives at `blob_path`
//! relative to the project root.
//!
//! This loader reads the sidecar, verifies the binary peer exists,
//! stats its byte size and sha256 hash (thumbnail-cache keying in
//! Phase 5c), and returns a `LoadedArtifact` with the new `blob`
//! facts attached. It does NOT load the binary into memory — blobs
//! are streamed on demand via the Phase 5b download endpoint.

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::load::blob_allowlist::{is_allowed_blob_extension, media_type_for_extension};
use crate::schema::sidecar::{artifact_name_from_sidecar, blob_path_for_sidecar};
use crate::schema::{Artifact, ArtifactShape};
use crate::schema_migration::{FileType, SchemaMigrationError, migrate_value};

use super::artifact::LoadedArtifact;

/// Stat + hash facts about a blob's binary peer. Attached to the
/// `LoadedArtifact` when the shape is `blob` so downstream code
/// (Phase 5b download, Phase 5c thumbnails, Phase 5c DTO) doesn't
/// re-stat on every request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobFacts {
    /// Absolute path to the binary peer on disk.
    pub binary_path: PathBuf,
    /// Binary size in bytes, captured at load time. Stale after a
    /// re-upload; discovery refreshes it on the next pass.
    pub byte_size: u64,
    /// sha256 hex digest of the binary at load time. Used as the
    /// thumbnail cache key (5c) and as an `ETag` (5b).
    pub content_hash: String,
    /// Best-effort media type derived from the binary's extension
    /// (Phase 5b adds a magic-byte sniff via `infer` on upload).
    pub media_type: &'static str,
}

/// Load a blob artifact from its sidecar path.
///
/// `project_root` is needed so the sidecar's `blob_path` (a
/// repo-relative string per `FORMAT-blobSidecar`) can be resolved
/// to an absolute path that's validated to stay inside the project
/// tree.
pub fn load_blob_artifact(
    sidecar_path: &Path,
    project_root: &Path,
) -> Result<LoadedArtifact, BlobLoadError> {
    let text = fs::read_to_string(sidecar_path).map_err(|source| BlobLoadError::Io {
        path: sidecar_path.to_path_buf(),
        source,
    })?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| BlobLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let (migrated, _) =
        migrate_value(FileType::Artifact, raw).map_err(|source| BlobLoadError::Schema {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let metadata: Artifact =
        serde_json::from_value(migrated).map_err(|source| BlobLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    if metadata.shape != ArtifactShape::Blob {
        return Err(BlobLoadError::ShapeMismatch {
            path: sidecar_path.to_path_buf(),
            shape: metadata.shape,
        });
    }

    let blob_path_field =
        metadata
            .blob_path
            .as_deref()
            .ok_or_else(|| BlobLoadError::MissingBlobPath {
                path: sidecar_path.to_path_buf(),
            })?;
    let binary_path =
        resolve_blob_path(project_root, blob_path_field).map_err(|err| match err {
            BlobPathError::EscapesProjectRoot => BlobLoadError::BlobPathEscapesRoot {
                path: sidecar_path.to_path_buf(),
                declared: blob_path_field.to_owned(),
            },
            BlobPathError::Absolute => BlobLoadError::BlobPathAbsolute {
                path: sidecar_path.to_path_buf(),
                declared: blob_path_field.to_owned(),
            },
        })?;

    if !binary_path.is_file() {
        return Err(BlobLoadError::BinaryMissing {
            path: sidecar_path.to_path_buf(),
            expected_binary: binary_path,
        });
    }

    let extension = binary_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_owned();
    if !is_allowed_blob_extension(&extension) {
        return Err(BlobLoadError::ExtensionNotAllowed {
            path: sidecar_path.to_path_buf(),
            extension,
        });
    }
    let media_type = media_type_for_extension(&extension);

    let (byte_size, content_hash) =
        hash_file(&binary_path).map_err(|source| BlobLoadError::Io {
            path: binary_path.clone(),
            source,
        })?;

    let name = artifact_name_from_sidecar(sidecar_path).ok_or_else(|| {
        BlobLoadError::InvalidSidecarName {
            path: sidecar_path.to_path_buf(),
        }
    })?;

    Ok(LoadedArtifact {
        name,
        source_path: sidecar_path.to_path_buf(),
        metadata,
        body: None,
        blob: Some(BlobFacts {
            binary_path,
            byte_size,
            content_hash,
            media_type,
        }),
    })
}

/// Resolve a `blob_path` field (a relative POSIX path from the
/// project root) into an absolute path, while refusing anything
/// that escapes the project tree (parent-dir traversal, absolute
/// paths).
fn resolve_blob_path(project_root: &Path, declared: &str) -> Result<PathBuf, BlobPathError> {
    let declared_path = Path::new(declared);
    if declared_path.is_absolute() {
        return Err(BlobPathError::Absolute);
    }
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in declared_path.components() {
        use std::path::Component;
        match comp {
            Component::Normal(part) => components.push(part),
            Component::CurDir => continue,
            Component::ParentDir => {
                // `..` could navigate out; `pop()` returning None
                // means we're already at the project root.
                if components.pop().is_none() {
                    return Err(BlobPathError::EscapesProjectRoot);
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(BlobPathError::Absolute);
            }
        }
    }
    let mut out = project_root.to_path_buf();
    for part in components {
        out.push(part);
    }
    Ok(out)
}

enum BlobPathError {
    EscapesProjectRoot,
    Absolute,
}

fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut size: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let digest = hasher.finalize();
    Ok((size, hex::encode(digest)))
}

/// Minimal hex encoder — a tiny hex-encoding helper kept local
/// rather than pulling in the `hex` crate for this one call site.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        static CHARS: &[u8; 16] = b"0123456789abcdef";
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(CHARS[(byte >> 4) as usize] as char);
            out.push(CHARS[(byte & 0x0f) as usize] as char);
        }
        out
    }
}

#[derive(Debug, Error)]
pub enum BlobLoadError {
    #[error("i/o error reading {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid blob sidecar JSON at {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("sidecar at {} has shape '{shape:?}', expected 'blob'", path.display())]
    ShapeMismatch { path: PathBuf, shape: ArtifactShape },

    #[error(
        "blob sidecar at {} does not declare a blobPath field",
        path.display()
    )]
    MissingBlobPath { path: PathBuf },

    #[error(
        "blob sidecar at {} declared an absolute blobPath '{declared}'; must be relative to the project root",
        path.display()
    )]
    BlobPathAbsolute { path: PathBuf, declared: String },

    #[error(
        "blob sidecar at {} declared '{declared}' which escapes the project root",
        path.display()
    )]
    BlobPathEscapesRoot { path: PathBuf, declared: String },

    #[error(
        "blob sidecar at {} points at a binary that does not exist on disk: {}",
        path.display(), expected_binary.display(),
    )]
    BinaryMissing {
        path: PathBuf,
        expected_binary: PathBuf,
    },

    #[error(
        "blob sidecar at {} points at an extension '{extension}' not in the Phase 5 allowlist",
        path.display()
    )]
    ExtensionNotAllowed { path: PathBuf, extension: String },

    #[error("sidecar filename at {} is not parseable as an artifact name", path.display())]
    InvalidSidecarName { path: PathBuf },

    #[error("schema migration failed for {}: {source}", path.display())]
    Schema {
        path: PathBuf,
        #[source]
        source: SchemaMigrationError,
    },
}

impl BlobLoadError {
    pub fn short_reason(&self) -> String {
        match self {
            Self::Io { source, .. } => format!("i/o error: {source}"),
            Self::Json { source, .. } => format!("JSON: {source}"),
            Self::ShapeMismatch { shape, .. } => {
                format!("sidecar declares shape {shape:?}, not blob")
            }
            Self::MissingBlobPath { .. } => "sidecar has no blobPath field".to_owned(),
            Self::BlobPathAbsolute { declared, .. } => {
                format!("blobPath '{declared}' is absolute")
            }
            Self::BlobPathEscapesRoot { declared, .. } => {
                format!("blobPath '{declared}' escapes the project root")
            }
            Self::BinaryMissing {
                expected_binary, ..
            } => {
                format!("binary peer missing at {}", expected_binary.display())
            }
            Self::ExtensionNotAllowed { extension, .. } => {
                format!("extension '{extension}' not in the blob allowlist")
            }
            Self::InvalidSidecarName { .. } => "sidecar filename is not parseable".to_owned(),
            Self::Schema { source, .. } => format!("schema: {source}"),
        }
    }

    /// If the failure is a too-new schema version, return the
    /// `(found, current)` pair for the project loader to emit a
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

/// Convenience: load a blob from a sidecar path that's already been
/// verified to end in `.reqforge.json`. Derives the binary path
/// from the sidecar rather than reading `blob_path` from inside
/// the JSON — used by the collection walker where the sidecar's
/// `blob_path` is expected to mirror the sibling layout.
pub fn load_blob_artifact_by_sidecar(
    sidecar_path: &Path,
    project_root: &Path,
) -> Result<LoadedArtifact, BlobLoadError> {
    // Preferred path: the sidecar's `blob_path` field IS the
    // canonical declaration. Fall through to the sibling-derived
    // binary only if the field is absent — that handles the Phase
    // 5b write path where the server populates `blob_path` at
    // upload time, while still tolerating an older sidecar that a
    // contributor hand-authored without it.
    let text = fs::read_to_string(sidecar_path).map_err(|source| BlobLoadError::Io {
        path: sidecar_path.to_path_buf(),
        source,
    })?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| BlobLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let (migrated, _) =
        migrate_value(FileType::Artifact, raw).map_err(|source| BlobLoadError::Schema {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    let metadata: Artifact =
        serde_json::from_value(migrated).map_err(|source| BlobLoadError::Json {
            path: sidecar_path.to_path_buf(),
            source,
        })?;
    if metadata.shape != ArtifactShape::Blob {
        return Err(BlobLoadError::ShapeMismatch {
            path: sidecar_path.to_path_buf(),
            shape: metadata.shape,
        });
    }
    if metadata.blob_path.is_some() {
        return load_blob_artifact(sidecar_path, project_root);
    }
    // Recover the sibling binary.
    let binary_path =
        blob_path_for_sidecar(sidecar_path).ok_or_else(|| BlobLoadError::MissingBlobPath {
            path: sidecar_path.to_path_buf(),
        })?;
    if !binary_path.is_file() {
        return Err(BlobLoadError::BinaryMissing {
            path: sidecar_path.to_path_buf(),
            expected_binary: binary_path,
        });
    }
    let extension = binary_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_owned();
    if !is_allowed_blob_extension(&extension) {
        return Err(BlobLoadError::ExtensionNotAllowed {
            path: sidecar_path.to_path_buf(),
            extension,
        });
    }
    let media_type = media_type_for_extension(&extension);
    let (byte_size, content_hash) =
        hash_file(&binary_path).map_err(|source| BlobLoadError::Io {
            path: binary_path.clone(),
            source,
        })?;
    let name = artifact_name_from_sidecar(sidecar_path).ok_or_else(|| {
        BlobLoadError::InvalidSidecarName {
            path: sidecar_path.to_path_buf(),
        }
    })?;
    Ok(LoadedArtifact {
        name,
        source_path: sidecar_path.to_path_buf(),
        metadata,
        body: None,
        blob: Some(BlobFacts {
            binary_path,
            byte_size,
            content_hash,
            media_type,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Artifact, ArtifactShape};
    use chrono::Utc;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn blob_metadata(blob_path: &str) -> Artifact {
        Artifact {
            schema_version: 1,
            uuid: Uuid::now_v7(),
            title: "Design doc".to_owned(),
            shape: ArtifactShape::Blob,
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
            blob_path: Some(blob_path.to_owned()),
            url: None,
            checked_at: None,
            check_status: None,
            overflow: BTreeMap::new(),
        }
    }

    fn write_sidecar_and_binary(
        project_root: &Path,
        relative_binary: &str,
        bytes: &[u8],
    ) -> PathBuf {
        let binary_path = project_root.join(relative_binary);
        if let Some(parent) = binary_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&binary_path, bytes).unwrap();
        let sidecar_path = crate::schema::sidecar::sidecar_path_for_blob(&binary_path);
        let metadata = blob_metadata(relative_binary);
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        sidecar_path
    }

    #[test]
    fn happy_path_loads_blob_with_stats_and_hash() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let sidecar =
            write_sidecar_and_binary(root, "artifacts/REQ/DES-spec.pdf", b"%PDF-1.4\n...\n");
        let loaded = load_blob_artifact(&sidecar, root).unwrap();
        assert_eq!(loaded.name, "DES-spec");
        assert!(loaded.body.is_none());
        let facts = loaded.blob.unwrap();
        assert_eq!(facts.byte_size, 13);
        assert_eq!(facts.media_type, "application/pdf");
        // sha256 of the exact byte string written above; fixed so
        // regressions on the hasher surface loudly.
        assert_eq!(facts.content_hash.len(), 64);
    }

    #[test]
    fn rejects_sidecar_whose_shape_is_not_blob() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let binary_path = root.join("DES-spec.pdf");
        fs::write(&binary_path, b"%PDF-1.4").unwrap();
        let sidecar_path = crate::schema::sidecar::sidecar_path_for_blob(&binary_path);
        let mut metadata = blob_metadata("DES-spec.pdf");
        metadata.shape = ArtifactShape::Content;
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        let err = load_blob_artifact(&sidecar_path, root).unwrap_err();
        assert!(matches!(err, BlobLoadError::ShapeMismatch { .. }));
    }

    #[test]
    fn rejects_missing_binary_peer() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let sidecar_path = root.join("DES-spec.pdf.reqforge.json");
        let metadata = blob_metadata("DES-spec.pdf");
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        let err = load_blob_artifact(&sidecar_path, root).unwrap_err();
        assert!(matches!(err, BlobLoadError::BinaryMissing { .. }));
    }

    #[test]
    fn rejects_blob_path_that_escapes_project_root() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let sidecar_path = root.join("evil.pdf.reqforge.json");
        let metadata = blob_metadata("../outside.pdf");
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        let err = load_blob_artifact(&sidecar_path, root).unwrap_err();
        assert!(matches!(err, BlobLoadError::BlobPathEscapesRoot { .. }));
    }

    #[test]
    fn rejects_blob_path_that_is_absolute() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let sidecar_path = root.join("abs.pdf.reqforge.json");
        let metadata = blob_metadata("/etc/passwd");
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        let err = load_blob_artifact(&sidecar_path, root).unwrap_err();
        assert!(matches!(err, BlobLoadError::BlobPathAbsolute { .. }));
    }

    #[test]
    fn rejects_extension_not_in_allowlist() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let binary_path = root.join("payload.exe");
        fs::write(&binary_path, b"MZ").unwrap();
        let sidecar_path = crate::schema::sidecar::sidecar_path_for_blob(&binary_path);
        let metadata = blob_metadata("payload.exe");
        fs::write(&sidecar_path, serde_json::to_vec_pretty(&metadata).unwrap()).unwrap();
        let err = load_blob_artifact(&sidecar_path, root).unwrap_err();
        assert!(matches!(err, BlobLoadError::ExtensionNotAllowed { .. }));
    }
}
