//! System configuration loader.
//!
//! The System config is optional: when `REQFORGE_SYSTEM_CONFIG` is
//! unset (or points at a non-existent path), ReqForge runs in
//! "unnamed-system" mode and simply accepts whatever Projects are
//! mounted without cross-reference.
//!
//! When a System config is present, the loader reconciles the
//! expected project slugs against what was actually discovered and
//! reports missing mounts per `DEPLOY-systemConfigFile`.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::mount::{MountInfo, MountState};
use crate::schema::SystemConfig;
use crate::schema_migration::{FileType, SchemaMigrationError, migrate_value};

/// Outcome of loading the System config.
#[derive(Debug)]
pub enum LoadedSystem {
    /// No `REQFORGE_SYSTEM_CONFIG` provided, or the path doesn't
    /// exist. ReqForge runs without a named System.
    Unnamed,
    /// A valid System config was loaded.
    Named {
        config: Box<SystemConfig>,
        source_path: PathBuf,
    },
}

impl LoadedSystem {
    pub fn config(&self) -> Option<&SystemConfig> {
        match self {
            Self::Unnamed => None,
            Self::Named { config, .. } => Some(config.as_ref()),
        }
    }
}

#[derive(Debug, Error)]
pub enum SystemLoadError {
    #[error("i/o error reading system config {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid system config at {}: {source}", path.display())]
    Parse {
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

    #[error(
        "system config {} is world-readable (mode {mode:#o}); API keys live in this file — \
         run `chmod 0600 {}` and try again",
        path.display(),
        path.display()
    )]
    InsecureMode { path: PathBuf, mode: u32 },
}

/// Load a System config from `path`. Returns `Unnamed` when `path` is
/// `None` or the file doesn't exist; returns an error when the file
/// exists but is malformed or has insecure permissions.
pub fn load_system_config(path: Option<&Path>) -> Result<LoadedSystem, SystemLoadError> {
    let Some(path) = path else {
        return Ok(LoadedSystem::Unnamed);
    };
    if !path.exists() {
        return Ok(LoadedSystem::Unnamed);
    }
    check_file_mode(path)?;
    let text = fs::read_to_string(path).map_err(|source| SystemLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| SystemLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    let (migrated, _) =
        migrate_value(FileType::System, raw).map_err(|source| SystemLoadError::Schema {
            path: path.to_path_buf(),
            source,
        })?;
    let config: SystemConfig =
        serde_json::from_value(migrated).map_err(|source| SystemLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(LoadedSystem::Named {
        config: Box::new(config),
        source_path: path.to_path_buf(),
    })
}

/// On POSIX hosts: reject System config files whose mode has any
/// "other" or "group" read/write bits set. The file holds API
/// keys per LLM-secretsViaEnv (Phase 13); a stray permission can
/// leak them. Windows is a no-op — its security model lives in
/// ACLs the Rust standard library doesn't expose.
#[cfg(unix)]
fn check_file_mode(path: &Path) -> Result<(), SystemLoadError> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(path).map_err(|source| SystemLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mode = meta.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(SystemLoadError::InsecureMode {
            path: path.to_path_buf(),
            mode,
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_file_mode(_path: &Path) -> Result<(), SystemLoadError> {
    Ok(())
}

/// Failure modes for [`write_system_config`].
#[derive(Debug, Error)]
pub enum SystemWriteError {
    #[error("serialising system config: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("system config at {} is read-only — operators editing via /llm need a writable mount (drop the `:ro` flag)", path.display())]
    ReadOnly { path: PathBuf },

    #[error("write failed at {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: crate::write::AtomicWriteError,
    },
}

impl SystemWriteError {
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::ReadOnly { .. })
    }
}

/// Atomic-write a `SystemConfig` to disk at `path` with mode 0600
/// on POSIX. Operators editing providers via `/llm` reach this
/// path; the loader's [`check_file_mode`] insists on 0600 so the
/// writer must produce 0600 too.
pub fn write_system_config(path: &Path, config: &SystemConfig) -> Result<(), SystemWriteError> {
    let bytes = serde_json::to_vec_pretty(config)?;
    if let Err(source) = crate::write::atomic_write_with_mode(path, &bytes, 0o600) {
        if is_readonly_error(&source) {
            return Err(SystemWriteError::ReadOnly {
                path: path.to_path_buf(),
            });
        }
        return Err(SystemWriteError::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn is_readonly_error(err: &crate::write::AtomicWriteError) -> bool {
    use crate::write::AtomicWriteError;
    use std::io::ErrorKind;
    let inner = match err {
        AtomicWriteError::CreateTemp { source, .. }
        | AtomicWriteError::Write { source, .. }
        | AtomicWriteError::Fsync { source, .. }
        | AtomicWriteError::Rename { source, .. }
        | AtomicWriteError::EnsureParent { source, .. } => source,
        AtomicWriteError::NoParent { .. } => return false,
    };
    matches!(
        inner.kind(),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
    )
}

/// Project slugs declared in the System config that are not present
/// among the mounted Projects, per `DEPLOY-systemConfigFile`.
///
/// Returns an empty vector when the System is Unnamed (nothing to
/// reconcile against).
pub fn missing_project_slugs(system: &LoadedSystem, mounts: &[MountInfo]) -> Vec<String> {
    let Some(config) = system.config() else {
        return Vec::new();
    };
    let mounted_slugs: std::collections::HashSet<&str> = mounts
        .iter()
        .filter_map(|m| match &m.state {
            MountState::Project(p) => Some(p.config.slug.as_str()),
            _ => None,
        })
        .collect();
    config
        .projects
        .iter()
        .filter(|p| !mounted_slugs.contains(p.slug.as_str()))
        .map(|p| p.slug.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn unnamed_system_when_path_is_none() {
        let system = load_system_config(None).unwrap();
        assert!(matches!(system, LoadedSystem::Unnamed));
    }

    #[test]
    fn unnamed_system_when_path_missing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("does-not-exist.json");
        let system = load_system_config(Some(&path)).unwrap();
        assert!(matches!(system, LoadedSystem::Unnamed));
    }

    /// Test helper: write `bytes` and chmod 0600 on POSIX so the
    /// load path's secure-mode check passes. Windows: just write.
    fn write_secure(path: &Path, bytes: &str) {
        fs::write(path, bytes).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    #[test]
    fn loads_valid_system_config() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("system.json");
        write_secure(
            &path,
            &serde_json::json!({
                "schemaVersion": 1,
                "name": "Test System",
                "projects": [{ "slug": "a" }, { "slug": "b" }]
            })
            .to_string(),
        );

        let system = load_system_config(Some(&path)).unwrap();
        match system {
            LoadedSystem::Named {
                config,
                source_path,
            } => {
                assert_eq!(config.name, "Test System");
                assert_eq!(config.projects.len(), 2);
                assert_eq!(source_path, path);
            }
            other => panic!("expected Named, got {other:?}"),
        }
    }

    #[test]
    fn malformed_system_config_errors() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("system.json");
        write_secure(&path, "{ not json");
        let err = load_system_config(Some(&path)).unwrap_err();
        assert!(matches!(err, SystemLoadError::Parse { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn world_readable_system_config_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("system.json");
        fs::write(
            &path,
            serde_json::json!({
                "schemaVersion": 1,
                "name": "leaky",
                "projects": []
            })
            .to_string(),
        )
        .unwrap();
        // Loosen mode to 0644 (the umask default). Loader must
        // refuse: API keys live in this file.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let err = load_system_config(Some(&path)).unwrap_err();
        match err {
            SystemLoadError::InsecureMode { mode, .. } => {
                assert_eq!(mode, 0o644);
            }
            other => panic!("expected InsecureMode, got {other:?}"),
        }
    }

    fn fake_mount_for_slug(slug: &str) -> MountInfo {
        use crate::load::LoadedProject;
        use crate::schema::ProjectConfig;
        MountInfo {
            path: PathBuf::from(format!("/fake/{slug}")),
            state: MountState::Project(LoadedProject {
                root: PathBuf::from(format!("/fake/{slug}")),
                config: ProjectConfig {
                    schema_version: 1,
                    slug: slug.to_owned(),
                    name: slug.to_owned(),
                    description: None,
                    artifacts_path: None,
                    scan_paths: None,
                    overflow: BTreeMap::new(),
                },
                collections: Vec::new(),
                diagnostics: Vec::new(),
            }),
        }
    }

    #[test]
    fn missing_slugs_is_empty_for_unnamed_system() {
        let mounts = vec![fake_mount_for_slug("a")];
        assert!(missing_project_slugs(&LoadedSystem::Unnamed, &mounts).is_empty());
    }

    #[test]
    fn missing_slugs_reports_expected_but_unmounted_projects() {
        let system = LoadedSystem::Named {
            config: Box::new(SystemConfig {
                schema_version: 1,
                name: "sys".to_owned(),
                projects: vec![
                    crate::schema::SystemProjectRef {
                        slug: "a".to_owned(),
                        overflow: BTreeMap::new(),
                    },
                    crate::schema::SystemProjectRef {
                        slug: "b".to_owned(),
                        overflow: BTreeMap::new(),
                    },
                    crate::schema::SystemProjectRef {
                        slug: "c".to_owned(),
                        overflow: BTreeMap::new(),
                    },
                ],
                link_types: Vec::new(),
                languages: None,
                llm: None,
                overflow: BTreeMap::new(),
            }),
            source_path: PathBuf::from("/fake/system.json"),
        };
        let mounts = vec![fake_mount_for_slug("a"), fake_mount_for_slug("c")];
        let missing = missing_project_slugs(&system, &mounts);
        assert_eq!(missing, vec!["b".to_owned()]);
    }
}
