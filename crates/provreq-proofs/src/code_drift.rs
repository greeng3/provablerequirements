//! REQ071 — the code-drift freshness axis. A verdict goes stale when the code it was produced
//! against moves, but `verdicts.yml` is itself committed, so a bare commit comparison would make an
//! in-tree record never fresh. The rule: when both sides carry a source fingerprint, drift is decided
//! by the fingerprint alone — the commit is *not* read; only when a fingerprint is absent does it
//! fall back to the commit comparison, so freshness is never widened on a basis the stored verdict
//! does not carry. `verdict_store::view` delegates the decision here.
//!
//! The decision is loop-free, so the Creusot proof is a pure finite case analysis. The inputs are
//! reduced to a payload-free [`Pair`] (presence + equality, the only facts the decision reads), so
//! the proved core carries no strings, no generics, and no `==`; `view` does the reduction at the
//! `&str` boundary. The `#[logic]` mirror restates the rule and the program fold's `#[ensures]` ties
//! the two; Kani checks the same core exhaustively over the twenty-five `Pair`-pair cases.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxies below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// A stored/current pair reduced to the two facts the decision reads: are both present, and (if so)
/// are they equal. Payload-free proxy — no `PartialEq` derive (that derive triggers Creusot's
/// `DeepModel` demand; the proofs use pearlite `==`/`match`, which needs no derive).
#[derive(Debug, Clone, Copy)]
pub enum Pair {
    /// Both sides present and equal.
    BothEqual,
    /// Both sides present and different.
    BothDiffer,
    /// Only the stored side is present.
    OnlyStored,
    /// Only the current side is present.
    OnlyCurrent,
    /// Neither side is present.
    Neither,
}

/// The code-drift axis outcome (REQ071). `verdict_store::view` renders each variant's stale message.
#[derive(Debug, Clone, Copy)]
pub enum CodeDrift {
    /// No code drift on this axis.
    Fresh,
    /// Both source fingerprints present and different — the fingerprinted source moved.
    SourceMoved,
    /// No fingerprint to compare, and the subject commit moved.
    CommitMoved,
    /// No fingerprint, and the stored commit can no longer be read.
    CommitUnreadable,
    /// No fingerprint, and the subject is a git repo now but was not when the verdict was stored.
    NowGitRepo,
}

/// The code-drift decision (REQ071): when the fingerprints are both present (`BothEqual`/`BothDiffer`)
/// the outcome is decided by them alone — the `commit` argument is not read — so landing the verdict
/// record (which moves the commit but not the fingerprinted source) is not drift. Otherwise the
/// commit-only fallback applies. The `#[ensures]` links it to [`code_drift_logic`] so the whole-case
/// equality is discharged deductively.
#[cfg_attr(creusot, ensures(result == code_drift_logic(fp, commit)))]
pub fn code_drift(fp: Pair, commit: Pair) -> CodeDrift {
    match fp {
        Pair::BothEqual => CodeDrift::Fresh,
        Pair::BothDiffer => CodeDrift::SourceMoved,
        _ => match commit {
            Pair::BothDiffer => CodeDrift::CommitMoved,
            Pair::OnlyStored => CodeDrift::CommitUnreadable,
            Pair::OnlyCurrent => CodeDrift::NowGitRepo,
            _ => CodeDrift::Fresh,
        },
    }
}

/// Logic mirror of [`code_drift`]; `with_mirrors` redirects the harness onto it and the program
/// decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn code_drift_logic(fp: Pair, commit: Pair) -> CodeDrift {
    pearlite! {
        match fp {
            Pair::BothEqual => CodeDrift::Fresh,
            Pair::BothDiffer => CodeDrift::SourceMoved,
            _ => match commit {
                Pair::BothDiffer => CodeDrift::CommitMoved,
                Pair::OnlyStored => CodeDrift::CommitUnreadable,
                Pair::OnlyCurrent => CodeDrift::NowGitRepo,
                _ => CodeDrift::Fresh,
            },
        }
    }
}

/// REQ071's sort: the fingerprint pair and the commit pair the decision reads. The category-1 harness
/// ranges over every combination.
#[cfg(any(kani, creusot, prusti))]
pub struct DriftInputs {
    pub fp: Pair,
    pub commit: Pair,
}

#[cfg(kani)]
impl kani::Arbitrary for Pair {
    fn any() -> Self {
        match kani::any::<u8>() % 5 {
            0 => Pair::BothEqual,
            1 => Pair::BothDiffer,
            2 => Pair::OnlyStored,
            3 => Pair::OnlyCurrent,
            _ => Pair::Neither,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for DriftInputs {
    fn any() -> Self {
        DriftInputs {
            fp: kani::any(),
            commit: kani::any(),
        }
    }
}

/// REQ071 stated as a property and checked against the real [`code_drift`]: when both fingerprints
/// are present the outcome is decided by the fingerprint alone (`SourceMoved` iff they differ, else
/// `Fresh`) — written without reading the commit, so proving it for every input establishes that the
/// commit cannot change the outcome there (the axis never widens). Otherwise the commit-only fallback.
///
/// `#[trusted]` under Creusot: this is the executable twin Kani runs, built from `matches!`. The
/// Creusot proof runs against the pearlite mirror [`code_drift_is_sound_logic`] (via `with_mirrors`),
/// whose content is the `#[ensures]`-linked decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn code_drift_is_sound(i: &DriftInputs) -> bool {
    let got = code_drift(i.fp, i.commit);
    match i.fp {
        Pair::BothEqual => matches!(got, CodeDrift::Fresh),
        Pair::BothDiffer => matches!(got, CodeDrift::SourceMoved),
        _ => match i.commit {
            Pair::BothDiffer => matches!(got, CodeDrift::CommitMoved),
            Pair::OnlyStored => matches!(got, CodeDrift::CommitUnreadable),
            Pair::OnlyCurrent => matches!(got, CodeDrift::NowGitRepo),
            _ => matches!(got, CodeDrift::Fresh),
        },
    }
}

/// Logic mirror of [`code_drift_is_sound`]; `with_mirrors` redirects the harness onto this. It
/// restates REQ071 reading the decision through [`code_drift_logic`], tied to the real code by the
/// program decision's `#[ensures]`.
#[cfg(creusot)]
#[logic(open)]
pub fn code_drift_is_sound_logic(i: &DriftInputs) -> bool {
    pearlite! {
        let got = code_drift_logic(i.fp, i.commit);
        match i.fp {
            Pair::BothEqual => got == CodeDrift::Fresh,
            Pair::BothDiffer => got == CodeDrift::SourceMoved,
            _ => match i.commit {
                Pair::BothDiffer => got == CodeDrift::CommitMoved,
                Pair::OnlyStored => got == CodeDrift::CommitUnreadable,
                Pair::OnlyCurrent => got == CodeDrift::NowGitRepo,
                _ => got == CodeDrift::Fresh,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_fingerprints_present_ignore_the_commit() {
        // Equal fingerprints → fresh, whatever the commit did.
        assert!(matches!(
            code_drift(Pair::BothEqual, Pair::BothDiffer),
            CodeDrift::Fresh
        ));
        // Differing fingerprints → source moved, whatever the commit did.
        assert!(matches!(
            code_drift(Pair::BothDiffer, Pair::BothEqual),
            CodeDrift::SourceMoved
        ));
    }

    #[test]
    fn no_fingerprint_falls_back_to_the_commit() {
        assert!(matches!(
            code_drift(Pair::Neither, Pair::BothDiffer),
            CodeDrift::CommitMoved
        ));
        assert!(matches!(
            code_drift(Pair::Neither, Pair::OnlyStored),
            CodeDrift::CommitUnreadable
        ));
        assert!(matches!(
            code_drift(Pair::Neither, Pair::OnlyCurrent),
            CodeDrift::NowGitRepo
        ));
        assert!(matches!(
            code_drift(Pair::Neither, Pair::Neither),
            CodeDrift::Fresh
        ));
    }
}
