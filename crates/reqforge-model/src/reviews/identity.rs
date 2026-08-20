//! Reviewer-identity plumbing per `REVIEW-reviewerIdentity`.
//!
//! Three sources contribute to the "who might be reviewing" list the
//! UI surfaces in its dropdown:
//!
//! 1. The active mount's `.git/config` `[user] name = …` value,
//!    parsed without spawning git and without linking `gix`.
//! 2. `<workspace>/reviewers.json`, persisted across container
//!    restarts. The workspace directory is resolved from
//!    `REQFORGE_WORKSPACE_DIR` — see `DiscoveryConfig`.
//! 3. The session cache on `AppState` (identities submitted during
//!    the current container lifetime).
//!
//! This module owns #1 and #2; #3 lives on `AppState` so it can
//! piggy-back on the state's existing `RwLock`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::schema::Overflow;

/// Aggregated identity options presented to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerIdentityOptions {
    /// Git user.name from the active mount's `.git/config`, or the
    /// workspace-level git config when no specific mount is in
    /// scope. Absent when neither source has a `[user] name` entry.
    pub git_default: Option<String>,
    /// Identities persisted in `<workspace>/reviewers.json`.
    pub persisted: Vec<String>,
    /// Identities used this container lifetime (from `AppState`).
    pub session: Vec<String>,
}

/// On-disk shape of `<workspace>/reviewers.json`. Unknown fields
/// round-trip through `overflow` per `FORMAT-fieldTolerance`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewersFile {
    #[serde(default)]
    pub reviewers: Vec<String>,

    #[serde(flatten)]
    pub overflow: Overflow,
}

/// Parse `.git/config` (a plain INI file) and return the first
/// `[user] name = …` value found. Returns `Ok(None)` when the file
/// exists but has no `[user]` section, or no `name` key inside it.
///
/// Deliberately simple: single-line values, `#` and `;` comments,
/// optional surrounding quotes stripped. Enough for the `user.name`
/// key we care about; real git supports include directives and
/// subsections we don't.
pub fn parse_git_config_user_name(path: &Path) -> Result<Option<String>, std::io::Error> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    Ok(parse_git_config_user_name_str(&text))
}

/// The string-oriented parser — split out so tests don't need a
/// tempdir for every case.
pub fn parse_git_config_user_name_str(text: &str) -> Option<String> {
    let mut in_user = false;
    for raw_line in text.lines() {
        let line = strip_inline_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            // Accept `[user]` and `[user "foo"]`-style subsections;
            // the key we want only exists under bare `[user]`.
            in_user = section == "user";
            continue;
        }
        if !in_user {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("name")
        {
            return Some(strip_quotes(value.trim()).to_owned());
        }
    }
    None
}

fn strip_inline_comment(line: &str) -> &str {
    // Respect quotes: a `#` or `;` inside a quoted value isn't a
    // comment. Good enough for `.git/config`; real INI pathology is
    // not our problem.
    let mut in_quotes = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '#' | ';' if !in_quotes => return &line[..i],
            _ => {}
        }
    }
    line
}

fn strip_quotes(value: &str) -> &str {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Load `<workspace>/reviewers.json` when it exists. An absent file
/// is not an error — the first review that commits a new identity
/// creates it. A malformed file IS an error; the caller surfaces
/// that so the operator can fix it instead of silently losing
/// persisted reviewers.
pub fn load_reviewers_json(path: &Path) -> Result<ReviewersFile, ReviewersFileError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|source| ReviewersFileError::Parse {
            path: path.to_path_buf(),
            source,
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ReviewersFile::default()),
        Err(source) => Err(ReviewersFileError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewersFileError {
    #[error("i/o error reading reviewers.json at {}: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in reviewers.json at {}: {source}", path.display())]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_user_section() {
        let text = "[user]\n    name = Alice\n    email = a@example.com\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn returns_none_when_user_section_missing() {
        let text = "[core]\n    repositoryformatversion = 0\n";
        assert!(parse_git_config_user_name_str(text).is_none());
    }

    #[test]
    fn returns_none_when_user_section_lacks_name() {
        let text = "[user]\n    email = a@example.com\n";
        assert!(parse_git_config_user_name_str(text).is_none());
    }

    #[test]
    fn strips_inline_comments_outside_quotes() {
        let text = "[user]\n    name = Alice # primary\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn keeps_hash_inside_quoted_value() {
        let text = "[user]\n    name = \"#Alice\"\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("#Alice")
        );
    }

    #[test]
    fn strips_surrounding_quotes() {
        let text = "[user]\n    name = \"Alice Example\"\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("Alice Example")
        );
    }

    #[test]
    fn ignores_subsection_names() {
        // Git allows `[user "foo"]` subsections; we want the bare
        // `[user]` section only.
        let text = "[user \"github\"]\n    name = Wrong\n[user]\n    name = Right\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("Right")
        );
    }

    #[test]
    fn key_match_is_case_insensitive() {
        // Git's conventional spelling is `name`, but the format is
        // case-insensitive for keys.
        let text = "[user]\n    Name = Alice\n";
        assert_eq!(
            parse_git_config_user_name_str(text).as_deref(),
            Some("Alice")
        );
    }

    #[test]
    fn missing_file_is_ok_none() {
        let temp = tempfile::tempdir().unwrap();
        let result = parse_git_config_user_name(&temp.path().join("nope")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn missing_reviewers_file_returns_empty_list() {
        let temp = tempfile::tempdir().unwrap();
        let loaded = load_reviewers_json(&temp.path().join("reviewers.json")).unwrap();
        assert!(loaded.reviewers.is_empty());
    }

    #[test]
    fn reads_reviewers_json_happy_path() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reviewers.json");
        std::fs::write(
            &path,
            serde_json::json!({ "reviewers": ["alice", "bob"] }).to_string(),
        )
        .unwrap();
        let loaded = load_reviewers_json(&path).unwrap();
        assert_eq!(loaded.reviewers, vec!["alice", "bob"]);
    }

    #[test]
    fn reviewers_json_round_trips_unknown_fields() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reviewers.json");
        let raw = serde_json::json!({
            "reviewers": ["alice"],
            "futureField": { "nested": true }
        });
        std::fs::write(&path, raw.to_string()).unwrap();
        let loaded = load_reviewers_json(&path).unwrap();
        let round = serde_json::to_value(&loaded).unwrap();
        assert_eq!(round, raw);
    }

    #[test]
    fn malformed_reviewers_json_is_an_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("reviewers.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let err = load_reviewers_json(&path).unwrap_err();
        assert!(matches!(err, ReviewersFileError::Parse { .. }));
    }
}
