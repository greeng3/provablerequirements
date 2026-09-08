//! Rename-suggestion prompt builder + response parser (Phase 10b).
//!
//! Given an artifact + its collection siblings, [`build_prompt`]
//! produces a generic `PromptRequest` that the 10a chain can hand
//! to any adapter family. [`parse_suggestions`] turns the model's
//! free-form text back into validated [`Suggestion`] records,
//! dropping lines that fail the filename regex or duplicate the
//! current name.
//!
//! Kept as a pure-compute module so unit tests can cover the
//! prompt shape and parser without spinning up an adapter; the
//! integration layer in `http::handlers` glues this to
//! `LlmRuntime::run_prompt`.

use serde::Serialize;

use crate::llm::{PromptMessage, PromptRequest, PromptRole};

const MAX_STYLE_ANCHORS: usize = 8;
const SUGGESTION_COUNT: usize = 3;
const MAX_NAME_LEN: usize = 64;

/// One accepted rename candidate produced by the LLM.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub name: String,
    pub rationale: String,
}

/// Input to [`build_prompt`] — the bits the prompt needs to
/// anchor on. Intentionally a simple POD so the handler doesn't
/// have to expose `LoadedCollection` / `LoadedArtifact` shapes
/// across module boundaries.
#[derive(Debug, Clone)]
pub struct PromptInput<'a> {
    pub collection_prefix: &'a str,
    pub current_name: &'a str,
    pub current_title: &'a str,
    /// Filename stems of sibling artifacts in the same
    /// collection. Used as style anchors so the LLM mirrors the
    /// project's established naming convention; truncated to
    /// `MAX_STYLE_ANCHORS` entries.
    pub sibling_names: &'a [String],
}

/// Build the generic `PromptRequest` for a single-artifact rename
/// suggestion. `max_tokens`/`temperature` pinned to values from
/// the ROADMAP locked decision.
pub fn build_prompt(input: &PromptInput<'_>) -> PromptRequest {
    let anchors: Vec<&str> = input
        .sibling_names
        .iter()
        .filter(|n| n.as_str() != input.current_name)
        .take(MAX_STYLE_ANCHORS)
        .map(String::as_str)
        .collect();

    let mut system = String::new();
    system.push_str("You suggest filenames for artifacts in a requirements management system.\n\n");
    system.push_str("Filename rules:\n");
    system.push_str("- Alphanumeric + dot/underscore/hyphen only (regex [A-Za-z0-9._\\-]+).\n");
    system.push_str(&format!(
        "- Prefixed with the collection prefix '{}'.\n",
        input.collection_prefix
    ));
    system.push_str("- Kebab-case, no spaces, under 64 characters.\n");
    system.push_str("- Descriptive but concise; mirror the existing style.\n");
    if !anchors.is_empty() {
        system.push_str("\nStyle anchors from this collection:\n");
        for anchor in &anchors {
            system.push_str("- ");
            system.push_str(anchor);
            system.push('\n');
        }
    }

    let mut user = String::new();
    user.push_str("Suggest exactly ");
    user.push_str(&SUGGESTION_COUNT.to_string());
    user.push_str(" alternative filenames for this artifact.\n\n");
    user.push_str(&format!("Current filename: {}\n", input.current_name));
    user.push_str(&format!("Title: {}\n\n", input.current_title));
    user.push_str("Output format: one suggestion per line, exactly:\n");
    user.push_str("name — rationale\n\n");
    user.push_str(
        "No numbered list, no preamble, no trailing text. The em-dash character (—) \
         is the separator. Rationale must be under 12 words.",
    );

    PromptRequest {
        system: Some(system),
        messages: vec![PromptMessage {
            role: PromptRole::User,
            content: user,
        }],
        max_tokens: 200,
        temperature: 0.2,
        timeout_ms: None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("LLM produced no usable suggestions")]
    NoSuggestions,
}

/// Parse the model's response into up to `SUGGESTION_COUNT`
/// validated suggestions. Dedupes against `current_name` (no
/// point suggesting the identity rename) and against itself.
pub fn parse_suggestions(text: &str, current_name: &str) -> Result<Vec<Suggestion>, ParseError> {
    let mut out: Vec<Suggestion> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name_raw, rationale_raw)) = split_name_rationale(line) else {
            continue;
        };
        let name = strip_leading_markers(name_raw);
        if !is_valid_stem(&name) || name.len() > MAX_NAME_LEN {
            continue;
        }
        if name == current_name {
            continue;
        }
        if out.iter().any(|s| s.name == name) {
            continue;
        }
        let rationale = rationale_raw.trim().to_owned();
        if rationale.is_empty() {
            continue;
        }
        out.push(Suggestion { name, rationale });
        if out.len() == SUGGESTION_COUNT {
            break;
        }
    }
    if out.is_empty() {
        return Err(ParseError::NoSuggestions);
    }
    Ok(out)
}

fn split_name_rationale(line: &str) -> Option<(&str, &str)> {
    // Preferred separator: em-dash with surrounding spaces.
    // Fall back to a plain hyphen because smaller models
    // sometimes decline to emit the em-dash glyph.
    if let Some((l, r)) = line.split_once(" — ") {
        return Some((l.trim(), r));
    }
    if let Some((l, r)) = line.split_once(" - ") {
        return Some((l.trim(), r));
    }
    if let Some((l, r)) = line.split_once(": ") {
        return Some((l.trim(), r));
    }
    None
}

fn strip_leading_markers(s: &str) -> String {
    // Strip a leading numbering like "1. ", "2) ", or a bullet.
    let s = s.trim();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && matches!(bytes[i], b'.' | b')') {
        i += 1;
        while i < bytes.len() && bytes[i] == b' ' {
            i += 1;
        }
        return s[i..].trim().to_owned();
    }
    if let Some(rest) = s.strip_prefix("- ") {
        return rest.trim().to_owned();
    }
    if let Some(rest) = s.strip_prefix("* ") {
        return rest.trim().to_owned();
    }
    s.to_owned()
}

fn is_valid_stem(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(current_name: &'a str, siblings: &'a [String]) -> PromptInput<'a> {
        PromptInput {
            collection_prefix: "REQ",
            current_name,
            current_title: "Some requirement",
            sibling_names: siblings,
        }
    }

    #[test]
    fn prompt_references_collection_prefix_and_filename_regex() {
        let siblings = vec!["REQ-a".to_owned(), "REQ-b".to_owned()];
        let req = build_prompt(&input("REQ-old-name", &siblings));
        let system = req.system.as_deref().unwrap();
        assert!(system.contains("collection prefix 'REQ'"));
        assert!(system.contains("[A-Za-z0-9._\\-]+"));
        assert!(system.contains("REQ-a"));
        assert!(system.contains("REQ-b"));
    }

    #[test]
    fn prompt_excludes_current_name_from_style_anchors() {
        let siblings = vec!["REQ-old-name".to_owned(), "REQ-other".to_owned()];
        let req = build_prompt(&input("REQ-old-name", &siblings));
        let system = req.system.as_deref().unwrap();
        assert!(system.contains("REQ-other"));
        // The current name shouldn't feature as a "style
        // anchor" — the model is trying to get away from it.
        assert!(!system.contains("- REQ-old-name"));
    }

    #[test]
    fn prompt_caps_style_anchors_at_eight() {
        let siblings: Vec<String> = (0..20).map(|i| format!("REQ-{i}")).collect();
        let req = build_prompt(&input("REQ-current", &siblings));
        let system = req.system.as_deref().unwrap();
        let count = (0..20)
            .filter(|i| system.contains(&format!("- REQ-{i}\n")))
            .count();
        assert_eq!(count, MAX_STYLE_ANCHORS);
    }

    #[test]
    fn prompt_carries_user_side_instructions_and_title() {
        let req = build_prompt(&input("REQ-x", &[]));
        let user = &req.messages[0].content;
        assert!(user.contains("Suggest exactly 3"));
        assert!(user.contains("Current filename: REQ-x"));
        assert!(user.contains("Title: Some requirement"));
    }

    #[test]
    fn parse_accepts_emdash_separated_lines() {
        let text = "\
REQ-valve-sizing — matches sibling REQ-pressure-envelope style
REQ-valve-selection — compact synonym
REQ-valve-specification — longer form";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].name, "REQ-valve-sizing");
        assert!(out[0].rationale.starts_with("matches sibling"));
    }

    #[test]
    fn parse_accepts_plain_hyphen_fallback_separator() {
        let text = "REQ-a - one\nREQ-b - two\nREQ-c - three";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(
            out.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["REQ-a", "REQ-b", "REQ-c"]
        );
    }

    #[test]
    fn parse_strips_numbered_list_and_bullet_prefixes() {
        let text = "\
1. REQ-first — why first
2) REQ-second — why second
- REQ-third — why third";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(
            out.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["REQ-first", "REQ-second", "REQ-third"]
        );
    }

    #[test]
    fn parse_drops_invalid_stems_silently() {
        let text = "\
REQ-ok — valid
REQ has spaces — invalid stem
R@Q-special — invalid char
REQ-fine — also valid";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "REQ-ok");
        assert_eq!(out[1].name, "REQ-fine");
    }

    #[test]
    fn parse_dedupes_against_current_name() {
        let text = "\
REQ-current — identity rename is useless
REQ-new — the real suggestion
REQ-other — another one";
        let out = parse_suggestions(text, "REQ-current").unwrap();
        assert_eq!(out.len(), 2);
        assert!(!out.iter().any(|s| s.name == "REQ-current"));
    }

    #[test]
    fn parse_dedupes_within_suggestions() {
        let text = "\
REQ-a — first
REQ-a — duplicate
REQ-b — third line saves us";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(
            out.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["REQ-a", "REQ-b"]
        );
    }

    #[test]
    fn parse_errors_when_zero_valid_lines() {
        let text = "\
nothing usable here
or here
or on this line either";
        assert!(matches!(
            parse_suggestions(text, "REQ-x"),
            Err(ParseError::NoSuggestions)
        ));
    }

    #[test]
    fn parse_caps_at_three_even_when_more_offered() {
        let text = "\
REQ-a — one
REQ-b — two
REQ-c — three
REQ-d — four";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(out.len(), SUGGESTION_COUNT);
        assert_eq!(out.iter().last().unwrap().name, "REQ-c");
    }

    #[test]
    fn parse_requires_non_empty_rationale() {
        let text = "\
REQ-a — \nREQ-b — real rationale\nREQ-c — another";
        let out = parse_suggestions(text, "REQ-old").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "REQ-b");
    }

    #[test]
    fn parse_rejects_over_long_stem() {
        let long = "REQ-".to_string() + &"x".repeat(200);
        let text = format!("{long} — too long\nREQ-short — fine");
        let out = parse_suggestions(&text, "REQ-old").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "REQ-short");
    }
}
