//! Step 6 — the living loop: verdicts as durable state that drifts.
//!
//! A verdict is produced on demand (Step 4) but does not stay true forever: the requirement prose
//! can move, the subject code can move, or the tool can change underneath it. The D9 provenance
//! every verdict carries (`requirement_revision` + `subject_commit` + `tool_version`) is exactly
//! the anchor to detect that — this module persists the verdict keyed by item id and compares its
//! provenance against the current world to decide whether it is still fresh.
//!
//! Persisted as a companion `verdicts.yml`, mirroring `drafts.yml`/`triage.yml`. The stored shape
//! IS the wire shape ([`crate::verdict::VerdictReport`]) — the web surface and the store never
//! diverge on what a verdict looks like. Re-verifying overwrites the stored verdict; a stale one is
//! never silently discarded, only flagged, so the operator decides when to re-run.
//!
//! Implements: REQ039 (persist verdicts; detect drift against provenance; surface freshness)

use crate::verdict::VerdictReport;
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

/// The companion file the verdict store persists to.
pub const VERDICT_FILE: &str = "verdicts.yml";

/// Every item's last verdict, keyed by item id. Additive over time — a re-verify replaces one
/// entry, and an item never verified simply has none.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct VerdictStore {
    #[serde(default)]
    pub verdicts: BTreeMap<String, VerdictReport>,
}

impl VerdictStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Load the verdict store from the companion root. A missing file is an empty store — nothing has
/// been verified yet, which is honest, not an error.
pub fn load(companion_root: &Path) -> Result<VerdictStore> {
    let path = companion_root.join(VERDICT_FILE);
    if !path.exists() {
        return Ok(VerdictStore::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Write the verdict store to the companion root.
pub fn save(companion_root: &Path, store: &VerdictStore) -> Result<()> {
    let path = companion_root.join(VERDICT_FILE);
    let yaml = serde_yaml::to_string(store).context("serializing verdict store")?;
    std::fs::write(&path, yaml).with_context(|| format!("writing {}", path.display()))
}

/// Record a verdict, returning a new store with it stored under its own id (immutable insert). A
/// later verdict for the same item replaces the earlier one — the store holds the *latest* answer.
pub fn record(store: &VerdictStore, verdict: VerdictReport) -> VerdictStore {
    let mut verdicts = store.verdicts.clone();
    verdicts.insert(verdict.id.clone(), verdict);
    VerdictStore { verdicts }
}

/// What a verdict was produced against, distilled to what drift needs: the subject commit and the
/// tool version (the requirement revision is per-item, compared separately against each item).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftAnchor {
    pub subject_commit: Option<String>,
    pub tool_version: String,
}

impl DriftAnchor {
    /// The anchor for the current world: this build's version, plus the caller-supplied subject
    /// HEAD (best-effort — `None` when the subject is not a git repo, never fabricated).
    pub fn current(subject_commit: Option<String>) -> Self {
        Self {
            subject_commit,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// A stored verdict paired with whether it still holds against the current world — the living-loop
/// surface. Carries the verdict's own labels plus the freshness verdict and, when stale, the
/// concrete reasons the operator must re-verify.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerdictView {
    pub status: String,
    pub basis: Option<String>,
    pub reason: Option<String>,
    /// The verdict is still anchored to the current world (nothing it depended on moved).
    pub fresh: bool,
    /// When not fresh, the concrete drifts — prose moved, code moved, tool changed — so the
    /// operator sees *why* a re-verify is owed, never just that one is.
    pub stale_reasons: Vec<String>,
}

/// Pair a stored verdict with its freshness against the current world. Pure over the stored
/// verdict, the item's current revision, and the [`DriftAnchor`] — no filesystem, so it is testable
/// without a subject. A verdict is stale when any provenance axis it was produced against has moved.
pub fn view(stored: &VerdictReport, current_revision: &str, anchor: &DriftAnchor) -> VerdictView {
    let mut stale_reasons = Vec::new();

    if stored.provenance.requirement_revision != current_revision {
        stale_reasons.push(
            "the requirement prose moved since this verdict — re-verify against the current text"
                .to_string(),
        );
    }

    match (&stored.provenance.subject_commit, &anchor.subject_commit) {
        (Some(was), Some(now)) if was != now => stale_reasons.push(format!(
            "the subject code moved since this verdict (commit {was} → {now}) — re-verify",
        )),
        (Some(_), None) => stale_reasons.push(
            "the subject's commit can no longer be read to confirm this verdict — re-verify"
                .to_string(),
        ),
        (None, Some(_)) => stale_reasons.push(
            "the subject is now a git repo; this verdict predates its history — re-verify"
                .to_string(),
        ),
        _ => {}
    }

    if stored.provenance.tool_version != anchor.tool_version {
        stale_reasons.push(format!(
            "the tool changed since this verdict (provreq {} → {}) — re-verify",
            stored.provenance.tool_version, anchor.tool_version
        ));
    }

    VerdictView {
        status: stored.status.clone(),
        basis: stored.basis.clone(),
        reason: stored.reason.clone(),
        fresh: stale_reasons.is_empty(),
        stale_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::{ProvenanceReport, VerdictReport};

    fn stored(revision: &str, commit: Option<&str>, tool: &str) -> VerdictReport {
        VerdictReport {
            id: "REQ001".into(),
            status: "holds".into(),
            basis: Some("proven".into()),
            reason: None,
            witness: None,
            detail: vec![],
            evidence: vec![],
            provenance: ProvenanceReport {
                requirement_revision: revision.into(),
                subject_commit: commit.map(str::to_string),
                tool_version: tool.into(),
            },
        }
    }

    // Verifies: REQ039 — a verdict produced against the current world is fresh, with no reasons.
    #[test]
    fn unmoved_verdict_is_fresh() {
        let v = stored("r1", Some("abc"), "0.0.1");
        let anchor = DriftAnchor {
            subject_commit: Some("abc".into()),
            tool_version: "0.0.1".into(),
        };
        let view = view(&v, "r1", &anchor);
        assert!(view.fresh);
        assert!(view.stale_reasons.is_empty());
    }

    // Verifies: REQ039 — each provenance axis that moves is an independent, named staleness reason;
    // several can drift at once.
    #[test]
    fn each_moved_axis_is_a_named_reason() {
        let v = stored("r1", Some("abc"), "0.0.1");
        let anchor = DriftAnchor {
            subject_commit: Some("def".into()),
            tool_version: "0.0.2".into(),
        };
        let view = view(&v, "r2", &anchor);
        assert!(!view.fresh);
        assert_eq!(view.stale_reasons.len(), 3, "prose + code + tool all moved");
        assert!(view.stale_reasons.iter().any(|r| r.contains("prose moved")));
        assert!(view.stale_reasons.iter().any(|r| r.contains("abc → def")));
        assert!(view
            .stale_reasons
            .iter()
            .any(|r| r.contains("0.0.1 → 0.0.2")));
    }

    // Verifies: REQ039 — recording a verdict then loading round-trips it, and a re-verify replaces
    // rather than duplicates the item's entry.
    #[test]
    fn record_and_load_round_trip_replacing_prior() {
        let dir = tempfile::tempdir().unwrap();
        let first = record(&VerdictStore::new(), stored("r1", Some("abc"), "0.0.1"));
        save(dir.path(), &first).unwrap();

        let mut second_report = stored("r2", Some("def"), "0.0.1");
        second_report.status = "fails".into();
        let second = record(&load(dir.path()).unwrap(), second_report);
        save(dir.path(), &second).unwrap();

        let loaded = load(dir.path()).unwrap();
        assert_eq!(
            loaded.verdicts.len(),
            1,
            "re-verify replaces, never duplicates"
        );
        assert_eq!(loaded.verdicts["REQ001"].status, "fails");
        assert_eq!(
            loaded.verdicts["REQ001"].provenance.requirement_revision,
            "r2"
        );
    }

    // Verifies: REQ039 — a missing store file is an empty store (nothing verified yet), not an error.
    #[test]
    fn missing_file_is_empty_store() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().verdicts.is_empty());
    }
}
