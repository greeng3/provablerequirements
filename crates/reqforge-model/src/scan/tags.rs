//! Tag parser for code-traceability scanning
//! (TRACE-codeTagFormat).
//!
//! Tags are recognised only within comments. Each tag takes
//! the form:
//!
//! ```text
//! <Verb>: <id>[, <id>]...
//! ```
//!
//! where `<Verb>` is a canonical built-in link-type name
//! (`Satisfies`, `Verifies`, `Derives-From`, `Supersedes`,
//! `Conflicts-With`, `Related-To`) or an alias (`Implements`
//! and `Requirements` both map to `Satisfies`). A tag may list
//! multiple comma-separated IDs and a trailing comma on a tag
//! line causes the list to continue onto subsequent comment-
//! only lines carrying bare IDs.
//!
//! This module is pure: it parses already-extracted comment
//! text into `RawTag` values. The walker wires it up to a
//! language-aware comment extractor in 9a.2.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The six canonical verbs matching the built-in link-type
/// catalog (`TRACE-linkCatalog`). Aliases canonicalise into
/// one of these before a tag lands in a `RawTag`.
pub const CANONICAL_VERBS: &[&str] = &[
    "Satisfies",
    "Verifies",
    "Derives-From",
    "Supersedes",
    "Conflicts-With",
    "Related-To",
];

/// Alias map: input verb (lowercased, hyphens normalised) →
/// canonical verb. Kept in a `OnceLock` so we pay the setup
/// cost once per process.
fn alias_table() -> &'static HashMap<String, &'static str> {
    static TABLE: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m: HashMap<String, &'static str> = HashMap::new();
        for canonical in CANONICAL_VERBS {
            m.insert(normalise_verb(canonical), *canonical);
        }
        // Aliases per TRACE-codeTagFormat.
        m.insert("implements".to_owned(), "Satisfies");
        m.insert("requirements".to_owned(), "Satisfies");
        m
    })
}

/// Lowercase + strip non-alphanumerics-except-hyphen so
/// `Derives-From`, `derives-from`, `DerivesFrom`,
/// `DERIVES-FROM` all collapse to the same key.
fn normalise_verb(input: &str) -> String {
    input
        .chars()
        .filter_map(|c| {
            if c == '-' {
                Some('-')
            } else if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect::<String>()
        .replace('-', "")
        .chars()
        .collect::<String>()
        .replace(' ', "")
        // At this point we have no hyphens; re-inject the
        // canonical hyphen pattern for the multi-word verbs so
        // keys line up with how we insert them into the table.
        .replace("derivesfrom", "derives-from")
        .replace("conflictswith", "conflicts-with")
        .replace("relatedto", "related-to")
}

/// A single parsed tag. `raw_id` is the source-code form
/// (e.g. `REQ-001`) — the walker resolves it against the
/// mounted projects in 9a.2 to produce an artifact key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTag {
    pub verb: String,
    pub raw_id: String,
    /// 1-based line number within the containing file of the
    /// line that opened this tag (the `Verb:` line, or the
    /// continuation line that carried the ID).
    pub line: usize,
}

/// One contiguous run of comment text carved out of a source
/// file. `starts_on_its_own_line` lets the parser preserve
/// the spec's "subsequent comment-only lines" continuation
/// rule: a bare ID line is only recognised as a continuation
/// when it's the whole content of its own comment.
#[derive(Debug, Clone)]
pub struct CommentRun<'a> {
    pub text: &'a str,
    /// 1-based starting line number of this run in the source
    /// file.
    pub line: usize,
    /// True when the run is composed of lines each of which is
    /// *only* a comment (no preceding code on the line). Line-
    /// comment runs always satisfy this; block-comments too
    /// when they start at column zero.
    pub comment_only_lines: bool,
}

/// Parse tags out of a sequence of comment runs. Multi-line
/// continuation fires only across runs whose lines were
/// comment-only — matching the spec's "subsequent comment-
/// only lines carrying bare IDs" requirement.
pub fn parse_tags(runs: &[CommentRun<'_>]) -> Vec<RawTag> {
    let mut out: Vec<RawTag> = Vec::new();
    // State carried across comment-only lines: the verb that
    // was on the most recent trailing-comma line, along with
    // the flag saying we're expecting more IDs.
    let mut continuation_verb: Option<String> = None;

    for run in runs {
        if continuation_verb.is_some() && !run.comment_only_lines {
            // A non-comment-only run breaks the continuation.
            continuation_verb = None;
        }
        for (offset, line) in run.text.lines().enumerate() {
            parse_tag_line(
                line,
                run.line + offset,
                &mut continuation_verb,
                &mut out,
                run.comment_only_lines,
            );
        }
        // Reset continuation state when a run without a
        // trailing-comma line ends. The `parse_tag_line` inner
        // already clears it on lines that close out the list.
    }
    out
}

fn parse_tag_line(
    line: &str,
    line_number: usize,
    continuation_verb: &mut Option<String>,
    out: &mut Vec<RawTag>,
    comment_only_lines: bool,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        // Blank line inside a comment run — continuations
        // tolerate a bare empty line.
        return;
    }
    // Try to find a `Verb:` prefix.
    if let Some((verb, rest)) = extract_verb(trimmed) {
        let canonical = canonicalise_verb(verb);
        let Some(canonical) = canonical else {
            // Unknown verb — drop the line.
            *continuation_verb = None;
            return;
        };
        let (ids, trailing_comma) = split_ids(rest);
        for raw_id in ids {
            out.push(RawTag {
                verb: canonical.to_owned(),
                raw_id,
                line: line_number,
            });
        }
        *continuation_verb = if trailing_comma && comment_only_lines {
            Some(canonical.to_owned())
        } else {
            None
        };
        return;
    }
    // No verb on this line — maybe we're in a continuation.
    if let Some(verb) = continuation_verb.clone() {
        let (ids, trailing_comma) = split_ids(trimmed);
        if ids.is_empty() {
            // Continuation ended without any ID — treat the
            // line as terminating state.
            *continuation_verb = None;
            return;
        }
        for raw_id in ids {
            out.push(RawTag {
                verb: verb.clone(),
                raw_id,
                line: line_number,
            });
        }
        if !trailing_comma {
            *continuation_verb = None;
        }
    }
}

fn extract_verb(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':')?;
    let (head, rest) = line.split_at(colon);
    // The verb has to be a contiguous token — no spaces — so
    // we reject a line like `See: foo, bar, Satisfies: REQ-1`
    // from matching as a tag. The verb is the last whitespace-
    // separated token of `head`.
    let token = head.split_whitespace().next_back()?;
    // The characters between `token` and the colon must be
    // nothing (so `Satisfies:` yields token = `Satisfies` and
    // rest starts at `: ...`). If there's leading text before
    // the verb token, ensure it's all whitespace.
    let verb_end_byte = head
        .rfind(token)
        .expect("token came from head, must be found");
    if verb_end_byte + token.len() != head.len() {
        return None;
    }
    if head[..verb_end_byte].trim_end().is_empty()
        || head[..verb_end_byte].chars().all(char::is_whitespace)
    {
        // Ensure the non-whitespace prefix of the line is
        // just our token. This rejects "foo Bar Satisfies:"
        // while accepting `   * Satisfies:` (the `*` in JS
        // block-comments) — the `*` is whitespace-ish if we
        // allowed it, but we don't: the leading prefix must
        // be pure whitespace. Per the spec the tag can only
        // be the "whole" comment line, discounting language-
        // specific leading markers which are stripped by the
        // comment extractor before tag parsing.
        Some((token, &rest[1..]))
    } else {
        None
    }
}

/// Canonicalise a verb name (case-insensitive, alias-aware).
/// Returns `None` when the input doesn't name a known verb.
pub fn canonicalise_verb(verb: &str) -> Option<&'static str> {
    alias_table().get(&normalise_verb(verb)).copied()
}

/// Split a comma-separated ID list into owned `String`s plus
/// a flag indicating whether the input ended on a trailing
/// comma (modulo whitespace). Empty components are dropped.
fn split_ids(input: &str) -> (Vec<String>, bool) {
    let trimmed = input.trim_end();
    let trailing_comma = trimmed.ends_with(',');
    let mut out: Vec<String> = Vec::new();
    for part in trimmed.trim_end_matches(',').split(',') {
        let id = part.trim();
        if id.is_empty() {
            continue;
        }
        if !is_plausible_id(id) {
            continue;
        }
        out.push(id.to_owned());
    }
    (out, trailing_comma)
}

/// A plausible ReqForge artifact ID matches the
/// `<prefix>-<artifactName>` form that `ART-artifactName` uses
/// throughout the codebase: alphanumeric + underscore + hyphen,
/// non-empty, contains at least one hyphen (to distinguish
/// prefix from name).
fn is_plausible_id(s: &str) -> bool {
    if !s.contains('-') {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(text: &str) -> Vec<RawTag> {
        parse_tags(&[CommentRun {
            text,
            line: 1,
            comment_only_lines: true,
        }])
    }

    #[test]
    fn canonicalise_handles_casing_hyphens_and_aliases() {
        assert_eq!(canonicalise_verb("Satisfies"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("satisfies"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("SATISFIES"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("Derives-From"), Some("Derives-From"));
        assert_eq!(canonicalise_verb("derives-from"), Some("Derives-From"));
        assert_eq!(canonicalise_verb("DerivesFrom"), Some("Derives-From"));
        assert_eq!(canonicalise_verb("Implements"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("requirements"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("Bogus"), None);
    }

    #[test]
    fn single_tag_single_id() {
        let tags = single("Satisfies: REQ-001\n");
        assert_eq!(
            tags,
            vec![RawTag {
                verb: "Satisfies".into(),
                raw_id: "REQ-001".into(),
                line: 1,
            }]
        );
    }

    #[test]
    fn single_tag_multiple_ids() {
        let tags = single("Verifies: TEST-001, TEST-002, TEST-003\n");
        assert_eq!(tags.len(), 3);
        assert!(tags.iter().all(|t| t.verb == "Verifies"));
        let ids: Vec<&str> = tags.iter().map(|t| t.raw_id.as_str()).collect();
        assert_eq!(ids, vec!["TEST-001", "TEST-002", "TEST-003"]);
    }

    #[test]
    fn trailing_comma_continues_across_lines() {
        let tags = single("Satisfies: REQ-001,\nREQ-002,\nREQ-003\n");
        assert_eq!(tags.len(), 3);
        assert!(tags.iter().all(|t| t.verb == "Satisfies"));
        let ids: Vec<&str> = tags.iter().map(|t| t.raw_id.as_str()).collect();
        assert_eq!(ids, vec!["REQ-001", "REQ-002", "REQ-003"]);
    }

    #[test]
    fn continuation_stops_when_list_closes() {
        // The second line is a fresh `Verifies:` tag, not a
        // continuation of the first — the first list closed
        // with no trailing comma.
        let tags = single("Satisfies: REQ-001\nVerifies: TEST-001\n");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].verb, "Satisfies");
        assert_eq!(tags[1].verb, "Verifies");
    }

    #[test]
    fn continuation_breaks_on_non_comment_only_run() {
        // Two runs: first ends with a trailing comma (so the
        // continuation would otherwise carry); second run is
        // NOT comment-only (simulating a code line) so the
        // state machine drops the continuation before
        // processing any further lines.
        let tags = parse_tags(&[
            CommentRun {
                text: "Satisfies: REQ-001,\n",
                line: 1,
                comment_only_lines: true,
            },
            CommentRun {
                text: "REQ-002\n",
                line: 5,
                comment_only_lines: false,
            },
        ]);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ-001");
    }

    #[test]
    fn implements_is_canonicalised_to_satisfies() {
        let tags = single("Implements: REQ-42\n");
        assert_eq!(
            tags,
            vec![RawTag {
                verb: "Satisfies".into(),
                raw_id: "REQ-42".into(),
                line: 1,
            }]
        );
    }

    #[test]
    fn bare_id_outside_a_continuation_is_ignored() {
        // No preceding `Verb:` so the line carries no known
        // intent.
        let tags = single("REQ-001\n");
        assert!(tags.is_empty());
    }

    #[test]
    fn unknown_verb_line_is_ignored_and_kills_any_continuation() {
        let tags = single("Satisfies: REQ-001,\nBogus: REQ-002\n");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ-001");
    }

    #[test]
    fn line_numbers_track_each_tag_position() {
        // The run starts at line 10. A multi-line continuation
        // emits one tag per ID with the right offset.
        let tags = parse_tags(&[CommentRun {
            text: "Satisfies: REQ-001,\nREQ-002\n",
            line: 10,
            comment_only_lines: true,
        }]);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].line, 10);
        assert_eq!(tags[1].line, 11);
    }

    #[test]
    fn plausible_ids_are_filtered() {
        // A bare number isn't a valid ID (no hyphen).
        let tags = single("Satisfies: 001, REQ-001\n");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ-001");
    }

    #[test]
    fn ids_with_underscores_are_accepted() {
        // Per INTEROP-doorstopIdNormalization, multi-word
        // NANUs use underscores; the scanner must accept them.
        let tags = single("Satisfies: DES-rocket_nozzle\n");
        assert_eq!(tags[0].raw_id, "DES-rocket_nozzle");
    }
}
