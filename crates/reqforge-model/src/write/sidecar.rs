//! Shape-aware write helpers for Phase 5.
//!
//! Layered on top of `atomic_write` + `reconcile_ownership` the
//! same way `write_artifact_file` is — callers get atomic
//! temp-file-then-rename semantics (per `STOR-atomicWrites`) plus
//! chown-from-`.git` (per `DEPLOY-chownFromDotGit`) in both
//! helpers.
//!
//! - [`write_sidecar_only`] is for URL artifacts — a single
//!   `.reqforge.json` with no binary peer.
//! - [`write_blob_and_sidecar`] is for blob artifacts — the
//!   binary first, then the sidecar, each atomically written and
//!   ownership-reconciled against the repo's `.git` entry.
//!
//! Both helpers serialize the `Artifact` with the same pretty-
//! printed 2-space indent the content-hosted path uses, so the
//! on-disk format is uniform across shapes.

use std::path::Path;

use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};
use thiserror::Error;

use crate::schema::Artifact;
use crate::write::{
    AtomicWriteError, OwnershipError, OwnershipOverrides, atomic_write, reconcile_ownership,
};

/// Render the sidecar JSON bytes the on-disk file should carry.
/// Ends with a trailing newline so tools that append to files
/// don't produce `}}` boundaries.
pub fn render_sidecar_json(metadata: &Artifact) -> Result<Vec<u8>, ShapeWriteError> {
    let mut bytes = Vec::with_capacity(512);
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut ser = Serializer::with_formatter(&mut bytes, formatter);
    metadata
        .serialize(&mut ser)
        .map_err(ShapeWriteError::SerializeJson)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Atomically write the sidecar for a URL artifact.
pub fn write_sidecar_only(
    sidecar_path: &Path,
    repo_root: &Path,
    metadata: &Artifact,
    overrides: OwnershipOverrides,
) -> Result<(), ShapeWriteError> {
    let bytes = render_sidecar_json(metadata)?;
    atomic_write(sidecar_path, &bytes).map_err(ShapeWriteError::AtomicWrite)?;
    reconcile_ownership(sidecar_path, repo_root, overrides).map_err(ShapeWriteError::Ownership)?;
    Ok(())
}

/// Atomically write the binary, then atomically write the sidecar,
/// then chown both. Order matters: if the crash window were to
/// catch us between the two writes, a stray binary without a
/// sidecar is recoverable (discovery flags it as an
/// `OrphanBinary`); a stray sidecar without a binary would be
/// worse.
pub fn write_blob_and_sidecar(
    binary_target: &Path,
    binary_bytes: &[u8],
    sidecar_target: &Path,
    repo_root: &Path,
    metadata: &Artifact,
    overrides: OwnershipOverrides,
) -> Result<(), ShapeWriteError> {
    atomic_write(binary_target, binary_bytes).map_err(ShapeWriteError::AtomicWrite)?;
    let sidecar_bytes = render_sidecar_json(metadata)?;
    atomic_write(sidecar_target, &sidecar_bytes).map_err(ShapeWriteError::AtomicWrite)?;
    reconcile_ownership(binary_target, repo_root, overrides).map_err(ShapeWriteError::Ownership)?;
    reconcile_ownership(sidecar_target, repo_root, overrides)
        .map_err(ShapeWriteError::Ownership)?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum ShapeWriteError {
    #[error("serialising sidecar JSON: {0}")]
    SerializeJson(#[source] serde_json::Error),

    #[error(transparent)]
    AtomicWrite(#[from] AtomicWriteError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Artifact, ArtifactShape};
    use chrono::Utc;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn metadata(shape: ArtifactShape) -> Artifact {
        Artifact {
            schema_version: 1,
            uuid: Uuid::now_v7(),
            title: "Artifact".to_owned(),
            shape,
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
            url: None,
            checked_at: None,
            check_status: None,
            overflow: BTreeMap::new(),
        }
    }

    fn init_fake_repo(root: &Path) {
        // reconcile_ownership asserts on the repo's .git entry; it
        // doesn't care what's in it for the "already matches" path,
        // just that it exists, so an empty directory is enough.
        fs::create_dir_all(root.join(".git")).unwrap();
    }

    #[test]
    fn write_sidecar_only_round_trips_through_tempdir() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        init_fake_repo(repo);
        let mut meta = metadata(ArtifactShape::Url);
        meta.url = Some("https://example.com".to_owned());
        let target = repo.join("RFC-9110.reqforge.json");
        write_sidecar_only(&target, repo, &meta, OwnershipOverrides::default()).unwrap();
        let text = fs::read_to_string(&target).unwrap();
        assert!(text.contains("\"url\": \"https://example.com\""));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn write_blob_and_sidecar_writes_both_files() {
        let temp = tempdir().unwrap();
        let repo = temp.path();
        init_fake_repo(repo);
        let binary_target = repo.join("DES-spec.pdf");
        let sidecar_target = crate::schema::sidecar::sidecar_path_for_blob(&binary_target);
        let mut meta = metadata(ArtifactShape::Blob);
        meta.blob_path = Some("DES-spec.pdf".to_owned());
        write_blob_and_sidecar(
            &binary_target,
            b"%PDF-1.4 binary payload",
            &sidecar_target,
            repo,
            &meta,
            OwnershipOverrides::default(),
        )
        .unwrap();
        assert!(binary_target.is_file());
        assert!(sidecar_target.is_file());
        let sidecar_text = fs::read_to_string(&sidecar_target).unwrap();
        assert!(sidecar_text.contains("\"blobPath\": \"DES-spec.pdf\""));
    }
}
