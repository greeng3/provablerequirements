//! Per-file-type migration registry + the core engine.
//!
//! A `Registry` is a `Vec<MigrationStep>` plus the "current
//! version" constant. `Registry::migrate` walks the chain
//! starting at the file's found version; it's the engine the
//! top-level `migrate_value` delegates to.

use std::fmt;

use serde::Serialize;
use serde_json::Value;

use super::errors::SchemaMigrationError;

/// One step in a migration chain. Takes the raw value at
/// version `from_version` and returns a value at
/// `from_version + 1`. The engine calls these in ascending
/// order starting at whatever the file's declared version was.
///
/// Steps are allowed to be fallible — if the file on disk has
/// drifted into a shape a step can't handle, it returns an error
/// string the engine wraps into `SchemaMigrationError::StepFailed`.
pub type MigrationStep = fn(Value) -> Result<Value, String>;

/// Outcome snapshot returned to callers. Tells them whether a
/// rewrite is warranted (migrated) and from/to versions for the
/// bulk-migrate per-file result payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationOutcome {
    pub file_type: FileType,
    pub from_version: u32,
    pub to_version: u32,
    /// `true` iff at least one step ran. A file already at the
    /// current version returns `false` so the bulk-migrate walker
    /// can skip the rewrite.
    pub migrated: bool,
}

/// One of the four on-disk file shapes. The enum's `Display` impl
/// is used directly in error messages, so keep the wire name
/// natural.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileType {
    Artifact,
    Collection,
    Project,
    System,
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FileType::Artifact => "artifact",
            FileType::Collection => "collection",
            FileType::Project => "project",
            FileType::System => "system",
        })
    }
}

/// A migration chain for one file type. `current_version` is the
/// version the chain migrates files up to; `steps[i]` migrates
/// `v == base_version + i` to `base_version + i + 1`.
///
/// `base_version` is `1` everywhere today (the format began at
/// v=1). The field exists so a future pruning of ancient
/// migrations — say when we drop v=1 support — doesn't require
/// re-indexing the `steps` vector.
pub struct Registry {
    file_type: FileType,
    base_version: u32,
    current_version: u32,
    steps: Vec<MigrationStep>,
}

impl Registry {
    /// Build a registry whose chain migrates `base_version` →
    /// `base_version + steps.len()`. `current_version` is set to
    /// `base_version + steps.len()` — so an empty chain means
    /// "current version is the same as base version".
    pub fn new(file_type: FileType, base_version: u32, steps: Vec<MigrationStep>) -> Self {
        let current_version = base_version + u32::try_from(steps.len()).unwrap_or(u32::MAX);
        Self {
            file_type,
            base_version,
            current_version,
            steps,
        }
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn current_version(&self) -> u32 {
        self.current_version
    }

    /// Walk the chain from the file's declared version up to
    /// `current_version`.
    pub fn migrate(&self, value: Value) -> Result<(Value, MigrationOutcome), SchemaMigrationError> {
        let found = read_schema_version(&value, self.file_type)?;
        if found > self.current_version {
            return Err(SchemaMigrationError::NewerThanCurrent {
                file_type: self.file_type,
                found,
                current: self.current_version,
            });
        }
        if found < self.base_version {
            // Treat too-old versions as "we no longer know how to
            // migrate this". For 11a this branch is unreachable
            // (base == 1 == earliest), but the guard matters as
            // soon as we start pruning old chains.
            return Err(SchemaMigrationError::StepFailed {
                file_type: self.file_type,
                from_version: found,
                to_version: self.base_version,
                detail: "declared version predates this build's oldest supported schema".into(),
            });
        }

        let start_offset = (found - self.base_version) as usize;
        let mut current_value = value;
        let mut current_version = found;
        for step in &self.steps[start_offset..] {
            let next_version = current_version + 1;
            current_value =
                step(current_value).map_err(|detail| SchemaMigrationError::StepFailed {
                    file_type: self.file_type,
                    from_version: current_version,
                    to_version: next_version,
                    detail,
                })?;
            // Each step is responsible for stamping the new
            // version onto the value; we double-check so a
            // buggy migration can't silently produce a value
            // with the wrong schemaVersion.
            stamp_schema_version(&mut current_value, next_version);
            current_version = next_version;
        }

        Ok((
            current_value,
            MigrationOutcome {
                file_type: self.file_type,
                from_version: found,
                to_version: self.current_version,
                migrated: found < self.current_version,
            },
        ))
    }
}

/// Read the `schemaVersion` field as a `u32`. Errors with
/// `InvalidSchemaVersion` if missing / wrong type / out of range.
fn read_schema_version(value: &Value, file_type: FileType) -> Result<u32, SchemaMigrationError> {
    let raw =
        value
            .get("schemaVersion")
            .ok_or_else(|| SchemaMigrationError::InvalidSchemaVersion {
                file_type,
                found: "missing field".into(),
            })?;
    let n = raw
        .as_u64()
        .ok_or_else(|| SchemaMigrationError::InvalidSchemaVersion {
            file_type,
            found: truncate(&raw.to_string(), 40),
        })?;
    if n == 0 || n > u64::from(u32::MAX) {
        return Err(SchemaMigrationError::InvalidSchemaVersion {
            file_type,
            found: truncate(&raw.to_string(), 40),
        });
    }
    Ok(n as u32)
}

/// Overwrite `schemaVersion` on the given value with `new_version`.
/// No-op if the value isn't a JSON object (already an error path,
/// but the `migrate_value` contract guarantees the top level is
/// an object — defensive either way).
fn stamp_schema_version(value: &mut Value, new_version: u32) {
    if let Value::Object(map) = value {
        map.insert(
            "schemaVersion".into(),
            Value::Number(serde_json::Number::from(new_version)),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// No-op migration that stamps the version forward and adds
    /// a marker field so tests can assert the step ran.
    fn step_add_marker(mut v: Value) -> Result<Value, String> {
        if let Value::Object(map) = &mut v {
            map.insert("marker_v2".into(), json!("ran"));
        }
        Ok(v)
    }

    fn step_add_marker_v3(mut v: Value) -> Result<Value, String> {
        if let Value::Object(map) = &mut v {
            map.insert("marker_v3".into(), json!("ran"));
        }
        Ok(v)
    }

    fn step_fails(_v: Value) -> Result<Value, String> {
        Err("deliberate test failure".into())
    }

    #[test]
    fn empty_chain_is_a_no_op_for_files_at_current_version() {
        let reg = Registry::new(FileType::Artifact, 1, Vec::new());
        let value = json!({ "schemaVersion": 1, "n": "hello" });
        let (out, outcome) = reg.migrate(value.clone()).unwrap();
        assert_eq!(out, value);
        assert_eq!(outcome.from_version, 1);
        assert_eq!(outcome.to_version, 1);
        assert!(!outcome.migrated);
    }

    #[test]
    fn empty_chain_refuses_files_at_higher_version() {
        let reg = Registry::new(FileType::Artifact, 1, Vec::new());
        let value = json!({ "schemaVersion": 2 });
        let err = reg.migrate(value).unwrap_err();
        assert_eq!(
            err,
            SchemaMigrationError::NewerThanCurrent {
                file_type: FileType::Artifact,
                found: 2,
                current: 1,
            }
        );
    }

    #[test]
    fn single_step_walks_and_stamps_new_version() {
        let reg = Registry::new(FileType::Artifact, 1, vec![step_add_marker]);
        let value = json!({ "schemaVersion": 1, "n": "hello" });
        let (out, outcome) = reg.migrate(value).unwrap();
        assert_eq!(out["schemaVersion"], 2);
        assert_eq!(out["marker_v2"], "ran");
        assert_eq!(outcome.from_version, 1);
        assert_eq!(outcome.to_version, 2);
        assert!(outcome.migrated);
    }

    #[test]
    fn multi_step_applies_each_step_in_order() {
        let reg = Registry::new(
            FileType::Artifact,
            1,
            vec![step_add_marker, step_add_marker_v3],
        );
        let value = json!({ "schemaVersion": 1 });
        let (out, outcome) = reg.migrate(value).unwrap();
        assert_eq!(out["schemaVersion"], 3);
        assert_eq!(out["marker_v2"], "ran");
        assert_eq!(out["marker_v3"], "ran");
        assert_eq!(outcome.from_version, 1);
        assert_eq!(outcome.to_version, 3);
    }

    #[test]
    fn migration_skips_already_applied_steps() {
        let reg = Registry::new(
            FileType::Artifact,
            1,
            vec![step_add_marker, step_add_marker_v3],
        );
        // File is already at v2 — the v1→v2 step must NOT run,
        // only the v2→v3 step.
        let value = json!({ "schemaVersion": 2, "marker_v2": "ran" });
        let (out, outcome) = reg.migrate(value).unwrap();
        assert_eq!(out["schemaVersion"], 3);
        assert_eq!(out["marker_v3"], "ran");
        assert_eq!(outcome.from_version, 2);
    }

    #[test]
    fn failing_step_propagates_with_from_to_detail() {
        let reg = Registry::new(FileType::Collection, 1, vec![step_fails]);
        let value = json!({ "schemaVersion": 1 });
        let err = reg.migrate(value).unwrap_err();
        match err {
            SchemaMigrationError::StepFailed {
                file_type,
                from_version,
                to_version,
                detail,
            } => {
                assert_eq!(file_type, FileType::Collection);
                assert_eq!(from_version, 1);
                assert_eq!(to_version, 2);
                assert_eq!(detail, "deliberate test failure");
            }
            other => panic!("expected StepFailed, got {other:?}"),
        }
    }

    #[test]
    fn missing_schema_version_errors_cleanly() {
        let reg = Registry::new(FileType::Project, 1, Vec::new());
        let value = json!({ "n": "no version field" });
        let err = reg.migrate(value).unwrap_err();
        assert!(matches!(
            err,
            SchemaMigrationError::InvalidSchemaVersion {
                file_type: FileType::Project,
                ..
            }
        ));
    }

    #[test]
    fn wrong_type_schema_version_errors_cleanly() {
        let reg = Registry::new(FileType::System, 1, Vec::new());
        let value = json!({ "schemaVersion": "one" });
        let err = reg.migrate(value).unwrap_err();
        match err {
            SchemaMigrationError::InvalidSchemaVersion { file_type, found } => {
                assert_eq!(file_type, FileType::System);
                assert!(found.contains("one"));
            }
            other => panic!("expected InvalidSchemaVersion, got {other:?}"),
        }
    }

    #[test]
    fn zero_schema_version_is_invalid() {
        let reg = Registry::new(FileType::Artifact, 1, Vec::new());
        let value = json!({ "schemaVersion": 0 });
        let err = reg.migrate(value).unwrap_err();
        assert!(matches!(
            err,
            SchemaMigrationError::InvalidSchemaVersion { .. }
        ));
    }

    #[test]
    fn file_type_display_matches_wire_casing() {
        assert_eq!(FileType::Artifact.to_string(), "artifact");
        assert_eq!(FileType::Collection.to_string(), "collection");
        assert_eq!(FileType::Project.to_string(), "project");
        assert_eq!(FileType::System.to_string(), "system");
    }
}
