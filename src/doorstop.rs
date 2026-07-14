//! Discovery of a subject repository's Doorstop layout.
//!
//! Reads the subject's own Doorstop configuration — nothing about any specific
//! repository is hardcoded — so the tool is subject-agnostic (A3).
//!
//! Implements: REQ007 (discover the subject's Doorstop layout from its own config)

use crate::source::{Item, RequirementsSource};
use anyhow::{Context, Result};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

const DOORSTOP_CONFIG: &str = ".doorstop.yml";

/// Directories never worth descending into when hunting for Doorstop docs.
const PRUNE_DIRS: [&str; 4] = [".git", "target", "node_modules", ".venv"];

/// One discovered Doorstop document: its directory, item prefix, and item IDs.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DoorstopDoc {
    pub dir: PathBuf,
    pub prefix: String,
    pub item_ids: Vec<String>,
}

#[derive(serde::Deserialize)]
struct Config {
    settings: Settings,
}

#[derive(serde::Deserialize)]
struct Settings {
    prefix: String,
}

fn is_pruned(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .is_some_and(|n| PRUNE_DIRS.contains(&n))
}

/// Discover every Doorstop document under `subject_root`, sorted by directory.
///
/// Symlinks are not followed (a subject repo may symlink to unrelated trees),
/// and heavy build/VCS directories are skipped.
///
/// Implements: REQ007
pub fn discover(subject_root: &Path) -> Result<Vec<DoorstopDoc>> {
    let mut docs = Vec::new();
    let walker = WalkDir::new(subject_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_pruned(e));
    for entry in walker {
        let entry = entry.with_context(|| format!("walking {}", subject_root.display()))?;
        if entry.file_type().is_file() && entry.file_name() == DOORSTOP_CONFIG {
            let dir = entry.path().parent().unwrap_or(subject_root).to_path_buf();
            docs.push(read_doc(entry.path(), dir)?);
        }
    }
    docs.sort_by(|a, b| a.dir.cmp(&b.dir));
    Ok(docs)
}

fn read_doc(config_path: &Path, dir: PathBuf) -> Result<DoorstopDoc> {
    let text = std::fs::read_to_string(config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;
    let cfg: Config = serde_yaml::from_str(&text)
        .with_context(|| format!("parsing {}", config_path.display()))?;
    let mut item_ids = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if let Some(id) = item_id(name, &cfg.settings.prefix) {
                item_ids.push(id);
            }
        }
    }
    item_ids.sort();
    Ok(DoorstopDoc {
        dir,
        prefix: cfg.settings.prefix,
        item_ids,
    })
}

/// Return the item ID if `filename` is a Doorstop item file for `prefix`
/// (`<prefix>[sep]<digits>.yml`), not the config itself.
fn item_id(filename: &str, prefix: &str) -> Option<String> {
    if filename == DOORSTOP_CONFIG {
        return None;
    }
    let stem = filename.strip_suffix(".yml")?;
    let rest = stem.strip_prefix(prefix)?;
    let digits = rest.trim_start_matches(|c: char| !c.is_ascii_digit());
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        Some(stem.to_string())
    } else {
        None
    }
}

/// The Doorstop [`RequirementsSource`] adapter (adapter #1): reads item prose
/// from a subject's Doorstop tree and supplies a content-hash revision token,
/// since Doorstop has no native change signal we key off (R-src-3).
///
/// Implements: REQ009 (Doorstop adapter behind the requirements-source seam)
pub struct DoorstopSource {
    root: PathBuf,
}

impl DoorstopSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[derive(serde::Deserialize)]
struct DoorstopItem {
    #[serde(default)]
    text: String,
}

impl RequirementsSource for DoorstopSource {
    fn items(&self) -> Result<Vec<Item>> {
        let mut items = Vec::new();
        for doc in discover(&self.root)? {
            for id in &doc.item_ids {
                let path = doc.dir.join(format!("{id}.yml"));
                let raw = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                let item: DoorstopItem = serde_yaml::from_str(&raw)
                    .with_context(|| format!("parsing {}", path.display()))?;
                let text = item.text.trim().to_string();
                items.push(Item {
                    revision: content_hash(&text),
                    id: id.clone(),
                    text,
                    title: None,
                    verification_hint: None,
                });
            }
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(items)
    }
}

/// A stable-per-build fingerprint of an item's prose, used as the revision token
/// when the source has no native one (R-src-3).
///
// ponytail: std SipHash — deterministic within a binary, NOT guaranteed stable
// across Rust releases; swap to a sha2 digest if cross-binary token stability
// (e.g. surviving a provreq upgrade) ever matters. Advisory use tolerates churn.
fn content_hash(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_id_accepts_item_files_and_rejects_others() {
        assert_eq!(item_id("REQ001.yml", "REQ").as_deref(), Some("REQ001"));
        assert_eq!(item_id("REQ-042.yml", "REQ").as_deref(), Some("REQ-042"));
        assert_eq!(item_id(".doorstop.yml", "REQ"), None);
        assert_eq!(item_id("README.yml", "REQ"), None);
        assert_eq!(item_id("REQ001.txt", "REQ"), None);
        assert_eq!(item_id("OTHER001.yml", "REQ"), None);
    }

    // Verifies: REQ007 — discovery reads prefix + item IDs from the subject's own config.
    #[test]
    fn discover_reads_prefix_and_items() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("reqs");
        std::fs::create_dir(&doc).unwrap();
        std::fs::write(doc.join(".doorstop.yml"), "settings:\n  prefix: REQ\n").unwrap();
        std::fs::write(doc.join("REQ001.yml"), "text: a\n").unwrap();
        std::fs::write(doc.join("REQ002.yml"), "text: b\n").unwrap();
        std::fs::write(doc.join("notes.md"), "ignore me").unwrap();

        let docs = discover(tmp.path()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].prefix, "REQ");
        assert_eq!(docs[0].item_ids, vec!["REQ001", "REQ002"]);
    }

    // Verifies: REQ009 — the Doorstop adapter yields source-agnostic items
    // carrying prose + a revision token, sorted by id.
    #[test]
    fn doorstop_source_reads_prose_and_revision() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("reqs");
        std::fs::create_dir(&doc).unwrap();
        std::fs::write(doc.join(".doorstop.yml"), "settings:\n  prefix: REQ\n").unwrap();
        std::fs::write(doc.join("REQ002.yml"), "text: |\n  the second item\n").unwrap();
        std::fs::write(doc.join("REQ001.yml"), "text: |\n  the first item\n").unwrap();

        let items = DoorstopSource::new(tmp.path()).items().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "REQ001");
        assert_eq!(items[0].text, "the first item");
        assert_eq!(items[1].id, "REQ002");
        // Distinct prose → distinct revision tokens.
        assert_ne!(items[0].revision, items[1].revision);
        // Same prose → same token (deterministic).
        assert_eq!(items[0].revision, content_hash("the first item"));
    }

    #[test]
    fn discover_skips_pruned_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let buried = tmp.path().join("target").join("reqs");
        std::fs::create_dir_all(&buried).unwrap();
        std::fs::write(buried.join(".doorstop.yml"), "settings:\n  prefix: REQ\n").unwrap();

        assert!(discover(tmp.path()).unwrap().is_empty());
    }
}
