//! Symbol resolver — the portable spine, and the net-new capability of this slice. From a
//! tag's line, forward-scan for the declaration that immediately follows it (modulo comment
//! continuation, blank lines, attributes and decorators) and take its name, so a later
//! slice can run and rate that artifact by name.
//!
//! Portable by construction: the declaration grammar is a small per-language table on
//! [`Language`] (keywords whose next identifier is the name, plus test-runner call forms),
//! not a per-language parser. A language provreq does not know yet costs a table entry, not
//! a new dependency. Where nothing resolves the tag simply carries no symbol — never a
//! wrong one.
//!
//! Implements: REQ075

use super::languages::Language;

/// The symbol the tag on `lines[tag_line-1]` annotates, or `None` when no declaration
/// immediately follows it. `tag_line` is 1-based.
///
/// The declaration must be the next thing after the tag once comment-continuation, blank,
/// attribute (`#[...]`) and decorator (`@...`) lines are skipped. The first line that is
/// neither skippable nor a declaration — ordinary code, a `use`, a module-level tag's
/// unrelated neighbour — ends the scan with `None`. That immediacy is what keeps a
/// module-level `//!` tag from latching onto a function three items down.
pub fn symbol_for(lines: &[&str], tag_line: usize, language: &Language) -> Option<String> {
    let start = tag_line.checked_sub(1)?;
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if is_skippable(trimmed) {
            continue;
        }
        // First non-skippable line: it is the carrier or the scan ends here.
        return decl_name(trimmed, language);
    }
    None
}

/// Lines that may sit between a tag and its declaration. A universal set that covers every
/// built-in language's comment, attribute and decorator prefixes — a leading `#` is a Rust
/// attribute or a Python/shell comment either way, and neither opens a declaration.
/// `// ponytail: universal prefix set; per-language skip table if a language ever needs one.`
fn is_skippable(trimmed: &str) -> bool {
    trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with('*')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('@')
}

/// The declaration name on `line`, if it is one. A keyword declaration (`pub async fn foo`,
/// `const BAR`, `def foo`) yields the identifier that follows the keyword; a test-runner
/// call (`it('adds', …)`) yields its title string.
fn decl_name(line: &str, language: &Language) -> Option<String> {
    let tokens = identifier_tokens(line);
    for kw in language.keyword_decls {
        if let Some(pos) = tokens.iter().position(|t| t == kw) {
            if let Some(name) = tokens.get(pos + 1) {
                return Some((*name).to_owned());
            }
        }
    }
    for call in language.test_call_decls {
        if let Some(rest) = line.trim_start().strip_prefix(call) {
            if rest.trim_start().starts_with('(') {
                if let Some(title) = first_quoted(rest) {
                    return Some(title);
                }
            }
        }
    }
    None
}

/// The identifier-shaped tokens of a line, in order — maximal runs of `[A-Za-z0-9_]`. Used
/// to find a declaration keyword and the name after it without a real tokenizer.
fn identifier_tokens(line: &str) -> Vec<&str> {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .collect()
}

/// The first single-, double-, or backtick-quoted substring in `s` (the test title). Naive:
/// it does not honour escapes, which a test title effectively never contains.
fn first_quoted(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let open = bytes
        .iter()
        .position(|&b| b == b'\'' || b == b'"' || b == b'`')?;
    let quote = bytes[open];
    let rest = &s[open + 1..];
    let close = rest.find(quote as char)?;
    Some(rest[..close].to_owned())
}

#[cfg(test)]
mod tests {
    use super::super::languages::language_for;
    use super::*;

    fn lines(src: &str) -> Vec<&str> {
        src.lines().collect()
    }

    // The exact provreq layout: a multi-line `//` tag, then `#[test]`, then the fn.
    #[test]
    fn resolves_a_fn_past_comment_continuation_and_an_attribute() {
        let src = "    // Verifies: REQ021 — a note\n    // continued\n    #[test]\n    fn the_test() {}\n";
        let sym = symbol_for(&lines(src), 1, language_for("a.rs").unwrap());
        assert_eq!(sym.as_deref(), Some("the_test"));
    }

    #[test]
    fn resolves_past_qualifiers_on_the_decl_line() {
        let src = "// Implements: REQ001\npub async fn login() -> bool { true }\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.rs").unwrap()).as_deref(),
            Some("login")
        );
    }

    #[test]
    fn resolves_non_fn_rust_items() {
        let src = "/// Implements: REQ001\nconst MAX_TRIES: u32 = 3;\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.rs").unwrap()).as_deref(),
            Some("MAX_TRIES")
        );
    }

    // A tag whose next real line is ordinary code names no carrier — this is what keeps a
    // module-level `//!` tag from grabbing a distant item.
    #[test]
    fn ordinary_code_after_the_tag_resolves_to_none() {
        let src = "//! Implements: REQ030\n\nuse std::path::Path;\n\nfn helper() {}\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.rs").unwrap()),
            None
        );
    }

    #[test]
    fn a_tag_with_nothing_after_it_resolves_to_none() {
        let src = "// Verifies: REQ001\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.rs").unwrap()),
            None
        );
    }

    #[test]
    fn python_resolves_a_def_past_a_decorator() {
        let src = "# Verifies: REQ001\n@pytest.fixture\ndef test_login():\n    pass\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.py").unwrap()).as_deref(),
            Some("test_login")
        );
    }

    #[test]
    fn typescript_resolves_a_test_title() {
        let src = "// Verifies: REQ001\nit('logs the user in', () => {});\n";
        assert_eq!(
            symbol_for(&lines(src), 1, language_for("a.ts").unwrap()).as_deref(),
            Some("logs the user in")
        );
    }
}
