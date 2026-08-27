//! HTML renderers for the Phase 6b report exports.
//!
//! One standalone `<!DOCTYPE html>` page per export: inline CSS,
//! title + scope block, then one body block per report kind.
//! Every artifact name is an `<a href>` built through
//! [`artifact_href`] so links work either relative (same-origin
//! re-serve) or absolute (via `REQFORGE_EXTERNAL_URL`).

use chrono::Utc;

use crate::reports::{
    ConflictsReport, CoverageMatrixReport, CycleNode, CyclesReport, FilesystemOrphansReport,
    ImpactAnalysisReport, LinkOrphansReport, ReportResponse, ReviewStatusReport, ScopeDto,
    UnresolvedLinksReport,
};

/// Embedded stylesheet kept tight — no external deps, so the
/// HTML renders the same offline as it does when served back
/// through ReqForge.
const INLINE_CSS: &str = "
  :root { color-scheme: light dark; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    max-width: 980px;
    margin: 2rem auto;
    padding: 0 1rem;
    color: #0f172a;
    background: #ffffff;
    line-height: 1.4;
  }
  @media (prefers-color-scheme: dark) {
    body { color: #e2e8f0; background: #0f172a; }
    table { border-color: #334155; }
    th { background: #1e293b; }
  }
  h1 { margin-bottom: 0.25rem; }
  .meta { color: #64748b; font-size: 0.85rem; margin-bottom: 1.5rem; }
  table { border-collapse: collapse; width: 100%; margin: 1rem 0; border: 1px solid #cbd5e1; }
  th, td { text-align: left; padding: 0.35rem 0.6rem; border-bottom: 1px solid #e2e8f0; font-size: 0.9rem; }
  th { background: #f1f5f9; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; }
  code, .mono { font-family: ui-monospace, 'Cascadia Code', 'Fira Code', Menlo, monospace; font-size: 0.85rem; }
  a { color: #0369a1; }
  .gap { color: #b91c1c; font-weight: 600; }
  .covered { color: #15803d; }
  .inactive { color: #94a3b8; font-style: italic; }
  .summary { color: #64748b; font-size: 0.9rem; }
  .section-h2 { margin-top: 2rem; font-size: 1.1rem; }
  .pill { display: inline-block; padding: 0.1rem 0.4rem; border-radius: 0.25rem; font-size: 0.7rem; background: #e2e8f0; }
  .pill.warn { background: #fef3c7; color: #92400e; }
  .pill.bad { background: #fee2e2; color: #991b1b; }
  .pill.good { background: #dcfce7; color: #166534; }
";

/// Top-level entry point. Dispatches on the `ReportResponse`
/// variant; every kind has a rendering even if it's just a
/// narrow table.
pub fn render(response: &ReportResponse, external_url: Option<&str>) -> Vec<u8> {
    let base = external_url.unwrap_or("");
    let (title, body) = match response {
        ReportResponse::UnresolvedLinks(r) => (
            "Unresolved links".to_owned(),
            render_unresolved_links(r, base),
        ),
        ReportResponse::LinkOrphans(r) => (
            "Link-graph orphans".to_owned(),
            render_link_orphans(r, base),
        ),
        ReportResponse::Cycles(r) => ("Cycles".to_owned(), render_cycles(r, base)),
        ReportResponse::Conflicts(r) => ("Conflicts".to_owned(), render_conflicts(r, base)),
        ReportResponse::CoverageMatrix(r) => (
            "Coverage matrix".to_owned(),
            render_coverage_matrix(r, base),
        ),
        ReportResponse::ImpactAnalysis(r) => (
            "Impact analysis".to_owned(),
            render_impact_analysis(r, base),
        ),
        ReportResponse::ReviewStatus(r) => ("Review status".to_owned(), render_review_status(r)),
        ReportResponse::FilesystemOrphans(r) => (
            "Filesystem orphans".to_owned(),
            render_filesystem_orphans(r),
        ),
        ReportResponse::CodeTraceability(r) => {
            ("Code traceability".to_owned(), render_code_traceability(r))
        }
    };
    let scope = scope_description(response.scope());
    let stamp = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<title>ReqForge · {title} · {scope}</title>\n\
<style>{css}</style>\n\
</head>\n\
<body>\n\
<h1>{title}</h1>\n\
<p class=\"meta\">Scope: <code>{scope}</code> · Generated: <code>{stamp}</code></p>\n\
{body}\n\
</body>\n\
</html>\n",
        title = escape(&title),
        scope = escape(&scope),
        css = INLINE_CSS,
        stamp = escape(&stamp),
        body = body,
    )
    .into_bytes()
}

/// Build a URL for an artifact detail page, honouring the
/// `external_url` base. Empty base produces a same-origin
/// relative path (works when the HTML is re-served through
/// ReqForge; breaks gracefully offline).
fn artifact_href(base: &str, project: &str, collection: &str, artifact: &str) -> String {
    format!(
        "{}/projects/{}/collections/{}/artifacts/{}",
        base.trim_end_matches('/'),
        project,
        collection,
        artifact,
    )
}

fn scope_description(scope: &ScopeDto) -> String {
    match scope {
        ScopeDto::System => "system".to_owned(),
        ScopeDto::Project { slug } => format!("project:{slug}"),
        ScopeDto::Collection { slug, prefix } => format!("collection:{slug}/{prefix}"),
    }
}

/// Minimal HTML escape for the five reserved characters.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

fn artifact_link_cell(base: &str, node: &CycleNode) -> String {
    let href = artifact_href(
        base,
        &node.project_slug,
        &node.collection_prefix,
        &node.artifact_name,
    );
    let title = escape(&node.title);
    let slug = escape(&node.project_slug);
    let coll = escape(&node.collection_prefix);
    let name = escape(&node.artifact_name);
    let inactive = if node.active {
        String::new()
    } else {
        " <span class=\"pill\">inactive</span>".to_owned()
    };
    format!(
        "<a href=\"{href}\"><code>{slug}/{coll}/{name}</code></a> <span class=\"summary\">{title}</span>{inactive}",
        href = escape(&href),
    )
}

// ---- per-kind body renderers ----

fn render_unresolved_links(report: &UnresolvedLinksReport, base: &str) -> String {
    if report.total_unresolved == 0 {
        return "<p>No unresolved links in scope.</p>".to_owned();
    }
    let mut body = format!(
        "<p class=\"summary\">{} unresolved link{}.</p>\n<table>\n<thead><tr><th>Source</th><th>Link type</th><th>Target hint</th><th>Reason</th></tr></thead>\n<tbody>\n",
        report.total_unresolved,
        if report.total_unresolved == 1 {
            ""
        } else {
            "s"
        },
    );
    for e in &report.entries {
        let reason_class = match e.reason {
            "target-missing" => "bad",
            "mount-missing" => "warn",
            _ => "",
        };
        let src_href = artifact_href(
            base,
            &e.source_project_slug,
            &e.source_collection_prefix,
            &e.source_artifact_name,
        );
        body.push_str(&format!(
            "<tr><td><a href=\"{src_href}\"><code>{sp}/{sc}/{sa}</code></a> <span class=\"summary\">{stitle}</span></td><td><code>{lt}</code></td><td><code>{hp}/{hc}/{ha}</code></td><td><span class=\"pill {cls}\">{reason}</span></td></tr>\n",
            src_href = escape(&src_href),
            sp = escape(&e.source_project_slug),
            sc = escape(&e.source_collection_prefix),
            sa = escape(&e.source_artifact_name),
            stitle = escape(&e.source_title),
            lt = escape(&e.link_type),
            hp = escape(&e.target_hint_project_slug),
            hc = escape(&e.target_hint_collection_prefix),
            ha = escape(&e.target_hint_artifact_name),
            cls = reason_class,
            reason = escape(e.reason),
        ));
    }
    body.push_str("</tbody>\n</table>\n");
    body
}

fn render_link_orphans(report: &LinkOrphansReport, base: &str) -> String {
    if report.total_orphans == 0 {
        return "<p>No link-graph orphans in scope.</p>".to_owned();
    }
    let mut body = format!(
        "<p class=\"summary\">{} orphan artifact{}.</p>\n<table>\n<thead><tr><th>Artifact</th><th>Shape</th><th>Status</th></tr></thead>\n<tbody>\n",
        report.total_orphans,
        if report.total_orphans == 1 { "" } else { "s" },
    );
    for e in &report.entries {
        let href = artifact_href(
            base,
            &e.project_slug,
            &e.collection_prefix,
            &e.artifact_name,
        );
        let status_pills = {
            let mut s = String::new();
            if !e.active {
                s.push_str(" <span class=\"pill\">inactive</span>");
            }
            if e.derived {
                s.push_str(" <span class=\"pill warn\">derived</span>");
            }
            s
        };
        body.push_str(&format!(
            "<tr><td><a href=\"{href}\"><code>{p}/{c}/{n}</code></a> <span class=\"summary\">{title}</span></td><td><code>{shape}</code></td><td>{status}</td></tr>\n",
            href = escape(&href),
            p = escape(&e.project_slug),
            c = escape(&e.collection_prefix),
            n = escape(&e.artifact_name),
            title = escape(&e.title),
            shape = shape_label(e.shape),
            status = status_pills,
        ));
    }
    body.push_str("</tbody>\n</table>\n");
    body
}

fn render_cycles(report: &CyclesReport, base: &str) -> String {
    let mut body = String::new();
    let checked_html: Vec<String> = report
        .link_types_checked
        .iter()
        .map(|t| format!("<code>{}</code>", escape(t)))
        .collect();
    body.push_str(&format!(
        "<p class=\"summary\">Checked link types: {}</p>\n",
        checked_html.join(", ")
    ));
    if report.truncated {
        body.push_str("<p><span class=\"pill warn\">truncated</span> at least one link type hit the per-type cap. Resolve these cycles and re-run the report.</p>\n");
    }
    if report.total_cycles == 0 {
        body.push_str("<p>No cycles in scope.</p>");
        return body;
    }
    for (idx, cycle) in report.cycles.iter().enumerate() {
        body.push_str(&format!(
            "<h2 class=\"section-h2\">Cycle {idx} · <code>{lt}</code> · {n} nodes</h2>\n<p>",
            idx = idx + 1,
            lt = escape(&cycle.link_type),
            n = cycle.nodes.len(),
        ));
        let parts: Vec<String> = cycle
            .nodes
            .iter()
            .map(|n| artifact_link_cell(base, n))
            .collect();
        // Close the loop by appending the first node faded.
        let mut joined = parts.join(" → ");
        if let Some(first) = cycle.nodes.first() {
            joined.push_str(" → ");
            joined.push_str(&format!(
                "<span class=\"summary\">{p}/{c}/{n}</span>",
                p = escape(&first.project_slug),
                c = escape(&first.collection_prefix),
                n = escape(&first.artifact_name),
            ));
        }
        body.push_str(&joined);
        body.push_str("</p>\n");
    }
    body
}

fn render_conflicts(report: &ConflictsReport, base: &str) -> String {
    if report.total_pairs == 0 {
        return "<p>No conflict pairs in scope.</p>".to_owned();
    }
    let mut body = format!(
        "<p class=\"summary\">{} conflict pair{}.</p>\n<table>\n<thead><tr><th>First</th><th>Second</th><th>Direction</th></tr></thead>\n<tbody>\n",
        report.total_pairs,
        if report.total_pairs == 1 { "" } else { "s" },
    );
    for p in &report.pairs {
        let dir = if p.bidirectional {
            "<span class=\"pill good\">bidirectional</span>"
        } else {
            "<span class=\"pill warn\">one-sided</span>"
        };
        body.push_str(&format!(
            "<tr><td>{first}</td><td>{second}</td><td>{dir}</td></tr>\n",
            first = artifact_link_cell(base, &p.first),
            second = artifact_link_cell(base, &p.second),
        ));
    }
    body.push_str("</tbody>\n</table>\n");
    body
}

fn render_coverage_matrix(report: &CoverageMatrixReport, base: &str) -> String {
    let mut body = String::new();
    if !report.unknown_requested_types.is_empty() {
        let list: Vec<String> = report
            .unknown_requested_types
            .iter()
            .map(|t| format!("<code>{}</code>", escape(t)))
            .collect();
        body.push_str(&format!(
            "<p><span class=\"pill warn\">warning</span> ignored unknown link type(s): {}.</p>\n",
            list.join(", "),
        ));
    }
    body.push_str(&format!(
        "<p class=\"summary\">{} parent artifact{} · <span class=\"{}\">{} gap{}</span></p>\n",
        report.total_parents,
        if report.total_parents == 1 { "" } else { "s" },
        if report.gap_count > 0 { "gap" } else { "" },
        report.gap_count,
        if report.gap_count == 1 { "" } else { "s" },
    ));
    body.push_str("<table>\n<thead><tr><th>Parent</th>");
    for lt in &report.covering_link_types {
        body.push_str(&format!("<th>{}</th>", escape(lt)));
    }
    body.push_str("<th>Gap?</th></tr></thead>\n<tbody>\n");
    for p in &report.parents {
        body.push_str(&format!(
            "<tr><td>{}</td>",
            artifact_link_cell(base, &p.parent)
        ));
        for lt in &report.covering_link_types {
            let n = p
                .covering_children
                .iter()
                .filter(|c| c.link_type == *lt)
                .count();
            body.push_str(&format!(
                "<td class=\"{}\">{}</td>",
                if n == 0 { "gap" } else { "covered" },
                n,
            ));
        }
        body.push_str(&format!(
            "<td>{}</td></tr>\n",
            if p.has_gap {
                "<span class=\"pill bad\">gap</span>"
            } else {
                "<span class=\"pill good\">covered</span>"
            },
        ));
    }
    body.push_str("</tbody>\n</table>\n");
    body
}

fn render_impact_analysis(report: &ImpactAnalysisReport, base: &str) -> String {
    let mut body = String::new();
    if let Some(reason) = &report.missing_seed_reason {
        body.push_str(&format!(
            "<p><span class=\"pill warn\">no seed</span> {}</p>",
            escape(reason)
        ));
        return body;
    }
    if let Some(seed) = &report.seed {
        body.push_str(&format!(
            "<p>Seed: {seed} · direction <code>{dir}</code></p>\n",
            seed = artifact_link_cell(base, seed),
            dir = escape(&report.direction),
        ));
    }
    if report.total_impacted == 0 {
        body.push_str("<p>Seed has no impacted artifacts in scope.</p>");
        return body;
    }
    body.push_str(&format!(
        "<p class=\"summary\">{} impacted artifact{}.</p>\n<table>\n<thead><tr><th>Depth</th><th>Artifact</th><th>Link types</th></tr></thead>\n<tbody>\n",
        report.total_impacted,
        if report.total_impacted == 1 { "" } else { "s" },
    ));
    for e in &report.impacted {
        let lts: Vec<String> = e
            .link_types
            .iter()
            .map(|t| format!("<code>{}</code>", escape(t)))
            .collect();
        body.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>\n",
            e.depth,
            artifact_link_cell(base, &e.node),
            lts.join(", "),
        ));
    }
    body.push_str("</tbody>\n</table>\n");
    body
}

fn render_review_status(report: &ReviewStatusReport) -> String {
    let mut body = String::new();
    body.push_str("<h2 class=\"section-h2\">Totals</h2>\n");
    body.push_str(&counts_table(&[("Total", &report.totals)]));
    body.push_str("<h2 class=\"section-h2\">By shape</h2>\n");
    body.push_str(&counts_table(&[
        ("Content", &report.by_shape.content),
        ("Blob", &report.by_shape.blob),
        ("URL", &report.by_shape.url),
    ]));
    if !report.by_project.is_empty() {
        body.push_str("<h2 class=\"section-h2\">By project</h2>\n");
        let rows: Vec<(String, &crate::reports::ReviewStatusCounts)> = report
            .by_project
            .iter()
            .map(|p| (p.project_slug.clone(), &p.counts))
            .collect();
        body.push_str(&counts_table_owned(&rows));
    }
    if !report.by_collection.is_empty() {
        body.push_str("<h2 class=\"section-h2\">By collection</h2>\n");
        let rows: Vec<(String, &crate::reports::ReviewStatusCounts)> = report
            .by_collection
            .iter()
            .map(|c| {
                (
                    format!("{}/{}", c.project_slug, c.collection_prefix),
                    &c.counts,
                )
            })
            .collect();
        body.push_str(&counts_table_owned(&rows));
    }
    body
}

fn counts_table(rows: &[(&str, &crate::reports::ReviewStatusCounts)]) -> String {
    let mut s = String::from(
        "<table>\n<thead><tr><th></th><th>Approved</th><th>Rejected</th><th>Re-requested</th><th>Never reviewed</th><th>Total</th></tr></thead>\n<tbody>\n",
    );
    for (label, c) in rows {
        s.push_str(&format!(
            "<tr><td><code>{label}</code></td><td class=\"covered\">{a}</td><td class=\"gap\">{r}</td><td>{rr}</td><td class=\"summary\">{nr}</td><td><b>{t}</b></td></tr>\n",
            label = escape(label),
            a = c.approved,
            r = c.rejected,
            rr = c.re_requested,
            nr = c.never_reviewed,
            t = c.total(),
        ));
    }
    s.push_str("</tbody>\n</table>\n");
    s
}

fn counts_table_owned(rows: &[(String, &crate::reports::ReviewStatusCounts)]) -> String {
    let refs: Vec<(&str, &crate::reports::ReviewStatusCounts)> =
        rows.iter().map(|(l, c)| (l.as_str(), *c)).collect();
    counts_table(&refs)
}

fn render_filesystem_orphans(report: &FilesystemOrphansReport) -> String {
    let mut body = String::new();
    if report.missing_sidecar.is_empty() && report.missing_binary.is_empty() {
        body.push_str("<p>No filesystem-orphans in scope.</p>");
        return body;
    }
    if !report.missing_sidecar.is_empty() {
        body.push_str(&format!(
            "<h2 class=\"section-h2\">Binaries without a sidecar ({})</h2>\n<table>\n<thead><tr><th>File</th><th>Collection</th><th>Media type</th><th>Size</th></tr></thead>\n<tbody>\n",
            report.missing_sidecar.len(),
        ));
        for e in &report.missing_sidecar {
            body.push_str(&format!(
                "<tr><td><code>{p}</code></td><td><code>{proj}/{coll}</code></td><td><code>{mt}</code></td><td>{size}</td></tr>\n",
                p = escape(&e.binary_relative_path),
                proj = escape(&e.project_slug),
                coll = escape(&e.collection_prefix),
                mt = escape(e.media_type),
                size = e.byte_size,
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }
    if !report.missing_binary.is_empty() {
        body.push_str(&format!(
            "<h2 class=\"section-h2\">Sidecars whose blob is missing ({})</h2>\n<p class=\"summary\">ReqForge will never delete these automatically. Restore the binary via git / filesystem, or delete the sidecar manually.</p>\n<table>\n<thead><tr><th>Sidecar</th><th>Collection</th><th>Declared blob path</th></tr></thead>\n<tbody>\n",
            report.missing_binary.len(),
        ));
        for e in &report.missing_binary {
            body.push_str(&format!(
                "<tr><td><code>{s}</code></td><td><code>{p}/{c}</code></td><td><code>{b}</code></td></tr>\n",
                s = escape(&e.sidecar_filename),
                p = escape(&e.project_slug),
                c = escape(&e.collection_prefix),
                b = escape(&e.declared_blob_path),
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }
    body
}

fn render_code_traceability(report: &crate::reports::CodeTraceabilityReport) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "<p class=\"summary\">{} artifact{} in scope · {} uncovered · {} orphan tag{}</p>\n",
        report.total_artifacts,
        if report.total_artifacts == 1 { "" } else { "s" },
        report.uncovered_count,
        report.orphan_tag_count,
        if report.orphan_tag_count == 1 {
            ""
        } else {
            "s"
        },
    ));
    if report.entries.is_empty() {
        body.push_str("<p>No artifacts in scope.</p>");
    } else {
        body.push_str(
            "<h2 class=\"section-h2\">Artifacts</h2>\n<table>\n<thead><tr><th>Artifact</th><th>expectsCodeTrace</th><th>Status</th><th>Locations</th></tr></thead>\n<tbody>\n",
        );
        for entry in &report.entries {
            let status = if entry.has_gap {
                "<span class=\"gap\">gap</span>"
            } else if entry.locations_by_verb.is_empty() {
                "<span class=\"muted\">no tags</span>"
            } else {
                "<span>covered</span>"
            };
            let mut locations_html = String::new();
            if entry.locations_by_verb.is_empty() {
                locations_html.push_str("<span class=\"muted\">—</span>");
            } else {
                for (verb, locations) in &entry.locations_by_verb {
                    locations_html
                        .push_str(&format!("<div><strong>{v}</strong>: ", v = escape(verb)));
                    for (idx, loc) in locations.iter().enumerate() {
                        if idx > 0 {
                            locations_html.push_str(", ");
                        }
                        locations_html.push_str(&format!(
                            "<code>{f}:{l}</code>",
                            f = escape(&loc.file.display().to_string()),
                            l = loc.line,
                        ));
                    }
                    locations_html.push_str("</div>");
                }
            }
            body.push_str(&format!(
                "<tr><td><code>{proj}/{col}/{name}</code></td><td>{ect}</td><td>{status}</td><td>{locs}</td></tr>\n",
                proj = escape(&entry.artifact.project_slug),
                col = escape(&entry.artifact.collection_prefix),
                name = escape(&entry.artifact.artifact_name),
                ect = entry.expects_code_trace,
                status = status,
                locs = locations_html,
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }
    if !report.orphan_tags.is_empty() {
        body.push_str(&format!(
            "<h2 class=\"section-h2\">Orphan tags ({})</h2>\n<p class=\"summary\">Tags in source that don't resolve to any mounted artifact — typically a rename or a typo.</p>\n<table>\n<thead><tr><th>Verb</th><th>Raw ID</th><th>File</th><th>Line</th></tr></thead>\n<tbody>\n",
            report.orphan_tags.len(),
        ));
        for o in &report.orphan_tags {
            body.push_str(&format!(
                "<tr><td>{v}</td><td><code>{id}</code></td><td><code>{f}</code></td><td>{l}</td></tr>\n",
                v = escape(&o.verb),
                id = escape(&o.raw_id),
                f = escape(&o.file.display().to_string()),
                l = o.line,
            ));
        }
        body.push_str("</tbody>\n</table>\n");
    }
    body
}

fn shape_label(shape: crate::schema::ArtifactShape) -> &'static str {
    match shape {
        crate::schema::ArtifactShape::Content => "content",
        crate::schema::ArtifactShape::Blob => "blob",
        crate::schema::ArtifactShape::Url => "url",
    }
}

/// Convenience accessor for `ReportResponse::scope()` used above.
/// Implemented here rather than on the enum because the scope
/// lives on each variant's inner struct.
impl ReportResponse {
    fn scope(&self) -> &ScopeDto {
        match self {
            ReportResponse::UnresolvedLinks(r) => &r.scope,
            ReportResponse::LinkOrphans(r) => &r.scope,
            ReportResponse::Cycles(r) => &r.scope,
            ReportResponse::Conflicts(r) => &r.scope,
            ReportResponse::CoverageMatrix(r) => &r.scope,
            ReportResponse::ImpactAnalysis(r) => &r.scope,
            ReportResponse::ReviewStatus(r) => &r.scope,
            ReportResponse::FilesystemOrphans(r) => &r.scope,
            ReportResponse::CodeTraceability(r) => &r.scope,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::{LinkOrphansReport, ScopeDto, UnresolvedLinkEntry, UnresolvedLinksReport};
    use crate::schema::ArtifactShape;
    use uuid::Uuid;

    fn uuid_of(n: u8) -> Uuid {
        let mut b = [0u8; 16];
        b[15] = n;
        b[6] = 0x70 | n;
        Uuid::from_bytes(b)
    }

    #[test]
    fn escape_handles_the_five_reserved_chars() {
        assert_eq!(
            escape("<b>&\"'<script>"),
            "&lt;b&gt;&amp;&quot;&#x27;&lt;script&gt;"
        );
    }

    #[test]
    fn artifact_href_composes_absolute_url_with_external_base() {
        let href = artifact_href("https://ex.com/", "sample", "REQ", "REQ-a");
        assert_eq!(
            href,
            "https://ex.com/projects/sample/collections/REQ/artifacts/REQ-a"
        );
    }

    #[test]
    fn artifact_href_empty_base_produces_relative_path() {
        let href = artifact_href("", "sample", "REQ", "REQ-a");
        assert_eq!(href, "/projects/sample/collections/REQ/artifacts/REQ-a");
    }

    #[test]
    fn html_doc_has_title_scope_and_one_anchor_per_row() {
        let report = UnresolvedLinksReport {
            scope: ScopeDto::System,
            total_unresolved: 1,
            entries: vec![UnresolvedLinkEntry {
                source_uuid: uuid_of(1),
                source_project_slug: "sample".into(),
                source_collection_prefix: "REQ".into(),
                source_artifact_name: "REQ-a".into(),
                source_title: "A".into(),
                source_shape: ArtifactShape::Content,
                link_type: "derives-from".into(),
                target_uuid: uuid_of(2),
                target_hint_project_slug: "sample".into(),
                target_hint_collection_prefix: "DES".into(),
                target_hint_artifact_name: "DES-ghost".into(),
                reason: "target-missing",
            }],
        };
        let bytes = render(
            &ReportResponse::UnresolvedLinks(report),
            Some("https://ex.com"),
        );
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("<!DOCTYPE html>"));
        assert!(text.contains("<title>ReqForge · Unresolved links · system</title>"));
        assert!(text.contains("https://ex.com/projects/sample/collections/REQ/artifacts/REQ-a"));
        assert!(text.contains("target-missing"));
    }

    #[test]
    fn html_doc_for_link_orphans_marks_empty_state() {
        let report = LinkOrphansReport {
            scope: ScopeDto::System,
            total_orphans: 0,
            entries: Vec::new(),
        };
        let bytes = render(&ReportResponse::LinkOrphans(report), None);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("No link-graph orphans in scope."));
    }
}
