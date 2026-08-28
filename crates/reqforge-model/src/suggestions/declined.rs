//! Persistence of declined link suggestions.
//!
//! On-disk path: `<project_root>/artifacts/.suggestions/declined.json`.
//! Atomic-written via [`crate::write::atomic_write`]. Conceptual
//! key for dedup is the triple `(from, to, link_type)` — a
//! re-run of analysis filters proposals against this key.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::errors::SuggestionError;
use super::types::DeclineRecord;
use crate::write::atomic_write;

/// Current on-disk schema version for `declined.json`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

const SIDECAR_RELATIVE_PATH: &str = "artifacts/.suggestions/declined.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeclinedSidecar {
    schema_version: u32,
    #[serde(default)]
    declined: Vec<DeclineRecord>,
}

/// Resolve the absolute path to the sidecar for a given project
/// root. Public so callers can pre-test existence in places
/// where they don't want the empty-vec fallback semantics.
pub fn sidecar_path(project_root: &Path) -> PathBuf {
    project_root.join(SIDECAR_RELATIVE_PATH)
}

/// Load the declined-suggestions list. Returns an empty vec if
/// the file does not exist (the common case before the operator
/// has rejected anything).
pub fn load(project_root: &Path) -> Result<Vec<DeclineRecord>, SuggestionError> {
    let path = sidecar_path(project_root);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let sidecar: DeclinedSidecar = serde_json::from_slice(&bytes)?;
    if sidecar.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(SuggestionError::SchemaTooNew {
            found: sidecar.schema_version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    Ok(sidecar.declined)
}

/// Replace the declined-suggestions list on disk. Creates the
/// parent directory tree if needed.
pub fn save(project_root: &Path, records: &[DeclineRecord]) -> Result<(), SuggestionError> {
    let path = sidecar_path(project_root);
    let sidecar = DeclinedSidecar {
        schema_version: CURRENT_SCHEMA_VERSION,
        declined: records.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&sidecar)?;
    atomic_write(&path, &bytes)?;
    Ok(())
}

/// Append a single decline record (load + push + save). Idempotent
/// on `id` — calling twice with the same id leaves the on-disk
/// list with a single entry.
pub fn append(project_root: &Path, record: DeclineRecord) -> Result<(), SuggestionError> {
    let mut records = load(project_root)?;
    if !records
        .iter()
        .any(|r| r.suggestion.id == record.suggestion.id)
    {
        records.push(record);
    }
    save(project_root, &records)
}

/// Remove the entry whose suggestion `id` matches and return it,
/// for the reinstate flow. Returns `None` if no match.
pub fn remove(project_root: &Path, id: Uuid) -> Result<Option<DeclineRecord>, SuggestionError> {
    let mut records = load(project_root)?;
    let Some(pos) = records.iter().position(|r| r.suggestion.id == id) else {
        return Ok(None);
    };
    let removed = records.remove(pos);
    save(project_root, &records)?;
    Ok(Some(removed))
}

/// Membership test by the conceptual `(from, to, link_type)` key.
/// Used to filter re-run proposals against the declined list.
pub fn is_declined(records: &[DeclineRecord], from: Uuid, to: Uuid, link_type: &str) -> bool {
    records.iter().any(|r| {
        r.suggestion.from == from && r.suggestion.to == to && r.suggestion.link_type == link_type
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suggestions::types::Suggestion;
    use chrono::{TimeZone, Utc};

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

    fn make_record(id_byte: u8, from_byte: u8, to_byte: u8, link_type: &str) -> DeclineRecord {
        DeclineRecord {
            suggestion: make_suggestion(id_byte, from_byte, to_byte, link_type),
            declined_at: Utc.with_ymd_and_hms(2026, 5, 4, 12, 0, 0).unwrap(),
        }
    }

    #[test]
    fn load_returns_empty_when_sidecar_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let records = load(temp.path()).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let records = vec![
            make_record(1, 10, 20, "derives-from"),
            make_record(2, 11, 21, "satisfies"),
        ];
        save(temp.path(), &records).unwrap();
        let loaded = load(temp.path()).unwrap();
        assert_eq!(loaded, records);
    }

    #[test]
    fn save_creates_parent_directory_tree() {
        let temp = tempfile::tempdir().unwrap();
        save(temp.path(), &[make_record(1, 10, 20, "derives-from")]).unwrap();
        assert!(
            temp.path()
                .join("artifacts/.suggestions/declined.json")
                .exists()
        );
    }

    #[test]
    fn append_adds_a_new_record() {
        let temp = tempfile::tempdir().unwrap();
        append(temp.path(), make_record(1, 10, 20, "derives-from")).unwrap();
        append(temp.path(), make_record(2, 11, 21, "satisfies")).unwrap();
        let loaded = load(temp.path()).unwrap();
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn append_is_idempotent_on_id() {
        let temp = tempfile::tempdir().unwrap();
        let record = make_record(1, 10, 20, "derives-from");
        append(temp.path(), record.clone()).unwrap();
        append(temp.path(), record).unwrap();
        let loaded = load(temp.path()).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn remove_pops_by_id_and_returns_the_record() {
        let temp = tempfile::tempdir().unwrap();
        let r1 = make_record(1, 10, 20, "derives-from");
        let r2 = make_record(2, 11, 21, "satisfies");
        save(temp.path(), &[r1.clone(), r2.clone()]).unwrap();

        let removed = remove(temp.path(), r1.suggestion.id).unwrap();
        assert_eq!(removed, Some(r1));
        let remaining = load(temp.path()).unwrap();
        assert_eq!(remaining, vec![r2]);
    }

    #[test]
    fn remove_returns_none_for_unknown_id() {
        let temp = tempfile::tempdir().unwrap();
        save(temp.path(), &[make_record(1, 10, 20, "derives-from")]).unwrap();
        let unknown = Uuid::from_bytes([0xff; 16]);
        assert_eq!(remove(temp.path(), unknown).unwrap(), None);
    }

    #[test]
    fn is_declined_matches_on_from_to_link_type_triple() {
        let records = vec![make_record(1, 10, 20, "derives-from")];
        let from = make_suggestion(1, 10, 20, "derives-from").from;
        let to = make_suggestion(1, 10, 20, "derives-from").to;
        assert!(is_declined(&records, from, to, "derives-from"));
        // Same endpoints, different link type → not declined.
        assert!(!is_declined(&records, from, to, "satisfies"));
        // Different from → not declined.
        let other_from = make_suggestion(99, 99, 20, "derives-from").from;
        assert!(!is_declined(&records, other_from, to, "derives-from"));
    }

    #[test]
    fn load_refuses_a_newer_schema_version() {
        let temp = tempfile::tempdir().unwrap();
        let path = sidecar_path(temp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, br#"{"schemaVersion":99,"declined":[]}"#).unwrap();
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
