//! Source-language registry. Absorbed from ReqForge's `scan/languages.rs` — the built-in
//! comment grammar for Rust/Python/JS/TS/shell/Dockerfile — with its System-JSON runtime
//! extensibility deliberately dropped (YAGNI; a static table serves every subject provreq
//! scans today). Two fields are added for provreq's symbol resolver: the declaration
//! keywords and test-call names a tag's carrier can take (see [`super::resolve`]).
//!
//! Implements: REQ075

/// One source-language entry. Matched against a file by name for `Dockerfile`, otherwise by
/// extension. `line_comments` / `block_comments` drive the carver; `keyword_decls` /
/// `test_call_decls` drive the symbol resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: &'static str,
    /// Lower-case extensions this language owns, without a leading dot.
    pub extensions: &'static [&'static str],
    pub line_comments: &'static [&'static str],
    pub block_comments: &'static [(&'static str, &'static str)],
    /// True when the language matches by filename (`Dockerfile`, `Dockerfile.*`).
    pub dockerfile_name_match: bool,
    /// Keywords a declaration can open with; the resolver takes the identifier that follows
    /// the keyword as the symbol (`fn foo`, `def foo`, `func foo`).
    pub keyword_decls: &'static [&'static str],
    /// Test-runner call names whose first string argument is the symbol (`it('name', …)`).
    pub test_call_decls: &'static [&'static str],
}

impl Language {
    /// True when this language owns the given file name. Extension match is
    /// case-insensitive; Dockerfile match is case-sensitive per the convention.
    pub fn matches_file(&self, file_name: &str) -> bool {
        if self.dockerfile_name_match {
            if file_name == "Dockerfile" {
                return true;
            }
            return file_name
                .strip_prefix("Dockerfile.")
                .is_some_and(|rest| !rest.is_empty());
        }
        let Some(dot) = file_name.rfind('.') else {
            return false;
        };
        let ext = file_name[dot + 1..].to_ascii_lowercase();
        self.extensions.iter().any(|e| *e == ext)
    }
}

/// The language a file belongs to, or `None` when none owns it.
pub fn language_for(file_name: &str) -> Option<&'static Language> {
    BUILTIN_LANGUAGES.iter().find(|l| l.matches_file(file_name))
}

/// Built-in languages — the same set ReqForge ships, plus each one's declaration grammar.
pub const BUILTIN_LANGUAGES: &[Language] = &[
    Language {
        name: "Rust",
        extensions: &["rs"],
        line_comments: &["///", "//!", "//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
        keyword_decls: &[
            "fn", "struct", "enum", "trait", "type", "const", "static", "mod", "union",
        ],
        test_call_decls: &[],
    },
    Language {
        name: "Python",
        extensions: &["py"],
        line_comments: &["#"],
        block_comments: &[("\"\"\"", "\"\"\""), ("'''", "'''")],
        dockerfile_name_match: false,
        keyword_decls: &["def", "class"],
        test_call_decls: &[],
    },
    Language {
        name: "JavaScript",
        extensions: &["js", "jsx", "mjs", "cjs"],
        line_comments: &["//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
        keyword_decls: &["function", "class"],
        test_call_decls: &["it", "test", "describe"],
    },
    Language {
        name: "TypeScript",
        extensions: &["ts", "tsx"],
        line_comments: &["//"],
        block_comments: &[("/*", "*/")],
        dockerfile_name_match: false,
        keyword_decls: &["function", "class"],
        test_call_decls: &["it", "test", "describe"],
    },
    Language {
        name: "POSIX shell",
        extensions: &["sh", "bash"],
        line_comments: &["#"],
        block_comments: &[],
        dockerfile_name_match: false,
        keyword_decls: &["function"],
        test_call_decls: &[],
    },
    Language {
        name: "Dockerfile",
        extensions: &[],
        line_comments: &["#"],
        block_comments: &[],
        dockerfile_name_match: true,
        keyword_decls: &[],
        test_call_decls: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_matches_rs_extension_case_insensitively() {
        let rust = language_for("main.rs").unwrap();
        assert_eq!(rust.name, "Rust");
        assert!(language_for("MAIN.RS").is_some());
        assert_eq!(language_for("main.py").unwrap().name, "Python");
    }

    #[test]
    fn dockerfile_matches_by_name_and_with_suffix() {
        assert_eq!(language_for("Dockerfile").unwrap().name, "Dockerfile");
        assert_eq!(language_for("Dockerfile.prod").unwrap().name, "Dockerfile");
        assert!(language_for("Dockerfile.").is_none());
        assert!(language_for("dockerfile").is_none());
        assert!(language_for("readme.md").is_none());
    }

    #[test]
    fn python_block_comments_include_triple_quoted_strings() {
        let py = language_for("a.py").unwrap();
        assert!(py.block_comments.contains(&("\"\"\"", "\"\"\"")));
        assert!(py.block_comments.contains(&("'''", "'''")));
    }
}
