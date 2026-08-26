//! Tag parser — parses `Implements:` / `Verifies:` tags out of already-carved comment
//! text. Absorbed from ReqForge's `scan/tags.rs` (its comment-only continuation rule and
//! verb-alias table came across intact), with one load-bearing adaptation: provreq's id
//! grammar does not require a hyphen, so `REQ021` is a valid id and not only `REQ-021`
//! (ReqForge's `is_plausible_id` demanded one, which would have dropped every provreq tag).
//!
//! Pure: it parses [`CommentRun`] values into [`RawTag`]s and reads no files.
//!
//! Implements: REQ075

use std::collections::HashMap;
use std::sync::OnceLock;

/// The six canonical verbs from ReqForge's link catalog. provreq only *acts* on the two
/// that speak about code (`Satisfies`/`Verifies` → [`super::TraceKind`]); the parser still
/// recognises all six so a mixed comment does not mis-parse, and the caller drops the rest.
pub const CANONICAL_VERBS: &[&str] = &[
    "Satisfies",
    "Verifies",
    "Derives-From",
    "Supersedes",
    "Conflicts-With",
    "Related-To",
];

fn alias_table() -> &'static HashMap<String, &'static str> {
    static TABLE: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m: HashMap<String, &'static str> = HashMap::new();
        for canonical in CANONICAL_VERBS {
            m.insert(normalise_verb(canonical), *canonical);
        }
        // `Implements:` and `Requirements:` are the spellings provreq's own tree uses; both
        // read as `Satisfies`.
        m.insert("implements".to_owned(), "Satisfies");
        m.insert("requirements".to_owned(), "Satisfies");
        m
    })
}

/// Lowercase + drop non-alphanumerics-except-hyphen so `Derives-From`, `derives-from`,
/// `DerivesFrom`, `DERIVES-FROM` all collapse to one key.
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
        .replace("derivesfrom", "derives-from")
        .replace("conflictswith", "conflicts-with")
        .replace("relatedto", "related-to")
}

/// A single parsed tag. `raw_id` is the source-code form as written (e.g. `REQ021`);
/// reconciling it against the subject's declared requirements is a downstream concern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTag {
    pub verb: String,
    pub raw_id: String,
    /// 1-based line of the `Verb:` line, or the continuation line carrying the id.
    pub line: usize,
}

/// One contiguous run of comment text carved from a source file.
#[derive(Debug, Clone)]
pub struct CommentRun<'a> {
    pub text: &'a str,
    /// 1-based starting line of this run in the source file.
    pub line: usize,
    /// True when every line of the run is *only* a comment (no preceding code). The
    /// continuation rule ("subsequent comment-only lines carrying bare ids") only fires
    /// across such runs.
    pub comment_only_lines: bool,
}

/// Parse tags out of a sequence of comment runs. Multi-line continuation fires only across
/// comment-only runs, matching ReqForge's spec.
pub fn parse_tags(runs: &[CommentRun<'_>]) -> Vec<RawTag> {
    let mut out: Vec<RawTag> = Vec::new();
    let mut continuation_verb: Option<String> = None;

    for run in runs {
        if continuation_verb.is_some() && !run.comment_only_lines {
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
        return;
    }
    if let Some((verb, rest)) = extract_verb(trimmed) {
        let Some(canonical) = canonicalise_verb(verb) else {
            // Unknown verb — drop the line and any continuation it might have joined.
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
    if let Some(verb) = continuation_verb.clone() {
        let (ids, trailing_comma) = split_ids(trimmed);
        if ids.is_empty() {
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
    let token = head.split_whitespace().next_back()?;
    let verb_end_byte = head
        .rfind(token)
        .expect("token came from head, must be found");
    if verb_end_byte + token.len() != head.len() {
        return None;
    }
    if head[..verb_end_byte].trim_end().is_empty()
        || head[..verb_end_byte].chars().all(char::is_whitespace)
    {
        // The non-whitespace prefix must be only our token — rejects `foo Bar Satisfies:`
        // while accepting `   * Satisfies:` once the carver has stripped the `*`.
        Some((token, &rest[1..]))
    } else {
        None
    }
}

/// Canonicalise a verb name (case-insensitive, alias-aware). `None` when it names no verb.
pub fn canonicalise_verb(verb: &str) -> Option<&'static str> {
    alias_table().get(&normalise_verb(verb)).copied()
}

/// Split an id list into owned ids plus whether it ended on a trailing comma. Splits on
/// commas **and** whitespace — provreq's real tags carry trailing prose on the same line
/// (`Verifies: REQ021 — the bindable symbols…`), and ReqForge's comma-only split swallowed
/// the id into the prose. Non-id-shaped tokens (the prose) are dropped; the caller still
/// prefix-filters what survives (see [`id_prefix`]).
fn split_ids(input: &str) -> (Vec<String>, bool) {
    let trimmed = input.trim_end();
    let trailing_comma = trimmed.ends_with(',');
    let out: Vec<String> = trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| is_id_shaped(t))
        .map(str::to_owned)
        .collect();
    (out, trailing_comma)
}

/// Whether a token has the shape of a requirement id: `<letters><optional - or _><digits>`
/// — `REQ021`, `REQ-001`, `req_001`, `category-1`. This drops prose (a word with no trailing
/// digits) but cannot tell a real id from a coincidence like `category-1`; the prefix filter
/// does that. Deliberately narrower than ReqForge's (which required a hyphen and would have
/// dropped every provreq id) and than doorstop name-NANUs (`DES-rocket_nozzle`), which are an
/// artifact-id form, not a code-tag form — code tags name `prefix+number` ids.
fn is_id_shaped(s: &str) -> bool {
    let rest = s.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    if rest.len() == s.len() {
        return false; // no leading letters
    }
    let digits = rest.strip_prefix(['-', '_']).unwrap_or(rest);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The alphabetic prefix of an id, upper-cased — `REQ021` → `REQ`, `category-1` → `CATEGORY`.
/// The caller keeps only tags whose prefix names a real requirement collection, which is how
/// a coincidental `category-1` in prose is told from a real id (mirrors the Python reader's
/// `valid_prefixes`).
pub fn id_prefix(id: &str) -> String {
    id.chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase()
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

    // The verb-alias table came across from ReqForge unchanged.
    #[test]
    fn canonicalise_handles_casing_hyphens_and_aliases() {
        assert_eq!(canonicalise_verb("Verifies"), Some("Verifies"));
        assert_eq!(canonicalise_verb("verifies"), Some("Verifies"));
        assert_eq!(canonicalise_verb("Derives-From"), Some("Derives-From"));
        assert_eq!(canonicalise_verb("DerivesFrom"), Some("Derives-From"));
        assert_eq!(canonicalise_verb("Implements"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("requirements"), Some("Satisfies"));
        assert_eq!(canonicalise_verb("Bogus"), None);
    }

    // The adaptation this port turns on: a hyphenless id is id-shaped, unlike ReqForge's
    // hyphen-required rule that would have dropped every provreq tag.
    #[test]
    fn hyphenless_provreq_ids_are_id_shaped() {
        assert!(is_id_shaped("REQ021"));
        assert!(is_id_shaped("QRUS042"));
        assert!(is_id_shaped("REQ-001"));
        assert!(is_id_shaped("req_001"));
        // Prose, bare numbers, and prefix-only tokens are not ids.
        assert!(!is_id_shaped("Something"));
        assert!(!is_id_shaped("001"));
        assert!(!is_id_shaped("REQ")); // no trailing digits
                                       // A doorstop name-NANU is an artifact-id form, not a code-tag form.
        assert!(!is_id_shaped("DES-rocket_nozzle"));
    }

    #[test]
    fn id_prefix_is_the_upper_cased_letter_run() {
        assert_eq!(id_prefix("REQ021"), "REQ");
        assert_eq!(id_prefix("req_001"), "REQ");
        assert_eq!(id_prefix("category-1"), "CATEGORY");
    }

    // provreq's real tag shape: an id followed by an em-dash and prose on the same line.
    #[test]
    fn trailing_prose_after_an_id_is_dropped() {
        let tags = single("Verifies: REQ021 — the bindable symbols are exactly the declared\n");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ021");
    }

    #[test]
    fn single_tag_single_id() {
        assert_eq!(
            single("Verifies: REQ021\n"),
            vec![RawTag {
                verb: "Verifies".into(),
                raw_id: "REQ021".into(),
                line: 1,
            }]
        );
    }

    #[test]
    fn single_tag_multiple_ids() {
        let ids: Vec<String> = single("Verifies: REQ021, REQ022, REQ023\n")
            .into_iter()
            .map(|t| t.raw_id)
            .collect();
        assert_eq!(ids, vec!["REQ021", "REQ022", "REQ023"]);
    }

    #[test]
    fn trailing_comma_continues_across_lines() {
        let ids: Vec<String> = single("Implements: REQ001,\nREQ002,\nREQ003\n")
            .into_iter()
            .map(|t| t.raw_id)
            .collect();
        assert_eq!(ids, vec!["REQ001", "REQ002", "REQ003"]);
    }

    #[test]
    fn continuation_breaks_on_non_comment_only_run() {
        let tags = parse_tags(&[
            CommentRun {
                text: "Implements: REQ001,\n",
                line: 1,
                comment_only_lines: true,
            },
            CommentRun {
                text: "REQ002\n",
                line: 5,
                comment_only_lines: false,
            },
        ]);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ001");
    }

    #[test]
    fn implements_is_canonicalised_to_satisfies() {
        assert_eq!(single("Implements: REQ042\n")[0].verb, "Satisfies");
    }

    #[test]
    fn bare_id_outside_a_continuation_is_ignored() {
        assert!(single("REQ001\n").is_empty());
    }

    #[test]
    fn unknown_verb_line_is_ignored_and_kills_any_continuation() {
        let tags = single("Implements: REQ001,\nBogus: REQ002\n");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].raw_id, "REQ001");
    }

    #[test]
    fn line_numbers_track_each_tag_position() {
        let tags = parse_tags(&[CommentRun {
            text: "Implements: REQ001,\nREQ002\n",
            line: 10,
            comment_only_lines: true,
        }]);
        assert_eq!(tags[0].line, 10);
        assert_eq!(tags[1].line, 11);
    }
}
