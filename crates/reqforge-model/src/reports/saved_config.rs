//! Per-report saved-config persistence (Phase 6a).
//!
//! Each report kind stores its last-used scope, options, and
//! inactive-filter state in a small JSON blob under
//! `<workspace>/report-configs/<kind>.json`. Reports use this
//! file to hydrate their defaults on navigation, so flipping
//! between reports doesn't reset the scope every time.
//!
//! Writes go through [`crate::write::atomic_write`] and so land
//! as mode 0644 (Phase 5d bug fix), which matters for the same
//! cross-user-readability reasons outlined there.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::reports::ReportKind;
use crate::write::atomic_write;

/// Opaque JSON value per report kind — the server persists
/// whatever the frontend posts and serves it back verbatim. The
/// frontend owns the shape; the backend doesn't interpret it
/// beyond basic JSON validation on the way in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedReportConfig {
    /// Free-form JSON object containing the report's persisted
    /// state. Kept opaque so the frontend can evolve the shape
    /// without a backend change.
    #[serde(flatten)]
    pub inner: serde_json::Value,
}

impl SavedReportConfig {
    pub fn empty() -> Self {
        Self {
            inner: serde_json::json!({}),
        }
    }

    pub fn from_value(value: serde_json::Value) -> Self {
        Self { inner: value }
    }
}

/// Resolve the on-disk path for a report kind's saved config.
/// Returns `None` when no workspace directory is configured —
/// callers then skip persistence and treat every view as fresh.
pub fn config_path_for(workspace_dir: Option<&Path>, kind: ReportKind) -> Option<PathBuf> {
    let workspace = workspace_dir?;
    Some(
        workspace
            .join("report-configs")
            .join(format!("{}.json", kind.as_kebab())),
    )
}

/// Read a report's saved config from disk. Returns
/// `Ok(SavedReportConfig::empty())` on file-missing or parse
/// error — the report renders its built-in defaults and the
/// next save overwrites whatever's there.
pub fn load(workspace_dir: Option<&Path>, kind: ReportKind) -> SavedReportConfig {
    let Some(path) = config_path_for(workspace_dir, kind) else {
        return SavedReportConfig::empty();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "saved report config is malformed; falling back to defaults"
            );
            SavedReportConfig::empty()
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => SavedReportConfig::empty(),
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "could not read saved report config"
            );
            SavedReportConfig::empty()
        }
    }
}

/// Persist a report's saved config. Errors when the workspace
/// dir is unset (the handler should 409 or similar) or when the
/// atomic write itself fails.
pub fn save(
    workspace_dir: Option<&Path>,
    kind: ReportKind,
    config: &SavedReportConfig,
) -> Result<(), SavedConfigError> {
    let path = config_path_for(workspace_dir, kind).ok_or(SavedConfigError::NoWorkspace)?;
    let bytes = serde_json::to_vec_pretty(&config.inner).map_err(SavedConfigError::Serialize)?;
    atomic_write(&path, &bytes).map_err(SavedConfigError::Write)?;
    Ok(())
}

/// Delete a report's saved config — the "reset to defaults"
/// button on the report header. Missing file is OK.
pub fn clear(workspace_dir: Option<&Path>, kind: ReportKind) -> Result<(), SavedConfigError> {
    let Some(path) = config_path_for(workspace_dir, kind) else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SavedConfigError::Delete { path, source }),
    }
}

#[derive(Debug, Error)]
pub enum SavedConfigError {
    #[error("workspace directory is not configured — saved report configs are disabled")]
    NoWorkspace,

    #[error("failed to serialise saved config: {0}")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to atomic-write saved config: {0}")]
    Write(#[source] crate::write::AtomicWriteError),

    #[error("failed to delete saved config at {}: {source}", path.display())]
    Delete {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn config_path_is_kebab_case_under_report_configs() {
        let ws = PathBuf::from("/workspace/.reqforge-workspace");
        let path = config_path_for(Some(&ws), ReportKind::CoverageMatrix).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/workspace/.reqforge-workspace/report-configs/coverage-matrix.json")
        );
    }

    #[test]
    fn config_path_is_none_without_workspace() {
        assert!(config_path_for(None, ReportKind::UnresolvedLinks).is_none());
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let temp = tempdir().unwrap();
        let got = load(Some(temp.path()), ReportKind::UnresolvedLinks);
        assert_eq!(got.inner, serde_json::json!({}));
    }

    #[test]
    fn save_then_load_round_trips() {
        let temp = tempdir().unwrap();
        let cfg = SavedReportConfig::from_value(serde_json::json!({
            "scope": "project:sample",
            "includeInactive": true
        }));
        save(Some(temp.path()), ReportKind::UnresolvedLinks, &cfg).unwrap();
        let got = load(Some(temp.path()), ReportKind::UnresolvedLinks);
        assert_eq!(got.inner, cfg.inner);
    }

    #[test]
    fn malformed_config_on_disk_falls_back_to_empty_defaults() {
        let temp = tempdir().unwrap();
        let dir = temp.path().join("report-configs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("unresolved-links.json"), "{not json").unwrap();
        let got = load(Some(temp.path()), ReportKind::UnresolvedLinks);
        assert_eq!(got.inner, serde_json::json!({}));
    }

    #[test]
    fn clear_removes_existing_config_and_tolerates_missing() {
        let temp = tempdir().unwrap();
        let cfg = SavedReportConfig::from_value(serde_json::json!({ "x": 1 }));
        save(Some(temp.path()), ReportKind::Cycles, &cfg).unwrap();
        clear(Some(temp.path()), ReportKind::Cycles).unwrap();
        clear(Some(temp.path()), ReportKind::Cycles).unwrap(); // idempotent
        let got = load(Some(temp.path()), ReportKind::Cycles);
        assert_eq!(got.inner, serde_json::json!({}));
    }

    #[test]
    fn save_without_workspace_errors() {
        let cfg = SavedReportConfig::empty();
        let err = save(None, ReportKind::Cycles, &cfg).unwrap_err();
        assert!(matches!(err, SavedConfigError::NoWorkspace));
    }
}
