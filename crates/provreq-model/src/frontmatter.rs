//! JSON-in-YAML frontmatter parser for content-hosted artifact
//! Markdown files, per `FORMAT-frontmatterDelimiters`.
//!
//! The on-disk convention is a line containing exactly `---`, the JSON
//! object spanning the next N lines, a closing `---` line, and the
//! Markdown body. Because any valid JSON is also valid YAML flow-style,
//! GitHub / GitLab / Pandoc / Jekyll / Hugo render the block
//! identically to their usual YAML frontmatter.
//!
//! This parser is intentionally strict: the first line of the input
//! must be `---`, and the JSON block must be closed by a matching
//! `---` line. Anything else is a diagnostic, reported per-artifact
//! by the loader so other artifacts in the same Project still load.

use thiserror::Error;

const DELIMITER: &str = "---";

/// Splits an artifact file into its JSON-frontmatter text and
/// Markdown-body text. Delimiter lines themselves are stripped from
/// both halves.
///
/// The returned slices borrow from `input`. The JSON half has not
/// been parsed; the loader runs `serde_json::from_str` on it.
pub fn split_frontmatter(input: &str) -> Result<(&str, &str), FrontmatterError> {
    let rest = input
        .strip_prefix(DELIMITER)
        .ok_or(FrontmatterError::MissingOpeningDelimiter)?;

    // The character immediately after the opening `---` must be a
    // newline — i.e. the delimiter occupied its own line.
    let after_opening = match rest.as_bytes().first() {
        Some(b'\n') => &rest[1..],
        Some(b'\r') if rest.as_bytes().get(1) == Some(&b'\n') => &rest[2..],
        _ => return Err(FrontmatterError::MissingOpeningDelimiter),
    };

    // Scan line-by-line for a closing delimiter line.
    let mut cursor = 0usize;
    let bytes = after_opening.as_bytes();
    while cursor < bytes.len() {
        let line_end = memchr_lf(bytes, cursor);
        let line = &after_opening[cursor..line_end];
        // A delimiter line is exactly `---`, tolerating a trailing `\r`
        // for CRLF files read verbatim.
        let trimmed = line.strip_suffix('\r').unwrap_or(line);
        if trimmed == DELIMITER {
            let json = &after_opening[..cursor];
            // Strip the trailing newline that separates the JSON body
            // from the closing delimiter, so the caller sees clean
            // JSON text.
            let json = json.strip_suffix('\n').unwrap_or(json);
            let json = json.strip_suffix('\r').unwrap_or(json);

            // Advance past the closing delimiter line and its newline.
            let after_close = if line_end < bytes.len() {
                line_end + 1 // past the `\n`
            } else {
                line_end
            };
            let body = &after_opening[after_close..];
            return Ok((json, body));
        }
        cursor = if line_end < bytes.len() {
            line_end + 1
        } else {
            line_end
        };
    }

    Err(FrontmatterError::UnterminatedFrontmatter)
}

/// Finds the next `\n` at-or-after `start`, returning its byte index
/// or `bytes.len()` if none is found.
fn memchr_lf(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|offset| start + offset)
        .unwrap_or(bytes.len())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrontmatterError {
    #[error("artifact does not start with a `---` frontmatter delimiter line")]
    MissingOpeningDelimiter,

    #[error("frontmatter is not terminated by a closing `---` line")]
    UnterminatedFrontmatter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_minimal_frontmatter_and_body() {
        let input = "---\n{\"schemaVersion\":1}\n---\n# Body\n";
        let (json, body) = split_frontmatter(input).unwrap();
        assert_eq!(json, "{\"schemaVersion\":1}");
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn splits_multiline_json_object() {
        let input = "\
---
{
  \"schemaVersion\": 1,
  \"title\": \"x\"
}
---
# Heading

Body paragraph.
";
        let (json, body) = split_frontmatter(input).unwrap();
        assert!(json.contains("\"schemaVersion\": 1"));
        assert!(json.contains("\"title\": \"x\""));
        assert!(body.starts_with("# Heading"));
        assert!(body.ends_with("Body paragraph.\n"));
    }

    #[test]
    fn allows_empty_body() {
        let input = "---\n{}\n---\n";
        let (json, body) = split_frontmatter(input).unwrap();
        assert_eq!(json, "{}");
        assert_eq!(body, "");
    }

    #[test]
    fn missing_opening_delimiter_errors() {
        let input = "{\"schemaVersion\":1}\n---\n# Body\n";
        let err = split_frontmatter(input).unwrap_err();
        assert_eq!(err, FrontmatterError::MissingOpeningDelimiter);
    }

    #[test]
    fn blank_line_before_opening_delimiter_errors() {
        let input = "\n---\n{}\n---\n";
        let err = split_frontmatter(input).unwrap_err();
        assert_eq!(err, FrontmatterError::MissingOpeningDelimiter);
    }

    #[test]
    fn dashes_not_on_own_line_errors() {
        // `---` followed by text on the same line is not a valid
        // delimiter.
        let input = "--- not-a-delimiter\n{}\n---\n";
        let err = split_frontmatter(input).unwrap_err();
        assert_eq!(err, FrontmatterError::MissingOpeningDelimiter);
    }

    #[test]
    fn unterminated_frontmatter_errors() {
        let input = "---\n{\"schemaVersion\":1}\n# Body without closing delimiter\n";
        let err = split_frontmatter(input).unwrap_err();
        assert_eq!(err, FrontmatterError::UnterminatedFrontmatter);
    }

    #[test]
    fn tolerates_crlf_line_endings() {
        let input = "---\r\n{\"schemaVersion\":1}\r\n---\r\n# Body\r\n";
        let (json, body) = split_frontmatter(input).unwrap();
        assert_eq!(json, "{\"schemaVersion\":1}");
        assert_eq!(body, "# Body\r\n");
    }

    #[test]
    fn resulting_json_is_valid_serde_json() {
        let input = "---\n{\"schemaVersion\":1,\"n\":\"x\"}\n---\n";
        let (json, _) = split_frontmatter(input).unwrap();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["n"], "x");
    }

    #[test]
    fn rejects_trailing_whitespace_on_delimiter() {
        // Spec says "exactly ---"; a trailing space means it's not a
        // delimiter, so we treat the frontmatter as unterminated.
        let input = "---\n{}\n--- \n";
        let err = split_frontmatter(input).unwrap_err();
        assert_eq!(err, FrontmatterError::UnterminatedFrontmatter);
    }
}
