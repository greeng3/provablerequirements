//! Scan-path resolution + ignore-directory machinery
//! (TRACE-codeScanConfig).
//!
//! Each Project's `scan_paths` declares the directories to
//! walk. When absent, a small set of sensible defaults
//! (`src/`, `tests/`, `lib/`) is used, filtered to those
//! actually present on disk. Ignore directories are hard-coded
//! — Phase 9a doesn't surface a user override.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::load::LoadedProject;

/// Default directories to scan when the project's
/// `reqforge.json` doesn't declare `scanPaths`. Matches the
/// common convention called out by TRACE-codeScanConfig.
pub const DEFAULT_SCAN_PATHS: &[&str] = &["src", "tests", "lib"];

/// Directory names the walker never descends into.
/// Hard-coded; if an operator needs an override we add the
/// surface in a later phase.
pub fn ignore_dirs() -> &'static HashSet<&'static str> {
    static DIRS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    DIRS.get_or_init(|| {
        [
            ".git",
            "node_modules",
            "target",
            "dist",
            "build",
            "__pycache__",
            ".venv",
        ]
        .into_iter()
        .collect()
    })
}

/// Resolve the list of absolute paths the walker should
/// descend into for one project. Declared scan paths are
/// honoured as-is (missing ones surface a warning to the
/// caller via the separate `missing` list); defaults are
/// silently filtered to those that exist.
pub struct ResolvedScanPaths {
    pub roots: Vec<PathBuf>,
    /// Declared scan paths that the walker couldn't find on
    /// disk. Empty when the caller relied on defaults (missing
    /// defaults are expected — a project without `tests/` is
    /// common).
    pub missing_declared: Vec<String>,
}

pub fn resolve_scan_paths(project: &LoadedProject) -> ResolvedScanPaths {
    let root = &project.root;
    match project.config.scan_paths.as_deref() {
        Some(declared) => resolve_declared(root, declared),
        None => resolve_defaults(root),
    }
}

fn resolve_declared(root: &Path, declared: &[String]) -> ResolvedScanPaths {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for entry in declared {
        // Refuse absolute + parent-traversal forms so a
        // project config can't point the scanner at `/etc` or
        // the filesystem root.
        let cleaned = entry.trim().trim_start_matches("./").replace('\\', "/");
        if cleaned.is_empty() || cleaned.contains("..") || cleaned.starts_with('/') {
            missing.push(entry.clone());
            continue;
        }
        let candidate = root.join(&cleaned);
        if !candidate.is_dir() {
            missing.push(entry.clone());
            continue;
        }
        roots.push(candidate);
    }
    ResolvedScanPaths {
        roots,
        missing_declared: missing,
    }
}

fn resolve_defaults(root: &Path) -> ResolvedScanPaths {
    let mut roots: Vec<PathBuf> = Vec::new();
    for default in DEFAULT_SCAN_PATHS {
        let candidate = root.join(default);
        if candidate.is_dir() {
            roots.push(candidate);
        }
    }
    ResolvedScanPaths {
        roots,
        missing_declared: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::LoadedProject;
    use crate::schema::ProjectConfig;
    use std::collections::BTreeMap;

    fn project_with_scan(root: PathBuf, scan_paths: Option<Vec<String>>) -> LoadedProject {
        LoadedProject {
            root,
            config: ProjectConfig {
                schema_version: 1,
                slug: "sample".into(),
                name: "sample".into(),
                description: None,
                artifacts_path: None,
                scan_paths,
                overflow: BTreeMap::new(),
            },
            collections: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn defaults_skip_missing_directories_silently() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let project = project_with_scan(root.clone(), None);
        let resolved = resolve_scan_paths(&project);
        assert_eq!(resolved.roots.len(), 2);
        assert!(resolved.roots.contains(&root.join("src")));
        assert!(resolved.roots.contains(&root.join("lib")));
        assert!(resolved.missing_declared.is_empty());
    }

    #[test]
    fn declared_paths_surface_missing_entries_as_warnings() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join("crates/alpha")).unwrap();
        let project = project_with_scan(
            root.clone(),
            Some(vec!["crates/alpha".into(), "crates/beta".into()]),
        );
        let resolved = resolve_scan_paths(&project);
        assert_eq!(resolved.roots, vec![root.join("crates/alpha")]);
        assert_eq!(resolved.missing_declared, vec!["crates/beta".to_owned()]);
    }

    #[test]
    fn declared_paths_reject_traversal_and_absolute_forms() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let project = project_with_scan(root, Some(vec!["../outside".into(), "/etc".into()]));
        let resolved = resolve_scan_paths(&project);
        assert!(resolved.roots.is_empty());
        assert_eq!(resolved.missing_declared.len(), 2);
    }

    #[test]
    fn ignore_dirs_contains_the_spec_minimum() {
        let dirs = ignore_dirs();
        for expected in [
            ".git",
            "node_modules",
            "target",
            "dist",
            "build",
            "__pycache__",
            ".venv",
        ] {
            assert!(dirs.contains(expected), "missing {expected}");
        }
    }
}
