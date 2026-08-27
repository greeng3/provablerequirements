//! Report exports (Phase 6b).
//!
//! Each report kind gets three download representations served
//! off `GET /api/reports/{kind}/export.{ext}`:
//!
//! - `json` — the existing `ReportResponse` wire shape, served
//!   with `Content-Disposition: attachment` so browsers download
//!   instead of rendering.
//! - `csv` — per-kind tabular encoding via the `csv` crate. Seven
//!   of the eight kinds support CSV; cycles declines with a
//!   structured 406 (the nested-loop shape has no flat encoding
//!   that isn't actively misleading — see the `REPORT-
//!   baselineExports` "may decline" clause).
//! - `html` — standalone single-page HTML with inline CSS and
//!   hyperlinked artifact breadcrumbs (absolute URLs via
//!   `REQFORGE_EXTERNAL_URL`, or same-origin relative when that
//!   env var is empty).
//!
//! Deferred per their own specs: PDF (`REPORT-pdfExport`), CLI
//! headless (`REPORT-cliExport`), publishable HTML site
//! (`REPORT-publishSite`), regulatory-formatted outputs
//! (`REPORT-regulatoryOutputs`).

pub mod csv;
pub mod doorstop;
pub mod filename;
pub mod html;

use crate::reports::ReportResponse;

/// The three supported export extensions. The handler's URL
/// dispatch validates against this set; unknown extensions → 404.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Csv,
    Html,
}

impl ExportFormat {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(Self::Json),
            "csv" => Some(Self::Csv),
            "html" => Some(Self::Html),
            _ => None,
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Html => "text/html; charset=utf-8",
        }
    }

    pub fn ext(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Html => "html",
        }
    }
}

/// Result of dispatching a CSV export — either the rendered bytes
/// or a structured 406 payload so the handler can surface the
/// locked "cycles declines" behaviour consistently.
#[derive(Debug)]
pub enum CsvOutcome {
    Bytes(Vec<u8>),
    NotAcceptable {
        reason: String,
        alternatives: &'static [&'static str],
    },
}

/// Dispatch a CSV export against the matching `ReportResponse`
/// variant. Cycles declines per the locked decision; everything
/// else delegates to a per-kind renderer in `exports::csv`.
pub fn render_csv(response: &ReportResponse) -> CsvOutcome {
    match response {
        ReportResponse::UnresolvedLinks(r) => CsvOutcome::Bytes(csv::unresolved_links(r)),
        ReportResponse::LinkOrphans(r) => CsvOutcome::Bytes(csv::link_orphans(r)),
        ReportResponse::Conflicts(r) => CsvOutcome::Bytes(csv::conflicts(r)),
        ReportResponse::CoverageMatrix(r) => CsvOutcome::Bytes(csv::coverage_matrix(r)),
        ReportResponse::ImpactAnalysis(r) => CsvOutcome::Bytes(csv::impact_analysis(r)),
        ReportResponse::ReviewStatus(r) => CsvOutcome::Bytes(csv::review_status(r)),
        ReportResponse::FilesystemOrphans(r) => CsvOutcome::Bytes(csv::filesystem_orphans(r)),
        ReportResponse::CodeTraceability(r) => CsvOutcome::Bytes(csv::code_traceability(r)),
        ReportResponse::Cycles(_) => CsvOutcome::NotAcceptable {
            reason: "The cycles report has no natural flat CSV encoding — \
                     nested loops don't fit a single row-per-entry schema. \
                     Use the JSON or HTML export instead."
                .to_owned(),
            alternatives: &["json", "html"],
        },
    }
}

/// Dispatch an HTML export. Unlike CSV this never declines — every
/// report kind has a sensible HTML rendering, even if it's just a
/// table with the same columns as the CSV + hyperlinked breadcrumb
/// links.
pub fn render_html(response: &ReportResponse, external_url: Option<&str>) -> Vec<u8> {
    html::render(response, external_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_round_trips() {
        for (s, expected) in [
            ("json", Some(ExportFormat::Json)),
            ("csv", Some(ExportFormat::Csv)),
            ("html", Some(ExportFormat::Html)),
            ("xml", None),
            ("", None),
        ] {
            assert_eq!(ExportFormat::from_ext(s), expected);
        }
    }

    #[test]
    fn mime_types_are_correct_per_format() {
        assert_eq!(ExportFormat::Json.mime(), "application/json");
        assert_eq!(ExportFormat::Csv.mime(), "text/csv; charset=utf-8");
        assert_eq!(ExportFormat::Html.mime(), "text/html; charset=utf-8");
    }
}
