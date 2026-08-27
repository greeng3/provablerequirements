//! Download-filename builder for the Phase 6b export endpoints.
//!
//! Output shape, per the locked decision:
//! `reqforge-<kind>-<scope-slug>-<YYYYMMDDTHHMMSSZ>.<ext>`
//!
//! Example:
//! `reqforge-coverage-matrix-collection-sample-REQ-20260422T030000Z.csv`

use chrono::{DateTime, Utc};

use crate::reports::{ReportKind, ScopeDto};

use super::ExportFormat;

/// Turn a `ScopeDto` into the path-safe slug that lands inside
/// the download filename. `system`, `project-<slug>`, or
/// `collection-<slug>-<prefix>` — all pieces are lower-cased and
/// unsafe characters are replaced with dashes so the filename
/// stays `[a-z0-9.-]+`.
pub fn scope_slug(scope: &ScopeDto) -> String {
    match scope {
        ScopeDto::System => "system".to_owned(),
        ScopeDto::Project { slug } => format!("project-{}", sanitize(slug)),
        ScopeDto::Collection { slug, prefix } => {
            format!("collection-{}-{}", sanitize(slug), sanitize(prefix))
        }
    }
}

fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for ch in s.chars() {
        let normal = if ch.is_ascii_alphanumeric() || ch == '.' {
            Some(ch.to_ascii_lowercase())
        } else if last_dash {
            None
        } else {
            Some('-')
        };
        if let Some(c) = normal {
            last_dash = c == '-';
            out.push(c);
        }
    }
    let trimmed = out.trim_matches('-');
    trimmed.to_owned()
}

/// Format a timestamp as `YYYYMMDDTHHMMSSZ` — compact enough for
/// a filename while still being unambiguous.
pub fn utc_stamp(at: DateTime<Utc>) -> String {
    at.format("%Y%m%dT%H%M%SZ").to_string()
}

pub fn build(
    kind: ReportKind,
    scope: &ScopeDto,
    format: ExportFormat,
    at: DateTime<Utc>,
) -> String {
    format!(
        "reqforge-{}-{}-{}.{}",
        kind.as_kebab(),
        scope_slug(scope),
        utc_stamp(at),
        format.ext(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn scope_slug_handles_the_three_variants() {
        assert_eq!(scope_slug(&ScopeDto::System), "system");
        assert_eq!(
            scope_slug(&ScopeDto::Project {
                slug: "Sample-Proj".into()
            }),
            "project-sample-proj"
        );
        assert_eq!(
            scope_slug(&ScopeDto::Collection {
                slug: "Sample".into(),
                prefix: "REQ".into()
            }),
            "collection-sample-req"
        );
    }

    #[test]
    fn scope_slug_collapses_punctuation_and_whitespace() {
        let slug = scope_slug(&ScopeDto::Project {
            slug: "my project / with stuff".into(),
        });
        assert_eq!(slug, "project-my-project-with-stuff");
    }

    #[test]
    fn filename_matches_locked_shape() {
        let at = Utc.with_ymd_and_hms(2026, 4, 22, 3, 0, 0).unwrap();
        let name = build(
            ReportKind::CoverageMatrix,
            &ScopeDto::Collection {
                slug: "sample".into(),
                prefix: "REQ".into(),
            },
            ExportFormat::Csv,
            at,
        );
        assert_eq!(
            name,
            "reqforge-coverage-matrix-collection-sample-req-20260422T030000Z.csv"
        );
    }

    #[test]
    fn filename_applies_to_every_format() {
        let at = Utc.with_ymd_and_hms(2026, 4, 22, 3, 0, 0).unwrap();
        let scope = ScopeDto::System;
        for (fmt, ext) in [
            (ExportFormat::Json, "json"),
            (ExportFormat::Csv, "csv"),
            (ExportFormat::Html, "html"),
        ] {
            let name = build(ReportKind::LinkOrphans, &scope, fmt, at);
            assert!(
                name.ends_with(&format!(".{ext}")),
                "expected .{ext} suffix, got {name}"
            );
        }
    }
}
