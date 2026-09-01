//! Shape-aware diff (Phase 5d).
//!
//! Implements: ART007
//!
//! Given two commit OIDs (or "current" for the working-tree view),
//! produce a structured DTO per artifact shape:
//!
//! - `content`: line diff of the Markdown body using
//!   [`similar::TextDiff`] with inline emphasis.
//! - `blob`: before/after byte size, content hash, media type —
//!   the UI renders a side-by-side preview (per the Phase 5 locked
//!   decision on image diff).
//! - `url`: before/after URL string plus a note reminding the
//!   reader that the external content may have shifted between
//!   checks.
//!
//! The actual extraction of "artifact at commit" lives in
//! [`crate::git_history`]; this module focuses on framing the
//! differences once both sides are in hand.

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

use crate::schema::ArtifactShape;

/// Top-level diff DTO tagged on `shape` so the frontend can
/// discriminate without a runtime schema guess.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "shape")]
pub enum ShapeDiff {
    #[serde(rename = "content")]
    Content(ContentDiff),
    #[serde(rename = "blob")]
    Blob(BlobDiff),
    #[serde(rename = "url")]
    Url(UrlDiff),
}

impl ShapeDiff {
    pub fn shape(&self) -> ArtifactShape {
        match self {
            Self::Content(_) => ArtifactShape::Content,
            Self::Blob(_) => ArtifactShape::Blob,
            Self::Url(_) => ArtifactShape::Url,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDiff {
    /// Sequence of `same` / `added` / `removed` line entries in
    /// order — suitable for direct rendering as a unified diff.
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    Same,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobDiff {
    /// `None` on sides that didn't have the blob at that commit
    /// (added / removed). All three facts may independently shift
    /// — a file renamed from .docx to .pdf bumps `mediaType`
    /// without changing `contentHash`.
    pub before: Option<BlobSide>,
    pub after: Option<BlobSide>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobSide {
    pub byte_size: u64,
    pub content_hash: String,
    pub media_type: String,
    /// Stable URL the UI can load for the side-by-side preview.
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlDiff {
    pub before: Option<String>,
    pub after: Option<String>,
    /// Matches `UX-diffView`'s external-content disclaimer —
    /// keeps the wording authoritative on the backend so every
    /// client renders the same caveat.
    pub note: &'static str,
}

impl UrlDiff {
    pub const EXTERNAL_CONTENT_NOTE: &'static str = "The URL string is what's stored locally. The external content at \
         either URL may have changed since each check — this diff does not \
         fetch or compare the remote content.";
}

/// Compute a line diff between two bodies. `None` on either side
/// means the artifact didn't exist at that commit; we fall back
/// to treating missing sides as empty strings so the UI still
/// renders an "all-added" or "all-removed" block.
pub fn diff_content(before: Option<&str>, after: Option<&str>) -> ContentDiff {
    let before = before.unwrap_or("");
    let after = after.unwrap_or("");
    let diff = TextDiff::from_lines(before, after);
    let mut lines = Vec::new();
    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            ChangeTag::Equal => DiffLineKind::Same,
            ChangeTag::Insert => DiffLineKind::Added,
            ChangeTag::Delete => DiffLineKind::Removed,
        };
        // TextDiff keeps the trailing newline on each line; strip
        // so the frontend doesn't render spurious blank rows.
        let text = change
            .value()
            .strip_suffix('\n')
            .unwrap_or(change.value())
            .to_owned();
        lines.push(DiffLine { kind, text });
    }
    ContentDiff { lines }
}

/// Compose a blob diff from two optional sides. The caller
/// resolves each side's `BlobSide` (or `None`) from the git
/// history + cached blob facts.
pub fn diff_blob(before: Option<BlobSide>, after: Option<BlobSide>) -> BlobDiff {
    BlobDiff { before, after }
}

/// Compose a URL diff from two optional URL strings.
pub fn diff_url(before: Option<String>, after: Option<String>) -> UrlDiff {
    UrlDiff {
        before,
        after,
        note: UrlDiff::EXTERNAL_CONTENT_NOTE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_content_marks_added_removed_and_same_lines() {
        let diff = diff_content(Some("a\nb\nc\n"), Some("a\nB\nc\n"));
        let kinds: Vec<DiffLineKind> = diff.lines.iter().map(|l| l.kind).collect();
        // `a`, remove `b`, add `B`, `c` — similar's line semantics
        // may interleave the add/remove; count the categories.
        let added = kinds
            .iter()
            .filter(|k| matches!(k, DiffLineKind::Added))
            .count();
        let removed = kinds
            .iter()
            .filter(|k| matches!(k, DiffLineKind::Removed))
            .count();
        let same = kinds
            .iter()
            .filter(|k| matches!(k, DiffLineKind::Same))
            .count();
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
        assert_eq!(same, 2);
    }

    #[test]
    fn diff_content_handles_one_sided_missing_input() {
        let diff = diff_content(None, Some("first\nsecond\n"));
        assert!(
            diff.lines
                .iter()
                .all(|l| matches!(l.kind, DiffLineKind::Added)),
            "all lines should be additions when before is missing",
        );
    }

    #[test]
    fn diff_content_strips_trailing_newlines_so_ui_does_not_render_blank_rows() {
        let diff = diff_content(Some("one\ntwo\n"), Some("one\ntwo\n"));
        for line in &diff.lines {
            assert!(!line.text.ends_with('\n'));
        }
    }

    #[test]
    fn url_diff_carries_the_stable_external_content_disclaimer() {
        let diff = diff_url(
            Some("https://old.example.com".into()),
            Some("https://new.example.com".into()),
        );
        assert_eq!(diff.note, UrlDiff::EXTERNAL_CONTENT_NOTE);
    }

    #[test]
    fn blob_diff_mirrors_both_sides_regardless_of_symmetry() {
        let side = BlobSide {
            byte_size: 1234,
            content_hash: "a".repeat(64),
            media_type: "application/pdf".to_owned(),
            download_url: "/api/artifacts/u/blob".to_owned(),
        };
        let d = diff_blob(None, Some(side.clone()));
        assert!(d.before.is_none());
        assert!(d.after.is_some());
        let d2 = diff_blob(Some(side), None);
        assert!(d2.before.is_some());
        assert!(d2.after.is_none());
    }

    #[test]
    fn shape_diff_is_tagged_so_frontend_can_discriminate() {
        let diff = ShapeDiff::Url(diff_url(None, Some("https://x".into())));
        let v = serde_json::to_value(&diff).unwrap();
        assert_eq!(v["shape"], "url");
        assert_eq!(v["after"], "https://x");
    }
}
