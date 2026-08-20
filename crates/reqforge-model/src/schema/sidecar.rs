//! Path helpers for blob and URL artifact sidecars, per
//! `FORMAT-blobSidecar` (flat-sibling layout locked in during the
//! Phase 5 planning round).
//!
//! Conventions:
//!
//! - A blob artifact `DES-spec.pdf` sits next to a sidecar
//!   `DES-spec.pdf.reqforge.json`. The binary keeps its original
//!   filename (including extension) so operators and git diff see
//!   the familiar name; the sidecar stacks `.reqforge.json` on top.
//! - A URL artifact has no binary peer; it's a single
//!   `<name>.reqforge.json` that carries shape / URL / check state.
//! - Filename stems derived from sidecars strip the full
//!   `.reqforge.json` suffix — a blob's name is `DES-spec`, not
//!   `DES-spec.pdf`, so collisions between a content-hosted `.md`
//!   and a blob are caught at discovery time.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The fixed sidecar suffix. Using a static keeps collision with
/// ordinary `.json` files out of scope.
pub const SIDECAR_SUFFIX: &str = ".reqforge.json";

/// Return the blob-sidecar path for a binary at `blob_path`:
/// `/foo/DES-spec.pdf` → `/foo/DES-spec.pdf.reqforge.json`.
pub fn sidecar_path_for_blob(blob_path: &Path) -> PathBuf {
    let mut name = blob_path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_default();
    name.push(SIDECAR_SUFFIX);
    blob_path.with_file_name(name)
}

/// Inverse of [`sidecar_path_for_blob`]. Returns `None` when the
/// sidecar name doesn't end in `.reqforge.json` or is a URL
/// artifact (name with only the sidecar suffix, no inner extension).
pub fn blob_path_for_sidecar(sidecar_path: &Path) -> Option<PathBuf> {
    let name = sidecar_path.file_name().and_then(OsStr::to_str)?;
    let stem = name.strip_suffix(SIDECAR_SUFFIX)?;
    // A stem with no '.' is a URL artifact — `RFC-9110.reqforge.json`
    // → stem is `RFC-9110`, which has no binary peer. Return None so
    // the caller doesn't go hunting for a missing file.
    if !stem.contains('.') {
        return None;
    }
    Some(sidecar_path.with_file_name(stem))
}

/// Sidecar filename for a URL artifact named `stem`. Keeps the URL
/// case distinct from the blob case — a URL artifact's sidecar
/// stem has no inner extension.
pub fn url_sidecar_filename(stem: &str) -> String {
    format!("{stem}{SIDECAR_SUFFIX}")
}

/// Artifact name stem from a sidecar path. For blobs this strips
/// both suffixes (`DES-spec.pdf.reqforge.json` → `DES-spec`); for
/// URL artifacts this strips just the sidecar suffix
/// (`RFC-9110.reqforge.json` → `RFC-9110`).
pub fn artifact_name_from_sidecar(sidecar_path: &Path) -> Option<String> {
    let name = sidecar_path.file_name().and_then(OsStr::to_str)?;
    let without_sidecar = name.strip_suffix(SIDECAR_SUFFIX)?;
    // For blob sidecars the inner extension is the file's real
    // extension; strip it so the artifact name doesn't include it.
    match without_sidecar.rsplit_once('.') {
        Some((stem, _ext)) => Some(stem.to_owned()),
        None => Some(without_sidecar.to_owned()),
    }
}

/// Is this a sidecar path (ends in `.reqforge.json`)?
pub fn is_sidecar_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.ends_with(SIDECAR_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_path_appends_suffix_to_blob_filename() {
        let blob = PathBuf::from("/foo/DES-spec.pdf");
        assert_eq!(
            sidecar_path_for_blob(&blob),
            PathBuf::from("/foo/DES-spec.pdf.reqforge.json"),
        );
    }

    #[test]
    fn blob_path_from_sidecar_is_the_inverse() {
        let sidecar = PathBuf::from("/foo/DES-spec.pdf.reqforge.json");
        assert_eq!(
            blob_path_for_sidecar(&sidecar),
            Some(PathBuf::from("/foo/DES-spec.pdf")),
        );
    }

    #[test]
    fn blob_path_from_url_sidecar_is_none() {
        // URL artifact: no binary peer.
        let sidecar = PathBuf::from("/foo/RFC-9110.reqforge.json");
        assert!(blob_path_for_sidecar(&sidecar).is_none());
    }

    #[test]
    fn blob_path_rejects_non_sidecar_name() {
        let not_sidecar = PathBuf::from("/foo/DES-spec.pdf");
        assert!(blob_path_for_sidecar(&not_sidecar).is_none());
    }

    #[test]
    fn artifact_name_drops_inner_extension_for_blob_sidecar() {
        let sidecar = PathBuf::from("/foo/DES-spec.pdf.reqforge.json");
        assert_eq!(
            artifact_name_from_sidecar(&sidecar).as_deref(),
            Some("DES-spec"),
        );
    }

    #[test]
    fn artifact_name_for_url_sidecar_has_no_inner_extension() {
        let sidecar = PathBuf::from("/foo/RFC-9110.reqforge.json");
        assert_eq!(
            artifact_name_from_sidecar(&sidecar).as_deref(),
            Some("RFC-9110"),
        );
    }

    #[test]
    fn url_sidecar_filename_uses_suffix() {
        assert_eq!(url_sidecar_filename("RFC-9110"), "RFC-9110.reqforge.json");
    }

    #[test]
    fn is_sidecar_path_detects_suffix() {
        assert!(is_sidecar_path(Path::new("x.reqforge.json")));
        assert!(is_sidecar_path(Path::new("x.pdf.reqforge.json")));
        assert!(!is_sidecar_path(Path::new("x.json")));
        assert!(!is_sidecar_path(Path::new("x.md")));
    }
}
