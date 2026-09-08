//! Doorstop import report renderers (Phase 8.2).
//!
//! Piggy-backs on the Phase 6b export scaffolding: the
//! doorstop import report renders to JSON (by default — just
//! the `ImportReport` DTO serialised), CSV (a multi-section
//! tabular layout matching the Phase 6b filesystem-orphans
//! pattern), and HTML (single-page with inline CSS).
//!
//! The JSON surface is already `Serialize`-able via the DTO,
//! so this module only needs to supply CSV + HTML renderers.

use crate::doorstop::report::{ImportReport, ReportRefDisposition};

/// CSV encoding of an import report. Emits four labelled
/// sections separated by blank rows so the output remains
/// viewable in a spreadsheet while encoding enough detail for
/// audit review.
pub fn render_csv(report: &ImportReport) -> Vec<u8> {
    let mut wtr = csv::WriterBuilder::new().flexible(true).from_writer(vec![]);

    // Section 1 — run metadata.
    let _ = wtr.write_record(["# doorstop import run"]);
    let _ = wtr.write_record(["project", report.project_slug.as_str()]);
    let _ = wtr.write_record(["source", report.source.as_str()]);
    let _ = wtr.write_record(["importRunAt", report.import_run_at.to_rfc3339().as_str()]);
    let _ = wtr.write_record(Vec::<&str>::new());

    // Section 2 — per-collection summary.
    let _ = wtr.write_record(["# collections"]);
    let _ = wtr.write_record([
        "prefix",
        "name",
        "directoryName",
        "artifactCount",
        "syntheticReviewCount",
        "legacyPreservedCount",
        "derivesFromLinkCount",
        "urlArtifactCount",
    ]);
    for c in &report.collections {
        let _ = wtr.write_record([
            c.prefix.as_str(),
            c.name.as_str(),
            c.directory_name.as_str(),
            &c.artifact_count.to_string(),
            &c.synthetic_review_count.to_string(),
            &c.legacy_preserved_count.to_string(),
            &c.derives_from_link_count.to_string(),
            &c.url_artifact_count.to_string(),
        ]);
    }
    let _ = wtr.write_record(Vec::<&str>::new());

    // Section 3 — ref dispositions.
    let _ = wtr.write_record(["# ref dispositions"]);
    let _ = wtr.write_record(["sourceUid", "kind", "value"]);
    for disp in &report.ref_dispositions {
        match disp {
            ReportRefDisposition::UrlArtifact {
                source_uid,
                url,
                url_artifact_name,
            } => {
                let _ = wtr.write_record([
                    source_uid.as_str(),
                    "url-artifact",
                    &format!("{url} (as {url_artifact_name})"),
                ]);
            }
            ReportRefDisposition::Legacy { source_uid, value } => {
                let _ = wtr.write_record([source_uid.as_str(), "legacy", value.as_str()]);
            }
        }
    }
    let _ = wtr.write_record(Vec::<&str>::new());

    // Section 4 — unresolved links.
    let _ = wtr.write_record(["# unresolved links"]);
    let _ = wtr.write_record(["sourceUid", "targetUid", "sourceMarkerPath"]);
    for u in &report.unresolved_links {
        let _ = wtr.write_record([
            u.source_uid.as_str(),
            u.target_uid.as_str(),
            &u.source_marker_path.display().to_string(),
        ]);
    }
    let _ = wtr.write_record(Vec::<&str>::new());

    // Section 5 — warnings.
    let _ = wtr.write_record(["# warnings"]);
    for w in &report.warnings {
        let _ = wtr.write_record([w.as_str()]);
    }

    wtr.flush().ok();
    wtr.into_inner().unwrap_or_default()
}

/// Standalone HTML rendering. No external CSS — keeps the
/// download self-contained for offline review.
pub fn render_html(report: &ImportReport) -> Vec<u8> {
    let mut out = String::with_capacity(4096);
    out.push_str(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Doorstop import report</title>
<style>
body { font-family: system-ui, sans-serif; max-width: 960px; margin: 2rem auto; padding: 0 1rem; color: #0f172a; }
h1 { font-size: 1.4rem; margin-bottom: 0.25rem; }
h2 { font-size: 1.1rem; border-bottom: 1px solid #cbd5e1; padding-bottom: 0.25rem; margin-top: 2rem; }
table { border-collapse: collapse; width: 100%; font-size: 0.9rem; }
th, td { border: 1px solid #e2e8f0; padding: 0.25rem 0.5rem; text-align: left; vertical-align: top; }
th { background: #f1f5f9; }
code { font-family: ui-monospace, monospace; font-size: 0.85rem; background: #f1f5f9; padding: 0 0.2rem; border-radius: 3px; }
.muted { color: #475569; }
.warn { background: #fef3c7; }
</style>
</head>
<body>
"#,
    );

    out.push_str(&format!(
        "<h1>Doorstop import report</h1><p class=\"muted\">Project <code>{}</code> · source <code>{}</code> · {}</p>",
        html_escape(&report.project_slug),
        html_escape(&report.source),
        html_escape(&report.import_run_at.to_rfc3339()),
    ));

    // Totals.
    out.push_str("<h2>Totals</h2><table>");
    let t = &report.totals;
    for (label, value) in [
        ("Collections created", t.collections_created),
        ("Artifacts imported", t.artifacts_imported),
        ("derives-from links", t.derives_from_links),
        ("URL artifacts", t.url_artifacts),
        ("cites links", t.cites_links),
        ("Legacy refs", t.legacy_refs),
        ("Synthetic review entries", t.synthetic_review_entries),
        ("Legacy-preserved fields", t.legacy_preserved_fields),
        ("Unresolved links", t.unresolved_link_count),
    ] {
        out.push_str(&format!(
            "<tr><th>{}</th><td>{}</td></tr>",
            html_escape(label),
            value
        ));
    }
    out.push_str("</table>");

    // Collections.
    out.push_str("<h2>Collections</h2>");
    if report.collections.is_empty() {
        out.push_str("<p class=\"muted\">None.</p>");
    } else {
        out.push_str(
            "<table><thead><tr><th>Prefix</th><th>Name</th><th>Dir</th><th>Artifacts</th><th>Synthetic reviews</th><th>Legacy preserved</th><th>derives-from</th><th>URL artifacts</th></tr></thead><tbody>",
        );
        for c in &report.collections {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                html_escape(&c.prefix),
                html_escape(&c.name),
                html_escape(&c.directory_name),
                c.artifact_count,
                c.synthetic_review_count,
                c.legacy_preserved_count,
                c.derives_from_link_count,
                c.url_artifact_count,
            ));
        }
        out.push_str("</tbody></table>");
    }

    // Ref dispositions.
    out.push_str("<h2>Ref dispositions</h2>");
    if report.ref_dispositions.is_empty() {
        out.push_str("<p class=\"muted\">No refs processed.</p>");
    } else {
        out.push_str(
            "<table><thead><tr><th>Source UID</th><th>Kind</th><th>Value</th></tr></thead><tbody>",
        );
        for disp in &report.ref_dispositions {
            match disp {
                ReportRefDisposition::UrlArtifact {
                    source_uid,
                    url,
                    url_artifact_name,
                } => {
                    out.push_str(&format!(
                        "<tr><td><code>{}</code></td><td>url-artifact</td><td>{} <span class=\"muted\">(as <code>{}</code>)</span></td></tr>",
                        html_escape(source_uid),
                        html_escape(url),
                        html_escape(url_artifact_name),
                    ));
                }
                ReportRefDisposition::Legacy { source_uid, value } => {
                    out.push_str(&format!(
                        "<tr><td><code>{}</code></td><td>legacy</td><td>{}</td></tr>",
                        html_escape(source_uid),
                        html_escape(value),
                    ));
                }
            }
        }
        out.push_str("</tbody></table>");
    }

    // Unresolved links.
    out.push_str("<h2>Unresolved links</h2>");
    if report.unresolved_links.is_empty() {
        out.push_str("<p class=\"muted\">None — all doorstop links resolved.</p>");
    } else {
        out.push_str(
            "<table><thead><tr><th>Source UID</th><th>Target UID</th><th>Source marker</th></tr></thead><tbody>",
        );
        for u in &report.unresolved_links {
            out.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td></tr>",
                html_escape(&u.source_uid),
                html_escape(&u.target_uid),
                html_escape(&u.source_marker_path.display().to_string()),
            ));
        }
        out.push_str("</tbody></table>");
    }

    // Warnings.
    out.push_str("<h2>Warnings</h2>");
    if report.warnings.is_empty() {
        out.push_str("<p class=\"muted\">None.</p>");
    } else {
        out.push_str("<ul>");
        for w in &report.warnings {
            out.push_str(&format!("<li class=\"warn\">{}</li>", html_escape(w)));
        }
        out.push_str("</ul>");
    }

    out.push_str("</body></html>\n");
    out.into_bytes()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
