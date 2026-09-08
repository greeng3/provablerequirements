//! File walker + comment extractor (Phase 9a.2).
//!
//! Given a resolved `SystemLanguage` set + a list of scan-
//! root directories, the walker descends recursively,
//! skipping the hard-coded ignore directories, matches each
//! file against the language registry, extracts comment
//! content, and runs the tag parser over it.
//!
//! The extractor is deliberately simple: it doesn't parse
//! string literals, so a `//` sequence inside a Rust string
//! literal is technically recognised as a comment. In
//! practice tags don't appear in string literals; the spec
//! explicitly limits tag recognition to comments, and the
//! cost of a full tokenizer per language isn't worth paying
//! for the one edge case.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::scan::config::ignore_dirs;
use crate::scan::languages::SystemLanguage;
use crate::scan::tags::{CommentRun, RawTag, parse_tags};

/// One match of a scanned file against the language registry
/// — the pair the walker carries around while processing.
struct ScanTarget<'a> {
    path: PathBuf,
    language: &'a SystemLanguage,
}

/// Raw tag with its source file attached, before cross-
/// project resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTag {
    pub file: PathBuf,
    pub line: usize,
    pub verb: String,
    pub raw_id: String,
}

/// Per-file error captured during the walk. We don't let one
/// unreadable file fail the whole scan — 9b's report surfaces
/// the failure list so operators can fix the offending files
/// piecemeal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileScanError {
    pub file: PathBuf,
    pub message: String,
}

/// Result of walking one project's scan roots. File counts +
/// per-file errors are carried alongside the raw tags so 9b
/// can build the final report.
#[derive(Debug, Clone, Default)]
pub struct WalkOutput {
    pub tags: Vec<FileTag>,
    pub scanned_file_count: usize,
    pub file_errors: Vec<FileScanError>,
}

/// Walk the given scan roots, matching files against the
/// language registry, and collect every tag.
pub fn walk_scan_roots(roots: &[PathBuf], languages: &[SystemLanguage]) -> WalkOutput {
    let mut out = WalkOutput::default();
    let ignore: &HashSet<&'static str> = ignore_dirs();
    for root in roots {
        visit_dir(root, languages, ignore, &mut out);
    }
    // Stable ordering for downstream determinism — sort by
    // (file, line) so the UI sees a deterministic tag list
    // across runs.
    out.tags.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.verb.cmp(&b.verb))
            .then(a.raw_id.cmp(&b.raw_id))
    });
    out
}

fn visit_dir(
    dir: &Path,
    languages: &[SystemLanguage],
    ignore: &HashSet<&'static str>,
    out: &mut WalkOutput,
) {
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            out.file_errors.push(FileScanError {
                file: dir.to_path_buf(),
                message: format!("read_dir: {err}"),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if ignore.contains(name) {
                continue;
            }
            visit_dir(&path, languages, ignore, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let Some(language) = languages.iter().find(|l| l.matches_file(file_name)) else {
            continue;
        };
        scan_file(
            ScanTarget {
                path: path.clone(),
                language,
            },
            out,
        );
    }
}

fn scan_file(target: ScanTarget<'_>, out: &mut WalkOutput) {
    let content = match fs::read_to_string(&target.path) {
        Ok(s) => s,
        Err(err) => {
            out.file_errors.push(FileScanError {
                file: target.path.clone(),
                message: format!("read_to_string: {err}"),
            });
            return;
        }
    };
    out.scanned_file_count += 1;
    let runs = extract_comment_runs(&content, target.language);
    let tags = parse_tags(&runs);
    for RawTag { verb, raw_id, line } in tags {
        out.tags.push(FileTag {
            file: target.path.clone(),
            line,
            verb,
            raw_id,
        });
    }
}

/// Extract every comment run from a source file. Tolerant:
/// unmatched block comments run to the end of the file.
pub fn extract_comment_runs<'a>(source: &'a str, language: &SystemLanguage) -> Vec<CommentRun<'a>> {
    let mut out: Vec<CommentRun<'a>> = Vec::new();
    let line_starts = compute_line_starts(source);
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // Try to open a block comment.
        if let Some((start, end, starts_line)) = try_block_comment(bytes, i, language, &line_starts)
        {
            let (content_start, content_end, run_start_line) = start;
            // `content_start` .. `content_end` is the comment
            // content — the markers themselves are excluded so
            // a block-comment marker like `"""` doesn't hit the
            // tag parser as literal text.
            let text: &str = &source[content_start..content_end];
            out.push(CommentRun {
                text,
                line: run_start_line,
                comment_only_lines: starts_line,
            });
            i = end;
            continue;
        }
        // Try to open a line comment on the current line.
        if let Some((content_start, eol, line_number, comment_only)) =
            try_line_comment(bytes, i, source, language, &line_starts)
        {
            let text: &str = &source[content_start..eol];
            out.push(CommentRun {
                text,
                line: line_number,
                comment_only_lines: comment_only,
            });
            // Advance past the newline (or end-of-file).
            i = if eol < bytes.len() { eol + 1 } else { eol };
            continue;
        }
        i += 1;
    }
    out
}

/// Find the byte position where each line starts. Index 0 is
/// always 0; subsequent entries are the byte index just after
/// each newline.
fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

/// Return the 1-based line number containing `pos`. Pure
/// binary search over `line_starts`.
fn line_number_at(line_starts: &[usize], pos: usize) -> usize {
    match line_starts.binary_search(&pos) {
        Ok(idx) => idx + 1,
        Err(idx) => idx, // `idx` is the count of entries ≤ pos
    }
}

/// Offset of the start of the line that contains `pos`.
fn line_start_of(line_starts: &[usize], pos: usize) -> usize {
    let idx = match line_starts.binary_search(&pos) {
        Ok(idx) => idx,
        Err(idx) => idx.saturating_sub(1),
    };
    line_starts.get(idx).copied().unwrap_or(0)
}

type BlockMatch = ((usize, usize, usize), usize, bool);

/// Try to open a block comment at `i`. Returns
/// `((content_start, content_end, start_line), cursor_after,
/// run_is_comment_only)` when successful.
fn try_block_comment(
    bytes: &[u8],
    i: usize,
    language: &SystemLanguage,
    line_starts: &[usize],
) -> Option<BlockMatch> {
    for (open, close) in &language.block_comments {
        let open_bytes = open.as_bytes();
        if bytes[i..].starts_with(open_bytes) {
            let content_start = i + open_bytes.len();
            let close_bytes = close.as_bytes();
            // Scan forward for the matching close marker.
            // Unmatched-close = comment runs to EOF.
            let search_area = &bytes[content_start..];
            let relative_close = find_subslice(search_area, close_bytes);
            let (content_end, cursor_after) = match relative_close {
                Some(pos) => (content_start + pos, content_start + pos + close_bytes.len()),
                None => (bytes.len(), bytes.len()),
            };
            let start_line = line_number_at(line_starts, i);
            // Is the block comment the first non-whitespace on
            // its line? If so, all the lines it spans are
            // comment-only for the purpose of the continuation
            // rule.
            let line_start = line_start_of(line_starts, i);
            let prefix = &bytes[line_start..i];
            let comment_only = prefix.iter().all(|b| b.is_ascii_whitespace());
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

/// Try to open a line comment at `i`. Returns
/// `(content_start, end_of_line, line_number, comment_only)`.
/// The longest matching marker wins so Rust's `///` beats
/// `//`.
fn try_line_comment(
    bytes: &[u8],
    i: usize,
    source: &str,
    language: &SystemLanguage,
    line_starts: &[usize],
) -> Option<LineMatch> {
    // Find the longest matching line-comment marker at `i`.
    let mut best_len = 0usize;
    for marker in &language.line_comments {
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
    let prefix = &bytes[line_start..i];
    let comment_only = prefix.iter().all(|b| b.is_ascii_whitespace());
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
    use super::*;
    use crate::scan::languages::BUILTIN_LANGUAGES;

    fn lang(name: &str) -> SystemLanguage {
        let b = BUILTIN_LANGUAGES.iter().find(|l| l.name == name).unwrap();
        SystemLanguage::from_builtin(b)
    }

    #[test]
    fn rust_line_comment_extracted_with_line_number() {
        let src = "fn main() {\n    // Satisfies: REQ-001\n}\n";
        let runs = extract_comment_runs(src, &lang("Rust"));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].line, 2);
        assert!(runs[0].text.contains("Satisfies: REQ-001"));
        // The comment is not the full line — leading whitespace
        // + `//` + space precedes it — so comment_only_lines is
        // false for continuation purposes.
        // Actually leading whitespace is allowed, per the
        // comment_only definition.
        assert!(runs[0].comment_only_lines);
    }

    #[test]
    fn rust_triple_slash_doc_comment_treated_as_line_comment() {
        let src = "/// Verifies: TEST-001\nfn f() {}\n";
        let runs = extract_comment_runs(src, &lang("Rust"));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Verifies: TEST-001"));
    }

    #[test]
    fn rust_block_comment_spans_lines_and_preserves_content() {
        let src = "/*\nSatisfies: REQ-001\nVerifies: TEST-001\n*/";
        let runs = extract_comment_runs(src, &lang("Rust"));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Satisfies: REQ-001"));
        assert!(runs[0].text.contains("Verifies: TEST-001"));
    }

    #[test]
    fn python_triple_quote_is_a_comment_run_for_tag_scanning() {
        let src = "\"\"\"\nSatisfies: REQ-001\n\"\"\"\n";
        let runs = extract_comment_runs(src, &lang("Python"));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].text.contains("Satisfies: REQ-001"));
    }

    #[test]
    fn unterminated_block_comment_runs_to_eof() {
        let src = "/*\nSatisfies: REQ-001\n";
        let runs = extract_comment_runs(src, &lang("Rust"));
        assert_eq!(runs.len(), 1);
        // Unterminated run still yields a comment text covering
        // the rest of the file.
        assert!(runs[0].text.contains("Satisfies: REQ-001"));
    }

    #[test]
    fn code_before_line_comment_is_recognised_but_flagged_mixed() {
        let src = "let x = 1; // Satisfies: REQ-001\n";
        let runs = extract_comment_runs(src, &lang("Rust"));
        assert_eq!(runs.len(), 1);
        assert!(!runs[0].comment_only_lines);
    }

    #[test]
    fn shell_and_dockerfile_recognise_hash_comments() {
        let src = "#!/usr/bin/env bash\n# Verifies: TEST-001\necho hi\n";
        let runs = extract_comment_runs(src, &lang("POSIX shell"));
        let verbs: Vec<&str> = runs.iter().map(|r| r.text).collect();
        assert!(verbs.iter().any(|t| t.contains("Verifies: TEST-001")));

        let df_runs =
            extract_comment_runs("# Satisfies: REQ-001\nFROM scratch\n", &lang("Dockerfile"));
        assert_eq!(df_runs.len(), 1);
        assert!(df_runs[0].text.contains("Satisfies: REQ-001"));
    }

    #[test]
    fn js_block_and_line_comments_both_recognised() {
        let src = "/**\n * Satisfies: REQ-001\n */\nconsole.log('x'); // Verifies: TEST-001\n";
        let runs = extract_comment_runs(src, &lang("JavaScript"));
        // Expect one block-comment run and one line-comment
        // run.
        assert!(runs.len() >= 2);
        let joined: String = runs.iter().map(|r| r.text).collect();
        assert!(joined.contains("Satisfies: REQ-001"));
        assert!(joined.contains("Verifies: TEST-001"));
    }
}
