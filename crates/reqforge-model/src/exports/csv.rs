//! Per-kind CSV renderers (Phase 6b).
//!
//! The `csv` crate handles quoting, embedded newlines, and
//! UTF-8 correctly — we just pick the schema per report kind and
//! stream records in. Each renderer is a pure function so the
//! handler's dispatch stays a thin `match`.
//!
//! Column schemas are stable (per `REPORT-baselineExports`) —
//! adding a column is a minor-version bump worth coordinating
//! with operator tooling.

use std::io::Write;

use crate::reports::{
    ConflictsReport, CoverageMatrixReport, FilesystemOrphansReport, ImpactAnalysisReport,
    LinkOrphansReport, ReviewStatusReport, UnresolvedLinksReport,
};

/// All list-shaped reports follow the same row-per-entry shape;
/// this helper does the mechanical csv::Writer dance so the
/// per-kind functions stay focused on the schema.
fn writer() -> csv::Writer<Vec<u8>> {
    csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(Vec::new())
}

fn finish(w: csv::Writer<Vec<u8>>) -> Vec<u8> {
    w.into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>")
}

pub fn unresolved_links(report: &UnresolvedLinksReport) -> Vec<u8> {
    let mut w = writer();
    w.write_record([
        "source_project",
        "source_collection",
        "source_artifact",
        "source_title",
        "source_shape",
        "link_type",
        "target_uuid",
        "target_hint_project",
        "target_hint_collection",
        "target_hint_artifact",
        "reason",
    ])
    .unwrap();
    for e in &report.entries {
        w.write_record([
            e.source_project_slug.as_str(),
            e.source_collection_prefix.as_str(),
            e.source_artifact_name.as_str(),
            e.source_title.as_str(),
            shape_str(e.source_shape),
            e.link_type.as_str(),
            &e.target_uuid.to_string(),
            e.target_hint_project_slug.as_str(),
            e.target_hint_collection_prefix.as_str(),
            e.target_hint_artifact_name.as_str(),
            e.reason,
        ])
        .unwrap();
    }
    finish(w)
}

pub fn link_orphans(report: &LinkOrphansReport) -> Vec<u8> {
    let mut w = writer();
    w.write_record([
        "project",
        "collection",
        "artifact",
        "title",
        "shape",
        "active",
        "derived",
    ])
    .unwrap();
    for e in &report.entries {
        w.write_record([
            e.project_slug.as_str(),
            e.collection_prefix.as_str(),
            e.artifact_name.as_str(),
            e.title.as_str(),
            shape_str(e.shape),
            bool_str(e.active),
            bool_str(e.derived),
        ])
        .unwrap();
    }
    finish(w)
}

pub fn conflicts(report: &ConflictsReport) -> Vec<u8> {
    let mut w = writer();
    w.write_record([
        "first_project",
        "first_collection",
        "first_artifact",
        "first_title",
        "second_project",
        "second_collection",
        "second_artifact",
        "second_title",
        "bidirectional",
    ])
    .unwrap();
    for p in &report.pairs {
        w.write_record([
            p.first.project_slug.as_str(),
            p.first.collection_prefix.as_str(),
            p.first.artifact_name.as_str(),
            p.first.title.as_str(),
            p.second.project_slug.as_str(),
            p.second.collection_prefix.as_str(),
            p.second.artifact_name.as_str(),
            p.second.title.as_str(),
            bool_str(p.bidirectional),
        ])
        .unwrap();
    }
    finish(w)
}

/// Compact parent × covering-link-type matrix per the locked
/// decision. One row per parent, one column per covering link
/// type, plus a trailing `has_gap` column so grep on the CSV
/// still finds the gap rows.
pub fn coverage_matrix(report: &CoverageMatrixReport) -> Vec<u8> {
    let mut w = writer();
    // Header row: project, collection, artifact, title, then one
    // column per covering link type, then has_gap.
    let mut header: Vec<String> = vec![
        "project".into(),
        "collection".into(),
        "artifact".into(),
        "title".into(),
    ];
    for lt in &report.covering_link_types {
        header.push(lt.clone());
    }
    header.push("has_gap".into());
    w.write_record(&header).unwrap();

    for parent in &report.parents {
        let mut row: Vec<String> = vec![
            parent.parent.project_slug.clone(),
            parent.parent.collection_prefix.clone(),
            parent.parent.artifact_name.clone(),
            parent.parent.title.clone(),
        ];
        for lt in &report.covering_link_types {
            let count = parent
                .covering_children
                .iter()
                .filter(|c| c.link_type == *lt)
                .count();
            row.push(count.to_string());
        }
        row.push(bool_str(parent.has_gap).to_owned());
        w.write_record(&row).unwrap();
    }
    finish(w)
}

/// Impact-analysis ships a row-per-impacted CSV. The seed is
/// echoed in the filename and a leading `# seed=...` comment
/// line so the CSV is self-describing.
pub fn impact_analysis(report: &ImpactAnalysisReport) -> Vec<u8> {
    let mut raw: Vec<u8> = Vec::new();
    if let Some(seed) = &report.seed {
        let _ = writeln!(
            raw,
            "# seed={}/{}/{} ({}) direction={}",
            seed.project_slug,
            seed.collection_prefix,
            seed.artifact_name,
            seed.uuid,
            report.direction,
        );
    } else if let Some(reason) = &report.missing_seed_reason {
        let _ = writeln!(raw, "# seed=unresolved ({})", reason);
    }
    let mut w = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(raw);
    w.write_record([
        "depth",
        "project",
        "collection",
        "artifact",
        "title",
        "shape",
        "active",
        "link_types",
    ])
    .unwrap();
    for e in &report.impacted {
        w.write_record([
            e.depth.to_string().as_str(),
            e.node.project_slug.as_str(),
            e.node.collection_prefix.as_str(),
            e.node.artifact_name.as_str(),
            e.node.title.as_str(),
            shape_str(e.node.shape),
            bool_str(e.node.active),
            e.link_types.join("|").as_str(),
        ])
        .unwrap();
    }
    w.into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>")
}

pub fn review_status(report: &ReviewStatusReport) -> Vec<u8> {
    let mut w = writer();
    w.write_record([
        "facet",
        "key",
        "approved",
        "rejected",
        "re_requested",
        "never_reviewed",
        "total",
    ])
    .unwrap();
    // Emit totals first, then by-shape, then by-project, then
    // by-collection — in that order so operators can grep on
    // the facet column.
    let counts = &report.totals;
    w.write_record([
        "totals",
        "-",
        &counts.approved.to_string(),
        &counts.rejected.to_string(),
        &counts.re_requested.to_string(),
        &counts.never_reviewed.to_string(),
        &counts.total().to_string(),
    ])
    .unwrap();
    for (label, c) in [
        ("shape:content", &report.by_shape.content),
        ("shape:blob", &report.by_shape.blob),
        ("shape:url", &report.by_shape.url),
    ] {
        w.write_record([
            "by-shape",
            label,
            &c.approved.to_string(),
            &c.rejected.to_string(),
            &c.re_requested.to_string(),
            &c.never_reviewed.to_string(),
            &c.total().to_string(),
        ])
        .unwrap();
    }
    for p in &report.by_project {
        w.write_record([
            "by-project",
            p.project_slug.as_str(),
            &p.counts.approved.to_string(),
            &p.counts.rejected.to_string(),
            &p.counts.re_requested.to_string(),
            &p.counts.never_reviewed.to_string(),
            &p.counts.total().to_string(),
        ])
        .unwrap();
    }
    for c in &report.by_collection {
        w.write_record([
            "by-collection",
            &format!("{}/{}", c.project_slug, c.collection_prefix),
            &c.counts.approved.to_string(),
            &c.counts.rejected.to_string(),
            &c.counts.re_requested.to_string(),
            &c.counts.never_reviewed.to_string(),
            &c.counts.total().to_string(),
        ])
        .unwrap();
    }
    finish(w)
}

/// Filesystem-orphans exports two logical tables in one CSV
/// separated by a `#### <section>` marker line. Each section's
/// column schema is stable.
pub fn filesystem_orphans(report: &FilesystemOrphansReport) -> Vec<u8> {
    let mut raw: Vec<u8> = Vec::new();
    let _ = writeln!(raw, "#### missing-sidecar");
    let mut w = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(raw);
    w.write_record([
        "project",
        "collection",
        "filename",
        "binary_relative_path",
        "byte_size",
        "media_type",
    ])
    .unwrap();
    for e in &report.missing_sidecar {
        w.write_record([
            e.project_slug.as_str(),
            e.collection_prefix.as_str(),
            e.filename.as_str(),
            e.binary_relative_path.as_str(),
            &e.byte_size.to_string(),
            e.media_type,
        ])
        .unwrap();
    }
    let mut raw = w
        .into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>");
    let _ = writeln!(raw);
    let _ = writeln!(raw, "#### missing-binary");
    let mut w = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(raw);
    w.write_record([
        "project",
        "collection",
        "sidecar_filename",
        "declared_blob_path",
    ])
    .unwrap();
    for e in &report.missing_binary {
        w.write_record([
            e.project_slug.as_str(),
            e.collection_prefix.as_str(),
            e.sidecar_filename.as_str(),
            e.declared_blob_path.as_str(),
        ])
        .unwrap();
    }
    w.into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>")
}

/// Code-traceability exports two logical tables: per-artifact
/// locations keyed by verb, and orphan tags. Mirrors the
/// filesystem-orphans two-section layout — one CSV with a
/// `#### <section>` separator line so the output stays
/// operator-readable in a spreadsheet.
pub fn code_traceability(report: &crate::reports::CodeTraceabilityReport) -> Vec<u8> {
    let mut raw: Vec<u8> = Vec::new();
    let _ = writeln!(raw, "#### locations");
    let mut w = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(raw);
    w.write_record([
        "project",
        "collection",
        "artifact",
        "expects_code_trace",
        "has_gap",
        "verb",
        "file",
        "line",
    ])
    .unwrap();
    for entry in &report.entries {
        if entry.locations_by_verb.is_empty() {
            // Still emit a row so uncovered artifacts surface
            // in the CSV export; the verb/file/line columns
            // stay empty.
            w.write_record([
                entry.artifact.project_slug.as_str(),
                entry.artifact.collection_prefix.as_str(),
                entry.artifact.artifact_name.as_str(),
                bool_str(entry.expects_code_trace),
                bool_str(entry.has_gap),
                "",
                "",
                "",
            ])
            .unwrap();
            continue;
        }
        for (verb, locations) in &entry.locations_by_verb {
            for loc in locations {
                w.write_record([
                    entry.artifact.project_slug.as_str(),
                    entry.artifact.collection_prefix.as_str(),
                    entry.artifact.artifact_name.as_str(),
                    bool_str(entry.expects_code_trace),
                    bool_str(entry.has_gap),
                    verb.as_str(),
                    &loc.file.display().to_string(),
                    &loc.line.to_string(),
                ])
                .unwrap();
            }
        }
    }
    let mut raw = w
        .into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>");
    let _ = writeln!(raw);
    let _ = writeln!(raw, "#### orphan-tags");
    let mut w = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(raw);
    w.write_record(["file", "line", "verb", "raw_id"]).unwrap();
    for o in &report.orphan_tags {
        w.write_record([
            &o.file.display().to_string(),
            &o.line.to_string(),
            o.verb.as_str(),
            o.raw_id.as_str(),
        ])
        .unwrap();
    }
    w.into_inner()
        .expect("csv::Writer::into_inner should not fail on Vec<u8>")
}

fn shape_str(shape: crate::schema::ArtifactShape) -> &'static str {
    match shape {
        crate::schema::ArtifactShape::Content => "content",
        crate::schema::ArtifactShape::Blob => "blob",
        crate::schema::ArtifactShape::Url => "url",
    }
}

fn bool_str(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reports::{
        ConflictPair, CoverageChildEntry, CoverageParentEntry, CycleNode, LinkOrphanEntry,
        ReviewStatusByShape, ReviewStatusCounts, ScopeDto, UnresolvedLinkEntry,
    };
    use crate::schema::ArtifactShape;
    use uuid::Uuid;

    fn test_uuid() -> Uuid {
        Uuid::parse_str("00000000-0000-7000-8000-000000000001").unwrap()
    }

    fn node(project: &str, coll: &str, name: &str, title: &str) -> CycleNode {
        CycleNode {
            uuid: test_uuid(),
            project_slug: project.into(),
            collection_prefix: coll.into(),
            artifact_name: name.into(),
            title: title.into(),
            shape: ArtifactShape::Content,
            active: true,
        }
    }

    #[test]
    fn unresolved_links_csv_has_expected_header_and_rows() {
        let report = UnresolvedLinksReport {
            scope: ScopeDto::System,
            total_unresolved: 1,
            entries: vec![UnresolvedLinkEntry {
                source_uuid: test_uuid(),
                source_project_slug: "sample".into(),
                source_collection_prefix: "REQ".into(),
                source_artifact_name: "REQ-a".into(),
                source_title: "A".into(),
                source_shape: ArtifactShape::Content,
                link_type: "derives-from".into(),
                target_uuid: test_uuid(),
                target_hint_project_slug: "sample".into(),
                target_hint_collection_prefix: "DES".into(),
                target_hint_artifact_name: "DES-a".into(),
                reason: "target-missing",
            }],
        };
        let text = String::from_utf8(unresolved_links(&report)).unwrap();
        assert!(text.starts_with("source_project,"));
        assert!(text.contains("target-missing"));
        assert!(text.contains("REQ-a"));
    }

    #[test]
    fn link_orphans_csv_quotes_titles_with_commas() {
        let report = LinkOrphansReport {
            scope: ScopeDto::System,
            total_orphans: 1,
            entries: vec![LinkOrphanEntry {
                uuid: test_uuid(),
                project_slug: "sample".into(),
                collection_prefix: "REQ".into(),
                artifact_name: "REQ-solo".into(),
                title: "Title, with comma".into(),
                shape: ArtifactShape::Content,
                active: true,
                derived: false,
            }],
        };
        let text = String::from_utf8(link_orphans(&report)).unwrap();
        assert!(
            text.contains("\"Title, with comma\""),
            "csv crate should quote the comma: got {text}"
        );
    }

    #[test]
    fn coverage_matrix_csv_has_compact_layout() {
        let report = CoverageMatrixReport {
            scope: ScopeDto::System,
            covering_link_types: vec!["satisfies".into(), "verifies".into()],
            unknown_requested_types: vec![],
            total_parents: 2,
            gap_count: 1,
            parents: vec![
                CoverageParentEntry {
                    parent: node("sample", "REQ", "REQ-a", "A"),
                    has_gap: false,
                    covering_children: vec![CoverageChildEntry {
                        child: node("sample", "DES", "DES-a", "Design"),
                        link_type: "satisfies".into(),
                    }],
                    covering_code_evidence: Vec::new(),
                },
                CoverageParentEntry {
                    parent: node("sample", "REQ", "REQ-b", "B"),
                    has_gap: true,
                    covering_children: vec![],
                    covering_code_evidence: Vec::new(),
                },
            ],
        };
        let text = String::from_utf8(coverage_matrix(&report)).unwrap();
        // Header lists satisfies + verifies + has_gap.
        assert!(text.contains("satisfies,verifies,has_gap"));
        // REQ-a row has 1 under satisfies, 0 under verifies, false gap.
        assert!(text.contains("REQ-a,A,1,0,false"));
        // REQ-b row has all zeros + true gap.
        assert!(text.contains("REQ-b,B,0,0,true"));
    }

    #[test]
    fn conflicts_csv_emits_sorted_pair_and_direction_flag() {
        let report = ConflictsReport {
            scope: ScopeDto::System,
            total_pairs: 1,
            pairs: vec![ConflictPair {
                first: node("sample", "REQ", "REQ-a", "A"),
                second: node("sample", "REQ", "REQ-b", "B"),
                bidirectional: true,
            }],
        };
        let text = String::from_utf8(conflicts(&report)).unwrap();
        assert!(text.contains("REQ-a,A,sample,REQ,REQ-b,B,true"));
    }

    #[test]
    fn review_status_csv_emits_totals_then_facets() {
        let totals = ReviewStatusCounts {
            approved: 1,
            rejected: 1,
            re_requested: 0,
            never_reviewed: 0,
        };
        let by_shape = ReviewStatusByShape {
            content: ReviewStatusCounts {
                approved: 1,
                rejected: 0,
                re_requested: 0,
                never_reviewed: 0,
            },
            blob: ReviewStatusCounts::default(),
            url: ReviewStatusCounts::default(),
        };
        let report = ReviewStatusReport {
            scope: ScopeDto::System,
            totals,
            by_project: vec![],
            by_collection: vec![],
            by_shape,
        };
        let text = String::from_utf8(review_status(&report)).unwrap();
        assert!(text.contains("totals,-,1,1,"));
        assert!(text.contains("by-shape,shape:content,1,"));
    }

    #[test]
    fn filesystem_orphans_csv_uses_two_section_headers() {
        use crate::reports::{OrphanBinary, OrphanSidecar};
        let report = FilesystemOrphansReport {
            scope: ScopeDto::System,
            missing_sidecar: vec![OrphanBinary {
                project_slug: "sample".into(),
                collection_prefix: "DES".into(),
                filename: "DES-logo.png".into(),
                binary_relative_path: "artifacts/DES/DES-logo.png".into(),
                byte_size: 123,
                media_type: "image/png",
            }],
            missing_binary: vec![OrphanSidecar {
                project_slug: "sample".into(),
                collection_prefix: "DES".into(),
                sidecar_filename: "DES-ghost.pdf.reqforge.json".into(),
                declared_blob_path: "artifacts/DES/DES-ghost.pdf".into(),
            }],
        };
        let text = String::from_utf8(filesystem_orphans(&report)).unwrap();
        assert!(text.contains("#### missing-sidecar"));
        assert!(text.contains("#### missing-binary"));
        assert!(text.contains("DES-logo.png"));
        assert!(text.contains("DES-ghost.pdf.reqforge.json"));
    }

    #[test]
    fn impact_analysis_csv_carries_seed_comment_line() {
        use crate::reports::{ImpactAnalysisReport, ImpactedArtifact};
        let seed_node = node("sample", "REQ", "REQ-a", "A");
        let report = ImpactAnalysisReport {
            scope: ScopeDto::System,
            seed: Some(seed_node.clone()),
            direction: "dependents".into(),
            total_impacted: 1,
            impacted: vec![ImpactedArtifact {
                node: node("sample", "REQ", "REQ-b", "B"),
                depth: 1,
                link_types: vec!["derives-from".into()],
            }],
            missing_seed_reason: None,
        };
        let text = String::from_utf8(impact_analysis(&report)).unwrap();
        assert!(text.starts_with("# seed=sample/REQ/REQ-a"));
        assert!(text.contains("direction=dependents"));
        assert!(text.contains("depth,project,collection,"));
        assert!(text.contains("1,sample,REQ,REQ-b"));
    }
}
