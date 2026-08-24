//! YAML readers for doorstop `.doorstop.yml` marker files and
//! per-item `.yml` files.
//!
//! The parser is tolerant of doorstop's conventions — unknown
//! extension fields land in `extensions` so the plan builder
//! can preserve them in the imported artifact's `legacy`
//! object (per INTEROP-doorstopItemMapping).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};
use serde_yaml::Value;

/// Accept either a YAML integer or a YAML string of digits for
/// the `settings.digits` field. Doorstop trees in the wild
/// sometimes quote the value (`digits: "4"`); Python's YAML
/// loader silently coerces, and so do we.
fn deserialize_u32_or_numeric_string<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| D::Error::custom(format!("digits {n:?} does not fit in u32"))),
        Some(Value::String(s)) => s
            .trim()
            .parse::<u32>()
            .map(Some)
            .map_err(|e| D::Error::custom(format!("digits string {s:?} is not a u32: {e}"))),
        Some(other) => Err(D::Error::custom(format!(
            "digits must be an integer or numeric string, got {other:?}"
        ))),
    }
}

/// Doorstop document marker — the contents of a `.doorstop.yml`
/// marker file. The wrapping `settings:` key mirrors the YAML
/// layout doorstop writes.
#[derive(Debug, Clone, Deserialize)]
pub struct DoorstopMarker {
    pub settings: DoorstopSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DoorstopSettings {
    pub prefix: String,
    #[serde(default)]
    pub sep: Option<String>,
    #[serde(default, deserialize_with = "deserialize_u32_or_numeric_string")]
    pub digits: Option<u32>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub itemformat: Option<String>,
}

/// A parsed doorstop document: its marker + the list of its
/// items, already read from disk and deserialised.
#[derive(Debug, Clone)]
pub struct DoorstopDocument {
    pub marker_path: PathBuf,
    pub directory: PathBuf,
    pub settings: DoorstopSettings,
    pub items: Vec<DoorstopItem>,
}

/// One doorstop item, after YAML parsing. Every field below
/// corresponds directly to a doorstop schema field; unknown
/// extension fields are collected in `extensions` so the plan
/// builder can route them into the imported artifact's
/// `legacy` object.
#[derive(Debug, Clone)]
pub struct DoorstopItem {
    /// Original source path on disk — used for preserve-
    /// originals semantics and for error messages that name
    /// the offending file.
    pub source_path: PathBuf,
    /// Doorstop UID derived from the file stem (e.g. the file
    /// `ART-activeField.yml` yields the UID `ART-activeField`).
    /// This is the identifier form doorstop uses on disk; the
    /// plan builder normalises it into a ReqForge name via
    /// [`super::ids`].
    pub uid: String,

    pub header: Option<String>,
    pub text: Option<String>,
    pub active: Option<bool>,
    pub derived: Option<bool>,
    pub level: Option<String>,
    pub normative: Option<bool>,
    pub links: Vec<String>,
    pub ref_field: Option<String>,
    pub reviewed: Option<String>,

    /// Extension fields. Everything the parser doesn't
    /// recognise above lands here in the original YAML order
    /// so the `legacy` preservation retains the operator's
    /// structure rather than re-sorting keys.
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("YAML error parsing {path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("marker file {path} missing required 'settings.prefix'")]
    MarkerMissingPrefix { path: PathBuf },
    #[error("item file {path} is not a YAML mapping")]
    ItemNotMapping { path: PathBuf },
}

/// Read a `.doorstop.yml` marker file.
pub fn read_marker(path: &Path) -> Result<DoorstopMarker, ParseError> {
    let bytes = fs::read(path).map_err(|source| ParseError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let marker: DoorstopMarker =
        serde_yaml::from_slice(&bytes).map_err(|source| ParseError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
    if marker.settings.prefix.trim().is_empty() {
        return Err(ParseError::MarkerMissingPrefix {
            path: path.to_path_buf(),
        });
    }
    Ok(marker)
}

/// Read a single doorstop item file. The UID is derived from
/// the file stem so a future renamed-item surface doesn't have
/// to re-parse the content.
pub fn read_item(path: &Path) -> Result<DoorstopItem, ParseError> {
    let bytes = fs::read(path).map_err(|source| ParseError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: Value = serde_yaml::from_slice(&bytes).map_err(|source| ParseError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    let mapping = match raw {
        Value::Mapping(m) => m,
        _ => {
            return Err(ParseError::ItemNotMapping {
                path: path.to_path_buf(),
            });
        }
    };

    let uid = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_owned();

    let mut header: Option<String> = None;
    let mut text: Option<String> = None;
    let mut active: Option<bool> = None;
    let mut derived: Option<bool> = None;
    let mut level: Option<String> = None;
    let mut normative: Option<bool> = None;
    let mut links: Vec<String> = Vec::new();
    let mut ref_field: Option<String> = None;
    let mut reviewed: Option<String> = None;
    let mut extensions: BTreeMap<String, Value> = BTreeMap::new();

    for (key, value) in mapping {
        let Some(key_str) = key.as_str() else {
            // A non-string key is exotic for doorstop but we
            // tolerate it by stringifying — lets the legacy
            // preservation surface retain it.
            extensions.insert(format!("{:?}", key), value);
            continue;
        };
        match key_str {
            "header" => header = value.as_str().map(str::to_owned),
            "text" => text = value.as_str().map(str::to_owned),
            "active" => active = value.as_bool(),
            "derived" => derived = value.as_bool(),
            "level" => level = Some(stringify_level(&value)),
            "normative" => normative = value.as_bool(),
            "links" => {
                links = parse_links(&value);
            }
            "ref" => {
                ref_field = value.as_str().map(str::to_owned);
            }
            "reviewed" => {
                reviewed = value.as_str().map(str::to_owned);
            }
            other => {
                extensions.insert(other.to_owned(), value);
            }
        }
    }

    Ok(DoorstopItem {
        source_path: path.to_path_buf(),
        uid,
        header,
        text,
        active,
        derived,
        level,
        normative,
        links,
        ref_field,
        reviewed,
        extensions,
    })
}

/// Doorstop stores `level` as either a scalar number (`1.5`)
/// or a string (`"1.2.3"`). We normalise to the string form so
/// the plan builder can forward it to `outlineLevel` as a
/// `String` per FORMAT-artifactMetadataSchema.
fn stringify_level(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim()
            .to_owned(),
    }
}

/// Doorstop `links` accepts several shapes: an empty
/// sequence; a sequence of scalars (`[REQ001, DES002]`); or a
/// sequence of single-key maps (`[{REQ001: <hash>}]`) where the
/// value is the reviewed hash the parent was approved at.
/// ReqForge doesn't track per-link hashes in Phase 8, so we
/// extract the UID text in all cases and drop the hash.
fn parse_links(value: &Value) -> Vec<String> {
    let Some(seq) = value.as_sequence() else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for entry in seq {
        if let Some(uid) = entry.as_str() {
            out.push(uid.to_owned());
            continue;
        }
        if let Value::Mapping(m) = entry
            && let Some((k, _v)) = m.iter().next()
            && let Some(uid) = k.as_str()
        {
            out.push(uid.to_owned());
        }
    }
    out
}

/// Walk a source tree for `.doorstop.yml` markers, reading
/// each + every sibling `.yml` item file in the same
/// directory. The walk is shallow-per-marker: items must live
/// next to their `.doorstop.yml`, matching doorstop's own
/// layout convention.
pub fn discover(source: &Path) -> Result<Vec<DoorstopDocument>, ParseError> {
    let mut out: Vec<DoorstopDocument> = Vec::new();
    discover_recursive(source, &mut out)?;
    // Stable order by marker path so the plan's pane list is
    // deterministic across runs.
    out.sort_by(|a, b| a.marker_path.cmp(&b.marker_path));
    Ok(out)
}

fn discover_recursive(dir: &Path, out: &mut Vec<DoorstopDocument>) -> Result<(), ParseError> {
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(source) => {
            return Err(ParseError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut marker_path: Option<PathBuf> = None;
    let mut child_dirs: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            // Skip .git and other dotfile-roots to stay out
            // of VCS metadata.
            if p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with('.') && s != ".")
            {
                continue;
            }
            child_dirs.push(p);
        } else if p.file_name().and_then(|s| s.to_str()) == Some(".doorstop.yml") {
            marker_path = Some(p);
        }
    }

    if let Some(marker) = marker_path {
        let parsed_marker = read_marker(&marker)?;
        let directory = marker.parent().unwrap_or(dir).to_path_buf();
        let items = read_items_in_dir(&directory)?;
        out.push(DoorstopDocument {
            marker_path: marker,
            directory,
            settings: parsed_marker.settings,
            items,
        });
    }

    for child in child_dirs {
        discover_recursive(&child, out)?;
    }
    Ok(())
}

fn read_items_in_dir(dir: &Path) -> Result<Vec<DoorstopItem>, ParseError> {
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(source) => {
            return Err(ParseError::Io {
                path: dir.to_path_buf(),
                source,
            });
        }
    };
    let mut items: Vec<DoorstopItem> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if file_name == ".doorstop.yml" {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if !file_name.ends_with(".yml") && !file_name.ends_with(".yaml") {
            continue;
        }
        items.push(read_item(&path)?);
    }
    items.sort_by(|a, b| a.uid.cmp(&b.uid));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_marker_with_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(".doorstop.yml");
        std::fs::write(
            &marker,
            "settings:\n  prefix: REQ\n  sep: '-'\n  digits: 3\n  parent: ART\n  itemformat: yaml\n",
        )
        .unwrap();
        let parsed = read_marker(&marker).unwrap();
        assert_eq!(parsed.settings.prefix, "REQ");
        assert_eq!(parsed.settings.sep.as_deref(), Some("-"));
        assert_eq!(parsed.settings.digits, Some(3));
        assert_eq!(parsed.settings.parent.as_deref(), Some("ART"));
        assert_eq!(parsed.settings.itemformat.as_deref(), Some("yaml"));
    }

    #[test]
    fn reads_marker_with_quoted_digits() {
        // Some doorstop trees in the wild quote the integer
        // (e.g. `digits: "4"`) — Python's YAML loader silently
        // coerces but our serde parser used to refuse. Accept
        // both shapes.
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(".doorstop.yml");
        std::fs::write(&marker, "settings:\n  digits: \"4\"\n  prefix: REQ\n").unwrap();
        let parsed = read_marker(&marker).unwrap();
        assert_eq!(parsed.settings.digits, Some(4));
    }

    #[test]
    fn marker_with_garbage_digits_errors() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(".doorstop.yml");
        std::fs::write(
            &marker,
            "settings:\n  digits: \"not-a-number\"\n  prefix: REQ\n",
        )
        .unwrap();
        let err = read_marker(&marker);
        assert!(matches!(err, Err(ParseError::Yaml { .. })));
    }

    #[test]
    fn marker_without_prefix_errors() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join(".doorstop.yml");
        std::fs::write(&marker, "settings:\n  prefix: ''\n").unwrap();
        let err = read_marker(&marker);
        assert!(matches!(err, Err(ParseError::MarkerMissingPrefix { .. })));
    }

    #[test]
    fn item_parses_all_fields_and_normalises_level() {
        let dir = tempfile::tempdir().unwrap();
        let item = dir.path().join("REQ001.yml");
        std::fs::write(
            &item,
            "active: true\nderived: false\nheader: |\n  Title here\nlevel: 1.5\nlinks: []\nnormative: true\nref: ''\nreviewed: abc\ntext: |\n  Body text.\n",
        )
        .unwrap();
        let parsed = read_item(&item).unwrap();
        assert_eq!(parsed.uid, "REQ001");
        assert_eq!(parsed.header.as_deref(), Some("Title here\n"));
        assert_eq!(parsed.text.as_deref(), Some("Body text.\n"));
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.derived, Some(false));
        assert_eq!(parsed.level.as_deref(), Some("1.5"));
        assert_eq!(parsed.normative, Some(true));
        assert_eq!(parsed.reviewed.as_deref(), Some("abc"));
        assert_eq!(parsed.ref_field.as_deref(), Some(""));
        assert!(parsed.links.is_empty());
        assert!(parsed.extensions.is_empty());
    }

    #[test]
    fn item_links_accept_scalar_and_mapping_forms() {
        let dir = tempfile::tempdir().unwrap();
        let item = dir.path().join("DES001.yml");
        std::fs::write(&item, "links:\n  - REQ001\n  - {REQ002: \"some-hash\"}\n").unwrap();
        let parsed = read_item(&item).unwrap();
        assert_eq!(parsed.links, vec!["REQ001", "REQ002"]);
    }

    #[test]
    fn item_extension_fields_land_in_extensions_map() {
        let dir = tempfile::tempdir().unwrap();
        let item = dir.path().join("ITM-001.yml");
        std::fs::write(
            &item,
            "header: |\n  t\ntext: ''\ncustom_field: custom-value\nverification: manual\n",
        )
        .unwrap();
        let parsed = read_item(&item).unwrap();
        assert!(parsed.extensions.contains_key("custom_field"));
        assert!(parsed.extensions.contains_key("verification"));
    }

    #[test]
    fn discover_walks_subdirs_and_sorts_markers() {
        let dir = tempfile::tempdir().unwrap();
        // Marker A at the root.
        std::fs::write(
            dir.path().join(".doorstop.yml"),
            "settings:\n  prefix: ROOT\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("ROOT001.yml"),
            "header: |\n  Root\ntext: ''\n",
        )
        .unwrap();
        // Marker B in a subdir.
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join(".doorstop.yml"), "settings:\n  prefix: SUB\n").unwrap();
        std::fs::write(sub.join("SUB001.yml"), "header: |\n  Sub\ntext: ''\n").unwrap();
        let docs = discover(dir.path()).unwrap();
        assert_eq!(docs.len(), 2);
        // Root marker sorts before the sub marker by path.
        assert!(docs[0].marker_path < docs[1].marker_path);
        assert_eq!(docs[0].settings.prefix, "ROOT");
        assert_eq!(docs[1].settings.prefix, "SUB");
        assert_eq!(docs[0].items.len(), 1);
    }

    #[test]
    fn discover_skips_dot_directories_like_git() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".doorstop.yml"),
            "settings:\n  prefix: ROOT\n",
        )
        .unwrap();
        // Pretend .git contains a stray marker — the walk
        // must not descend into it.
        let git = dir.path().join(".git");
        std::fs::create_dir(&git).unwrap();
        std::fs::write(
            git.join(".doorstop.yml"),
            "settings:\n  prefix: INSIDE_GIT\n",
        )
        .unwrap();
        let docs = discover(dir.path()).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].settings.prefix, "ROOT");
    }
}
