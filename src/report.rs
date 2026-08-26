//! The on-demand traceability report (Phase 4d, #340) — the named deliverable of Phase 4. One
//! row per requirement: is it formalized, what code implements it, what tests verify it, and what
//! the last stored verdict says (proven / not-determined / disproven) — with the honest
//! `correspondence` marker, so an asserted verdict never reads as a mechanical one.
//!
//! Pure assembly over data other slices already produce: [`crate::status::backlog`] pairs each
//! item with its formalization state and last stored verdict; [`crate::trace::scan`] finds the
//! `Implements:`/`Verifies:` tags. Nothing is run here — the verdict is read from the store
//! (`provreq verify` produced it), so a report is fast and free of side effects. The Rust successor
//! to `scripts/traceability.py`'s report, which slice (g) retires.
//!
//! Implements: REQ077

use crate::status::{Formalization, ItemState};
use crate::trace::{Tag, TraceKind};
use std::collections::BTreeSet;
use std::path::Path;

/// A tag's location, for a report row's implements/verifies cells.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
    pub symbol: Option<String>,
}

impl Location {
    fn of(tag: &Tag) -> Location {
        Location {
            file: tag.file.display().to_string(),
            line: tag.line,
            symbol: tag.symbol.clone(),
        }
    }
}

/// What the last stored verdict says about a requirement, in the report's vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerdictCell {
    /// `proven` (a `holds`), `disproven` (a `fails`), or `not-determined` (an `unknown`).
    pub outcome: String,
    /// The D8 basis behind a `proven` (e.g. `proven`, `not-falsified`) — `None` otherwise.
    pub basis: Option<String>,
    /// `mechanical` or `asserted` — the masquerade guard, carried onto every row.
    pub correspondence: String,
    /// Whether the stored verdict is still anchored to the current world (REQ039).
    pub fresh: bool,
}

/// One requirement's row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: String,
    pub title: Option<String>,
    pub formalized: bool,
    pub implements: Vec<Location>,
    pub verifies: Vec<Location>,
    /// `None` when the requirement has never been verified — distinct from a recorded
    /// `not-determined`, which means an engine or a tagged test ran and could not decide.
    pub verdict: Option<VerdictCell>,
}

/// A tag naming a requirement id the subject does not declare.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Orphan {
    pub req_id: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
}

/// The whole report: a row per requirement, plus orphan tags.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub rows: Vec<Row>,
    pub orphans: Vec<Orphan>,
}

/// A requirement id compared `-`/`_`-insensitively and case-folded, so a `REQ-021` tag matches
/// requirement `REQ021` (the same normalisation the verify flow and the traceability reader use).
fn canonical(id: &str) -> String {
    id.chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect::<String>()
        .to_ascii_uppercase()
}

fn verdict_cell(state: &ItemState) -> Option<VerdictCell> {
    let view = state.verdict.as_ref()?;
    let outcome = match view.status.as_str() {
        "holds" => "proven",
        "fails" => "disproven",
        _ => "not-determined",
    };
    Some(VerdictCell {
        outcome: outcome.to_string(),
        basis: view.basis.clone(),
        correspondence: view.correspondence.clone(),
        fresh: view.fresh,
    })
}

/// Assemble the report from the per-item states (formalization + stored verdict) and the scanned
/// tags. Pure — the caller loads the states and scans the tags, so this is testable without a
/// subject or a filesystem.
pub fn assemble(states: &[ItemState], tags: &[Tag]) -> Report {
    let known: BTreeSet<String> = states.iter().map(|s| canonical(&s.id)).collect();

    let rows = states
        .iter()
        .map(|state| {
            let want = canonical(&state.id);
            let matching = |kind: TraceKind| -> Vec<Location> {
                tags.iter()
                    .filter(|t| t.kind == kind && canonical(&t.req_id) == want)
                    .map(Location::of)
                    .collect()
            };
            Row {
                id: state.id.clone(),
                title: state.title.clone(),
                formalized: state.formalization == Formalization::Admitted,
                implements: matching(TraceKind::Implements),
                verifies: matching(TraceKind::Verifies),
                verdict: verdict_cell(state),
            }
        })
        .collect();

    let orphans = tags
        .iter()
        .filter(|t| !known.contains(&canonical(&t.req_id)))
        .map(|t| Orphan {
            req_id: t.req_id.clone(),
            kind: match t.kind {
                TraceKind::Implements => "implements".to_string(),
                TraceKind::Verifies => "verifies".to_string(),
            },
            file: t.file.display().to_string(),
            line: t.line,
        })
        .collect();

    Report { rows, orphans }
}

/// The requirement-id prefixes a subject declares — what [`crate::trace::scan`] filters tags by, so
/// a coincidental `category-1` in prose is never read as an id.
pub fn prefixes(states: &[ItemState]) -> BTreeSet<String> {
    states
        .iter()
        .map(|s| crate::trace::tags::id_prefix(&s.id))
        .collect()
}

/// Render the report as human-readable text.
pub fn render_text(report: &Report) -> String {
    let mut out = String::from("Traceability report\n");
    let mark = |b: bool| if b { "yes" } else { "—" };
    for row in &report.rows {
        let verdict = match &row.verdict {
            None => "never verified".to_string(),
            Some(v) => {
                let basis = v
                    .basis
                    .as_deref()
                    .map(|b| format!(" ({b})"))
                    .unwrap_or_default();
                let fresh = if v.fresh {
                    ""
                } else {
                    " [stale — re-verify]"
                };
                format!("{}{basis} [{}]{fresh}", v.outcome, v.correspondence)
            }
        };
        out.push_str(&format!(
            "\n{}{}\n  formalized: {}   implemented: {}   verified: {}\n  verdict: {verdict}\n",
            row.id,
            row.title
                .as_deref()
                .map(|t| format!(" — {t}"))
                .unwrap_or_default(),
            mark(row.formalized),
            mark(!row.implements.is_empty()),
            mark(!row.verifies.is_empty()),
        ));
        for loc in row.implements.iter().chain(row.verifies.iter()) {
            let sym = loc
                .symbol
                .as_deref()
                .map(|s| format!(" → {s}"))
                .unwrap_or_default();
            out.push_str(&format!("    {}:{}{sym}\n", loc.file, loc.line));
        }
    }
    if !report.orphans.is_empty() {
        out.push_str("\nOrphan tags (reference an unknown requirement):\n");
        for o in &report.orphans {
            out.push_str(&format!(
                "  {} ({}) at {}:{}\n",
                o.req_id, o.kind, o.file, o.line
            ));
        }
    }
    out
}

/// Load everything the report needs and assemble it. The one impure entry point: it reads the
/// companion state and scans the subject, but runs no engine and no test.
pub fn build(subject: &Path) -> anyhow::Result<Report> {
    let (companion, items) = crate::adopt::resolve(subject)?;
    let triage = crate::triage::load(&companion)?;
    let drafts = crate::draft::load(&companion)?;
    let verdicts = crate::verdict_store::load(&companion)?;
    let anchor = crate::verdict_store::DriftAnchor::current(
        crate::verify::subject_head_commit(subject),
        crate::proving_env::ProvingEnv::current(&companion),
        crate::verify::current_fingerprints(subject, &companion),
    );
    let states = crate::status::backlog(&items, &triage, &drafts, &verdicts, &anchor);
    let tags = crate::trace::scan(subject, &companion, &prefixes(&states));
    Ok(assemble(&states, &tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Formalization;

    fn state(
        id: &str,
        formalization: Formalization,
        verdict: Option<crate::verdict_store::VerdictView>,
    ) -> ItemState {
        ItemState {
            id: id.into(),
            title: Some(format!("title {id}")),
            text: "prose".into(),
            classification: None,
            classified_by: None,
            formalization,
            verdict,
        }
    }

    fn view(status: &str, correspondence: &str, fresh: bool) -> crate::verdict_store::VerdictView {
        crate::verdict_store::VerdictView {
            status: status.into(),
            basis: (status == "holds").then(|| "not-falsified".into()),
            reason: None,
            detail: vec![],
            witness: None,
            evidence: vec![],
            correspondence: correspondence.into(),
            fresh,
            stale_reasons: if fresh {
                vec![]
            } else {
                vec!["prose moved".into()]
            },
            environment: None,
        }
    }

    fn tag(kind: TraceKind, req_id: &str, symbol: &str) -> Tag {
        Tag {
            req_id: req_id.into(),
            kind,
            file: std::path::PathBuf::from("src/a.rs"),
            line: 10,
            symbol: Some(symbol.into()),
        }
    }

    // Verifies: REQ077 — a row joins the item's formalization + verdict with its code tags, and the
    // verdict's correspondence rides onto the row so an asserted verdict never reads as mechanical.
    #[test]
    fn a_row_joins_formalization_tags_and_the_asserted_verdict() {
        let states = vec![state(
            "REQ075",
            Formalization::None,
            Some(view("holds", "asserted", true)),
        )];
        let tags = vec![
            tag(TraceKind::Verifies, "REQ075", "the_test"),
            tag(TraceKind::Implements, "REQ-075", "the_impl"),
        ];
        let report = assemble(&states, &tags);
        let row = &report.rows[0];
        assert!(!row.formalized);
        assert_eq!(
            row.implements.len(),
            1,
            "REQ-075 canonically matches REQ075"
        );
        assert_eq!(row.verifies.len(), 1);
        let v = row.verdict.as_ref().unwrap();
        assert_eq!(v.outcome, "proven");
        assert_eq!(v.correspondence, "asserted");
        assert!(report.orphans.is_empty());
    }

    // Verifies: REQ077 — statuses map to the report's outcome vocabulary; a never-verified row is
    // distinct from a recorded not-determined.
    #[test]
    fn outcomes_map_and_never_verified_is_distinct() {
        let states = vec![
            state(
                "A",
                Formalization::Admitted,
                Some(view("fails", "mechanical", true)),
            ),
            state(
                "B",
                Formalization::Admitted,
                Some(view("unknown", "mechanical", true)),
            ),
            state("C", Formalization::None, None),
        ];
        let report = assemble(&states, &[]);
        assert_eq!(
            report.rows[0].verdict.as_ref().unwrap().outcome,
            "disproven"
        );
        assert_eq!(
            report.rows[1].verdict.as_ref().unwrap().outcome,
            "not-determined"
        );
        assert!(
            report.rows[2].verdict.is_none(),
            "never verified, not not-determined"
        );
        assert!(report.rows[0].formalized);
    }

    // Verifies: REQ077 — a tag naming an id no requirement declares is an orphan, not a row.
    #[test]
    fn a_tag_for_an_unknown_id_is_an_orphan() {
        let states = vec![state("REQ001", Formalization::None, None)];
        let tags = vec![tag(TraceKind::Verifies, "REQ999", "ghost")];
        let report = assemble(&states, &tags);
        assert!(report.rows[0].verifies.is_empty());
        assert_eq!(report.orphans.len(), 1);
        assert_eq!(report.orphans[0].req_id, "REQ999");
    }

    // Verifies: REQ077 — a stale verdict is marked so in the text render, and the asserted marker
    // shows on the row.
    #[test]
    fn text_render_marks_stale_and_asserted() {
        let states = vec![state(
            "REQ075",
            Formalization::None,
            Some(view("holds", "asserted", false)),
        )];
        let text = render_text(&assemble(&states, &[]));
        assert!(text.contains("proven"), "{text}");
        assert!(text.contains("[asserted]"), "{text}");
        assert!(text.contains("stale"), "{text}");
    }
}
