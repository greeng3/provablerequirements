//! Source-language registry (TRACE-codeLanguageRegistry).
//!
//! A `Language` entry names its file-extension globs, line-
//! comment markers, and block-comment markers so the walker
//! can extract comments uniformly across languages. Built-ins
//! ship with the binary; System-declared entries augment the
//! registry at configuration-load time.
//!
//! System entries are add-only: a user-declared entry whose
//! `name` collides with a built-in is rejected at load time —
//! per the spec, the fix is upstream, not a silent override.

use std::collections::HashSet;

use thiserror::Error;

/// One source-language entry. Matched against a file by
/// (name == file name) for `Dockerfile` — per the spec —
/// otherwise by extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: &'static str,
    /// Lower-case file extensions this language owns, without
    /// a leading dot (e.g. `"rs"`, `"py"`).
    pub extensions: &'static [&'static str],
    /// Line-comment markers. A line-comment runs from the
    /// marker to the end of the line.
    pub line_comments: &'static [&'static str],
    /// Block-comment marker pairs. A block-comment runs from
    /// the opening marker to the matching closing marker,
    /// spanning line breaks. Python's triple-quoted strings
    /// are expressed as block-comment pairs per the spec.
    pub block_comments: &'static [(&'static str, &'static str)],
    /// When true, the language matches a file by name rather
    /// than extension. Used for `Dockerfile` and files matching
    /// the `Dockerfile.*` convention (see `matches_file`).
    pub dockerfile_name_match: bool,
}

impl Language {
    /// True when this language owns the given file path.
    /// Extension match is case-insensitive; Dockerfile match
    /// is case-sensitive per the convention.
    pub fn matches_file(&self, file_name: &str) -> bool {
        if self.dockerfile_name_match {
            // Match `Dockerfile` and any `Dockerfile.*` form.
            if file_name == "Dockerfile" {
                return true;
            }
            if let Some(rest) = file_name.strip_prefix("Dockerfile.")
                && !rest.is_empty()
            {
                return true;
            }
            return false;
        }
        let Some(dot) = file_name.rfind('.') else {
            return false;
        };
        let ext = file_name[dot + 1..].to_ascii_lowercase();
        self.extensions.iter().any(|e| *e == ext)
    }
}

/// Built-in languages — the spec-mandated minimum. YAML is
/// deliberately omitted.
pub const BUILTIN_LANGUAGES: &[Language] = &[
    Language {
        name: "Rust",
        extensions: &["rs"],
        line_comments: &["///", "//!", "//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
    },
    Language {
        name: "Python",
        extensions: &["py"],
        line_comments: &["#"],
        // Triple-quoted strings are treated as comments for
        // tag scanning per the spec. The parser matches them
        // as block-comment markers.
        block_comments: &[("\"\"\"", "\"\"\""), ("'''", "'''")],
        dockerfile_name_match: false,
    },
    Language {
        name: "JavaScript",
        extensions: &["js", "jsx", "mjs", "cjs"],
        line_comments: &["//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
    },
    Language {
        name: "TypeScript",
        extensions: &["ts", "tsx"],
        line_comments: &["//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
    },
    Language {
        name: "POSIX shell",
        extensions: &["sh", "bash"],
        line_comments: &["#"],
        block_comments: &[],
        dockerfile_name_match: false,
    },
    Language {
        name: "Dockerfile",
        extensions: &[],
        line_comments: &["#"],
        block_comments: &[],
        dockerfile_name_match: true,
    },
];

/// Errors from loading System-declared language entries.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LanguageRegistryError {
    #[error(
        "System-declared language '{0}' collides with a built-in entry; \
         built-ins are not overridable — file a bug or submit a change \
         against ReqForge instead"
    )]
    BuiltinCollision(String),
    #[error(
        "System-declared languages entry is not an array (expected a list of \
         {{ name, extensions, lineComments, blockComments }} objects)"
    )]
    NotAnArray,
    #[error("System-declared language entry {index} is missing the '{field}' field")]
    MissingField { index: usize, field: &'static str },
    #[error("System-declared language entry {index} has '{field}' of the wrong type")]
    WrongType { index: usize, field: &'static str },
}

/// Return the effective language registry — built-ins plus
/// System-declared entries. Each System entry is required to
/// carry `name`, `extensions[]`, `lineComments[]`, and
/// `blockComments[][2]`; `dockerfileNameMatch` is optional
/// (defaults to false).
///
/// Returned Languages referencing System-declared strings are
/// heap-allocated; we reuse the built-in's `&'static` form for
/// the built-ins so no per-entry allocation happens for the
/// common case.
pub fn effective_languages(
    system_languages: Option<&serde_json::Value>,
) -> Result<Vec<SystemLanguage>, LanguageRegistryError> {
    let mut out: Vec<SystemLanguage> = BUILTIN_LANGUAGES
        .iter()
        .map(SystemLanguage::from_builtin)
        .collect();
    let Some(value) = system_languages else {
        return Ok(out);
    };
    let array = match value {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Null => return Ok(out),
        _ => return Err(LanguageRegistryError::NotAnArray),
    };
    let builtin_names: HashSet<&str> = BUILTIN_LANGUAGES.iter().map(|l| l.name).collect();
    for (idx, raw) in array.iter().enumerate() {
        let obj = raw.as_object().ok_or(LanguageRegistryError::WrongType {
            index: idx,
            field: "<root>",
        })?;
        let name = obj.get("name").and_then(|v| v.as_str()).ok_or(
            LanguageRegistryError::MissingField {
                index: idx,
                field: "name",
            },
        )?;
        if builtin_names.contains(name) {
            return Err(LanguageRegistryError::BuiltinCollision(name.to_owned()));
        }
        let extensions = parse_string_list(obj.get("extensions"), idx, "extensions")?;
        let line_comments = parse_string_list(obj.get("lineComments"), idx, "lineComments")?;
        let block_comments = parse_block_comments(obj.get("blockComments"), idx)?;
        let dockerfile_name_match = obj
            .get("dockerfileNameMatch")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        out.push(SystemLanguage {
            name: name.to_owned(),
            extensions: extensions
                .into_iter()
                .map(|e| e.to_ascii_lowercase())
                .collect(),
            line_comments,
            block_comments,
            dockerfile_name_match,
        });
    }
    Ok(out)
}

/// Owned-form language entry that carries both built-ins and
/// System-declared entries in a single list. Conversion from
/// `Language` (the static built-in form) is zero-cost at the
/// type level — we clone the tiny `&'static str` slices into
/// owned `String`s so the runtime representation is uniform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLanguage {
    pub name: String,
    pub extensions: Vec<String>,
    pub line_comments: Vec<String>,
    pub block_comments: Vec<(String, String)>,
    pub dockerfile_name_match: bool,
}

impl SystemLanguage {
    pub fn from_builtin(lang: &Language) -> Self {
        SystemLanguage {
            name: lang.name.to_owned(),
            extensions: lang.extensions.iter().map(|s| (*s).to_owned()).collect(),
            line_comments: lang.line_comments.iter().map(|s| (*s).to_owned()).collect(),
            block_comments: lang
                .block_comments
                .iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect(),
            dockerfile_name_match: lang.dockerfile_name_match,
        }
    }

    pub fn matches_file(&self, file_name: &str) -> bool {
        if self.dockerfile_name_match {
            if file_name == "Dockerfile" {
                return true;
            }
            if let Some(rest) = file_name.strip_prefix("Dockerfile.")
                && !rest.is_empty()
            {
                return true;
            }
            return false;
        }
        let Some(dot) = file_name.rfind('.') else {
            return false;
        };
        let ext = file_name[dot + 1..].to_ascii_lowercase();
        self.extensions.contains(&ext)
    }
}

fn parse_string_list(
    value: Option<&serde_json::Value>,
    index: usize,
    field: &'static str,
) -> Result<Vec<String>, LanguageRegistryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or(LanguageRegistryError::WrongType { index, field })?;
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let s = entry
            .as_str()
            .ok_or(LanguageRegistryError::WrongType { index, field })?;
        out.push(s.to_owned());
    }
    Ok(out)
}

fn parse_block_comments(
    value: Option<&serde_json::Value>,
    index: usize,
) -> Result<Vec<(String, String)>, LanguageRegistryError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let array = value.as_array().ok_or(LanguageRegistryError::WrongType {
        index,
        field: "blockComments",
    })?;
    let mut out = Vec::with_capacity(array.len());
    for pair in array {
        let pair_array = pair.as_array().ok_or(LanguageRegistryError::WrongType {
            index,
            field: "blockComments",
        })?;
        if pair_array.len() != 2 {
            return Err(LanguageRegistryError::WrongType {
                index,
                field: "blockComments",
            });
        }
        let start = pair_array[0]
            .as_str()
            .ok_or(LanguageRegistryError::WrongType {
                index,
                field: "blockComments",
            })?;
        let end = pair_array[1]
            .as_str()
            .ok_or(LanguageRegistryError::WrongType {
                index,
                field: "blockComments",
            })?;
        out.push((start.to_owned(), end.to_owned()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_matches_rs_extension_case_insensitively() {
        let rust = BUILTIN_LANGUAGES.iter().find(|l| l.name == "Rust").unwrap();
        assert!(rust.matches_file("main.rs"));
        assert!(rust.matches_file("MAIN.RS"));
        assert!(!rust.matches_file("main.py"));
    }

    #[test]
    fn dockerfile_matches_by_name_and_with_suffix() {
        let df = BUILTIN_LANGUAGES
            .iter()
            .find(|l| l.name == "Dockerfile")
            .unwrap();
        assert!(df.matches_file("Dockerfile"));
        assert!(df.matches_file("Dockerfile.prod"));
        assert!(!df.matches_file("Dockerfile."));
        assert!(!df.matches_file("dockerfile"));
        assert!(!df.matches_file("readme.md"));
    }

    #[test]
    fn python_block_comments_include_triple_quoted_strings() {
        let py = BUILTIN_LANGUAGES
            .iter()
            .find(|l| l.name == "Python")
            .unwrap();
        let pairs: Vec<_> = py.block_comments.iter().collect();
        assert!(pairs.contains(&&("\"\"\"", "\"\"\"")));
        assert!(pairs.contains(&&("'''", "'''")));
    }

    #[test]
    fn effective_languages_returns_builtins_when_system_has_none() {
        let out = effective_languages(None).unwrap();
        assert_eq!(out.len(), BUILTIN_LANGUAGES.len());
        let names: Vec<&str> = out.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"Rust"));
        assert!(names.contains(&"Dockerfile"));
    }

    #[test]
    fn system_declared_language_merges_into_registry() {
        let value = serde_json::json!([
            {
                "name": "Ruby",
                "extensions": ["rb"],
                "lineComments": ["#"],
                "blockComments": [["=begin", "=end"]]
            }
        ]);
        let out = effective_languages(Some(&value)).unwrap();
        let ruby = out.iter().find(|l| l.name == "Ruby").unwrap();
        assert_eq!(ruby.extensions, vec!["rb".to_owned()]);
        assert_eq!(ruby.line_comments, vec!["#".to_owned()]);
        assert_eq!(
            ruby.block_comments,
            vec![("=begin".to_owned(), "=end".to_owned())]
        );
    }

    #[test]
    fn system_declared_language_colliding_with_builtin_is_rejected() {
        let value = serde_json::json!([
            { "name": "Rust", "extensions": ["rs"], "lineComments": ["//"], "blockComments": [] }
        ]);
        let err = effective_languages(Some(&value)).unwrap_err();
        assert_eq!(err, LanguageRegistryError::BuiltinCollision("Rust".into()));
    }

    #[test]
    fn system_languages_rejects_non_array_root() {
        let value = serde_json::json!({ "name": "Ruby" });
        let err = effective_languages(Some(&value)).unwrap_err();
        assert_eq!(err, LanguageRegistryError::NotAnArray);
    }

    #[test]
    fn system_language_missing_name_is_reported_with_the_index() {
        let value = serde_json::json!([
            { "extensions": ["rb"], "lineComments": ["#"], "blockComments": [] }
        ]);
        let err = effective_languages(Some(&value)).unwrap_err();
        assert_eq!(
            err,
            LanguageRegistryError::MissingField {
                index: 0,
                field: "name"
            }
        );
    }

    #[test]
    fn system_language_block_comments_must_be_pair_arrays() {
        // Missing end marker in the pair.
        let value = serde_json::json!([
            {
                "name": "Ruby",
                "extensions": ["rb"],
                "lineComments": [],
                "blockComments": [["=begin"]]
            }
        ]);
        let err = effective_languages(Some(&value)).unwrap_err();
        assert_eq!(
            err,
            LanguageRegistryError::WrongType {
                index: 0,
                field: "blockComments"
            }
        );
    }
}
