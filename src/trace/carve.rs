//! Comment carver — extracts every comment run from a source file per a language's comment
//! grammar, so the tag parser only ever sees comment text. Absorbed from ReqForge's
//! `scan/walker.rs::extract_comment_runs` (and its line/offset helpers), adapted to the
//! static [`Language`] table. Like ReqForge's, it does not tokenise string literals, so a
//! `//`-like sequence inside a string is treated as a comment — tags never appear there and
//! a full per-language tokenizer is not worth the one edge case.
//!
//! Implements: REQ075

use super::languages::Language;
use super::tags::CommentRun;

/// Extract every comment run from a source file. Tolerant: an unmatched block comment runs
/// to end-of-file.
pub fn extract_comment_runs<'a>(source: &'a str, language: &Language) -> Vec<CommentRun<'a>> {
    let mut out: Vec<CommentRun<'a>> = Vec::new();
    let line_starts = compute_line_starts(source);
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if let Some((start, end, starts_line)) = try_block_comment(bytes, i, language, &line_starts)
        {
            let (content_start, content_end, run_start_line) = start;
            out.push(CommentRun {
                text: &source[content_start..content_end],
                line: run_start_line,
                comment_only_lines: starts_line,
            });
            i = end;
            continue;
        }
        if let Some((content_start, eol, line_number, comment_only)) =
            try_line_comment(bytes, i, source, language, &line_starts)
        {
            out.push(CommentRun {
                text: &source[content_start..eol],
                line: line_number,
                comment_only_lines: comment_only,
            });
            i = if eol < bytes.len() { eol + 1 } else { eol };
            continue;
        }
        i += 1;
    }
    out
}

/// The byte position each line starts at. Index 0 is always 0.
fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

/// The 1-based line number containing `pos`.
fn line_number_at(line_starts: &[usize], pos: usize) -> usize {
    match line_starts.binary_search(&pos) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    }
}

/// Offset of the start of the line containing `pos`.
fn line_start_of(line_starts: &[usize], pos: usize) -> usize {
    let idx = match line_starts.binary_search(&pos) {
        Ok(idx) => idx,
        Err(idx) => idx.saturating_sub(1),
    };
    line_starts.get(idx).copied().unwrap_or(0)
}

type BlockMatch = ((usize, usize, usize), usize, bool);

/// Try to open a block comment at `i`, returning `((content_start, content_end,
/// start_line), cursor_after, run_is_comment_only)`.
fn try_block_comment(
    bytes: &[u8],
    i: usize,
    language: &Language,
    line_starts: &[usize],
) -> Option<BlockMatch> {
    for &(open, close) in language.block_comments {
        let open_bytes = open.as_bytes();
        if bytes[i..].starts_with(open_bytes) {
            let content_start = i + open_bytes.len();
            let close_bytes = close.as_bytes();
            let relative_close = find_subslice(&bytes[content_start..], close_bytes);
            let (content_end, cursor_after) = match relative_close {
                Some(pos) => (content_start + pos, content_start + pos + close_bytes.len()),
                None => (bytes.len(), bytes.len()),
            };
            let start_line = line_number_at(line_starts, i);
            let line_start = line_start_of(line_starts, i);
            let comment_only = bytes[line_start..i].iter().all(u8::is_ascii_whitespace);
            return Some((
                (content_start, content_end, start_line),
                cursor_after,
                comment_only,
            ));
        }
    }
    None
}

type LineMatch = (usize, usize, usize, bool);

/// Try to open a line comment at `i`. The longest matching marker wins so Rust's `///`
/// beats `//`.
fn try_line_comment(
    bytes: &[u8],
    i: usize,
    source: &str,
    language: &Language,
    line_starts: &[usize],
) -> Option<LineMatch> {
    let mut best_len = 0usize;
    for &marker in language.line_comments {
        let mb = marker.as_bytes();
        if bytes[i..].starts_with(mb) && mb.len() > best_len {
            best_len = mb.len();
        }
    }
    if best_len == 0 {
        return None;
    }
    let content_start = i + best_len;
    let eol = source[content_start..]
        .find('\n')
        .map(|n| content_start + n)
        .unwrap_or(bytes.len());
    let line_number = line_number_at(line_starts, i);
    let line_start = line_start_of(line_starts, i);
    let comment_only = bytes[line_start..i].iter().all(u8::is_ascii_whitespace);
    Some((content_start, eol, line_number, comment_only))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::super::languages::language_for;
    use super::*;

    #[test]
    fn rust_line_comment_extracted_with_line_number() {
        let src = "fn main() {\n    // Verifies: REQ001\n}\n";
        let runs = extract_comment_runs(src, language_for("a.rs").unwrap());
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].line, 2);
        assert!(runs[0].text.contains("Verifies: REQ001"));
        assert!(runs[0].comment_only_lines);
    }

    #[test]
    fn rust_triple_slash_doc_comment_treated_as_line_comment() {
        let runs = extract_comment_runs(
            "/// Verifies: REQ001\nfn f() {}\n",
            language_for("a.rs").unwrap(),
        );
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Verifies: REQ001"));
    }

    #[test]
    fn rust_block_comment_spans_lines_and_preserves_content() {
        let src = "/*\nImplements: REQ001\nVerifies: REQ002\n*/";
        let runs = extract_comment_runs(src, language_for("a.rs").unwrap());
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Implements: REQ001"));
        assert!(runs[0].text.contains("Verifies: REQ002"));
    }

    #[test]
    fn python_triple_quote_is_a_comment_run_for_tag_scanning() {
        let runs = extract_comment_runs(
            "\"\"\"\nImplements: REQ001\n\"\"\"\n",
            language_for("a.py").unwrap(),
        );
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Implements: REQ001"));
    }

    #[test]
    fn unterminated_block_comment_runs_to_eof() {
        let runs = extract_comment_runs("/*\nImplements: REQ001\n", language_for("a.rs").unwrap());
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Implements: REQ001"));
    }

    #[test]
    fn code_before_line_comment_is_flagged_mixed() {
        let runs = extract_comment_runs(
            "let x = 1; // Implements: REQ001\n",
            language_for("a.rs").unwrap(),
        );
        assert_eq!(runs.len(), 1);
        assert!(!runs[0].comment_only_lines);
    }

    #[test]
    fn shell_recognises_hash_comments() {
        let runs = extract_comment_runs(
            "#!/usr/bin/env bash\n# Verifies: REQ001\necho hi\n",
            language_for("s.sh").unwrap(),
        );
        assert!(runs.iter().any(|r| r.text.contains("Verifies: REQ001")));
    }
}
