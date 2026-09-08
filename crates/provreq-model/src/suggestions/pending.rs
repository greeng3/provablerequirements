//! Persistence of pending link suggestions.
//!
//! On-disk path: `<project_root>/artifacts/.suggestions/pending.json`.
//! Atomic-written via [`crate::write::atomic_write`]. Lets an
//! analysis run produce a queue the operator works through over
//! multiple sessions — proposals don't evaporate when the
//! container restarts.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::errors::SuggestionError;
use super::types::Suggestion;
use crate::write::atomic_write;

/// Current on-disk schema version for `pending.json`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

const SIDECAR_RELATIVE_PATH: &str = "artifacts/.suggestions/pending.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingSidecar {
    schema_version: u32,
    #[serde(default)]
    suggestions: Vec<Suggestion>,
}

/// Resolve the absolute path to the sidecar for a given project
/// root.
pub fn sidecar_path(project_root: &Path) -> PathBuf {
    project_root.join(SIDECAR_RELATIVE_PATH)
}

/// Load the pending-suggestions list. Returns an empty vec if
/// the file does not exist.
pub fn load(project_root: &Path) -> Result<Vec<Suggestion>, SuggestionError> {
    let path = sidecar_path(project_root);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let sidecar: PendingSidecar = serde_json::from_slice(&bytes)?;
    if sidecar.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(SuggestionError::SchemaTooNew {
            found: sidecar.schema_version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    Ok(sidecar.suggestions)
}

/// Replace the pending-suggestions list on disk.
pub fn save(project_root: &Path, suggestions: &[Suggestion]) -> Result<(), SuggestionError> {
    let path = sidecar_path(project_root);
    let sidecar = PendingSidecar {
        schema_version: CURRENT_SCHEMA_VERSION,
        suggestions: suggestions.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&sidecar)?;
    atomic_write(&path, &bytes)?;
    Ok(())
}

/// Remove the suggestion whose `id` matches and return it.
/// Returns `None` if no match.
pub fn remove(project_root: &Path, id: Uuid) -> Result<Option<Suggestion>, SuggestionError> {
    let mut suggestions = load(project_root)?;
    let Some(pos) = suggestions.iter().position(|s| s.id == id) else {
        return Ok(None);
    };
    let removed = suggestions.remove(pos);
    save(project_root, &suggestions)?;
    Ok(Some(removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_suggestion(id_byte: u8, from_byte: u8, to_byte: u8, link_type: &str) -> Suggestion {
        Suggestion {
            id: Uuid::from_bytes([
                0x01, 0x94, 0xf6, 0xd0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, id_byte,
            ]),
            from: Uuid::from_bytes([
                0x01, 0x94, 0xf6, 0xd0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, from_byte,
            ]),
            to: Uuid::from_bytes([
                0x01, 0x94, 0xf6, 0xd0, 0, 0, 0x70, 0, 0x80, 0, 0, 0, 0, 0, 0, to_byte,
            ]),
            link_type: link_type.to_owned(),
            confidence: 0.85,
            rationale: "test".to_owned(),
        }
    }

    #[test]
    fn load_returns_empty_when_sidecar_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let suggestions = load(temp.path()).unwrap();
        assert!(suggestions.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let suggestions = vec![
            make_suggestion(1, 10, 20, "derives-from"),
            make_suggestion(2, 11, 21, "satisfies"),
        ];
        save(temp.path(), &suggestions).unwrap();
        let loaded = load(temp.path()).unwrap();
        assert_eq!(loaded, suggestions);
    }

    #[test]
    fn save_creates_parent_directory_tree() {
        let temp = tempfile::tempdir().unwrap();
        save(temp.path(), &[make_suggestion(1, 10, 20, "derives-from")]).unwrap();
        assert!(
            temp.path()
                .join("artifacts/.suggestions/pending.json")
                .exists()
        );
    }

    #[test]
    fn remove_pops_by_id_and_returns_the_suggestion() {
        let temp = tempfile::tempdir().unwrap();
        let s1 = make_suggestion(1, 10, 20, "derives-from");
        let s2 = make_suggestion(2, 11, 21, "satisfies");
        save(temp.path(), &[s1.clone(), s2.clone()]).unwrap();

        let removed = remove(temp.path(), s1.id).unwrap();
        assert_eq!(removed, Some(s1));
        let remaining = load(temp.path()).unwrap();
        assert_eq!(remaining, vec![s2]);
    }

    #[test]
    fn remove_returns_none_for_unknown_id() {
        let temp = tempfile::tempdir().unwrap();
        save(temp.path(), &[make_suggestion(1, 10, 20, "derives-from")]).unwrap();
        let unknown = Uuid::from_bytes([0xff; 16]);
        assert_eq!(remove(temp.path(), unknown).unwrap(), None);
    }

    #[test]
    fn load_refuses_a_newer_schema_version() {
        let temp = tempfile::tempdir().unwrap();
        let path = sidecar_path(temp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, br#"{"schemaVersion":99,"suggestions":[]}"#).unwrap();
        let err = load(temp.path()).unwrap_err();
        assert!(matches!(
            err,
            SuggestionError::SchemaTooNew {
                found: 99,
                supported: 1
            }
        ));
    }
}
