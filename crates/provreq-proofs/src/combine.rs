//! REQ030 — the outcome fold. `verdict::aggregate` reduces every engine's evidence to one verdict
//! polarity and basis through this pure fold: a hold and a refutation together are
//! divergence-needs-review; any refutation refutes; otherwise a hold reports the **strongest** basis
//! any present evidence earned; no evidence at all is inconclusive. Never a majority vote.
//! `verdict::combine_outcome` delegates here, so the proof is about the rule provreq applies.
//!
//! Like REQ076's guard this folds over an **arbitrary-length** slice, so the proof is a
//! max-over-sequence induction, not a length-3 reimplementation (#539). The maximum is handled
//! without recursion: because the basis scale is a finite three-rung lattice, the strongest present
//! basis is simply the highest rung that appears — three existentials over the sequence
//! ([`strongest_prefix`]). The program fold carries `best == strongest_prefix(.., i)` as a loop
//! invariant, so Creusot discharges the whole-length equality by prefix extension. Kani still checks
//! the same core at the length-3 bound, since `ProofEnsemble` is `Arbitrary` as a 3-element `Vec`.

// Only the logic types and macros — not the whole `prelude`, whose glob re-exports a `Clone` derive
// macro that collides with std's on the `#[derive(Clone)]` proxies below (E0659).
#[cfg(creusot)]
use creusot_std::logic::{Int, Seq};
#[cfg(creusot)]
use creusot_std::macros::*;

/// A verdict polarity. Payload-free proxy for `verdict::Status`; no `PartialEq` derive (the derive
/// is what triggers Creusot's `DeepModel` demand — the proofs use `==`/`match`, which is pearlite's
/// logical equality and needs no derive).
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Holds,
    Fails,
    Unknown,
}

/// The rung of assurance a holding engine earned. Payload-free proxy for `verdict::Basis`, ordered
/// weakest-to-strongest.
#[derive(Debug, Clone, Copy)]
pub enum Basis {
    NotFalsified,
    ModelCheckedBounded,
    Proven,
}

/// The combined outcome of the fold. Payload-free but for the single strongest basis a `Held`
/// carries; `verdict::combine_outcome` maps it back onto `(Status, Option<UnknownReason>,
/// Option<Basis>)`.
#[derive(Debug, Clone, Copy)]
pub enum Outcome {
    Divergence,
    Refuted,
    Held(Option<Basis>),
    Inconclusive,
}

/// Rank of a basis: higher is stronger. Internal to the fold — the app keeps its own display rank.
/// The `#[ensures]` links it to [`basis_rank_logic`] so the fold's rank comparisons can be read in
/// the loop invariants, which are stated through the logic rank.
#[cfg_attr(creusot, ensures(result@ == basis_rank_logic(b)))]
pub fn basis_rank(b: Basis) -> usize {
    match b {
        Basis::NotFalsified => 1,
        Basis::ModelCheckedBounded => 2,
        Basis::Proven => 3,
    }
}

/// Rank of an optional basis as a plain integer; absent is below every present rung. The executable
/// twin that Kani runs; `rank_opt` is its logic mirror.
pub fn rank_opt_exec(o: Option<Basis>) -> usize {
    match o {
        Some(b) => basis_rank(b),
        None => 0,
    }
}

/// The outcome fold over the whole evidence slice. `verdict::combine_outcome` reduces each engine's
/// `Evidence` to its `(Status, Option<Basis>)` and calls this, so this is the real decision, not a
/// copy. The `#[ensures]` links the result to [`combine_outcome_logic`], stated over the whole `Seq`;
/// the loop invariants make `any_holds`/`any_fails`/`best` the running fold over the prefix, so
/// Creusot discharges the whole-length property by prefix extension — no array bound.
#[cfg_attr(creusot, ensures(result == combine_outcome_logic(outcomes@)))]
pub fn combine_outcome(outcomes: &[(Status, Option<Basis>)]) -> Outcome {
    let mut any_holds = false;
    let mut any_fails = false;
    let mut best: Option<Basis> = None;
    let mut i = 0;
    #[cfg_attr(creusot, invariant(0 <= i@ && i@ <= outcomes@.len()))]
    #[cfg_attr(creusot, invariant(any_holds
        == (exists<j: Int> 0 <= j && j < i@ && (outcomes@[j]).0 == Status::Holds)))]
    #[cfg_attr(creusot, invariant(any_fails
        == (exists<j: Int> 0 <= j && j < i@ && (outcomes@[j]).0 == Status::Fails)))]
    #[cfg_attr(creusot, invariant(best == strongest_prefix(outcomes@, i@)))]
    while i < outcomes.len() {
        let (s, b) = outcomes[i];
        match s {
            Status::Holds => any_holds = true,
            Status::Fails => any_fails = true,
            Status::Unknown => {}
        }
        best = match (best, b) {
            (cur, None) => cur,
            (None, Some(_)) => b,
            (Some(c), Some(bi)) => {
                if basis_rank(c) >= basis_rank(bi) {
                    Some(c)
                } else {
                    Some(bi)
                }
            }
        };
        i += 1;
    }
    if any_holds && any_fails {
        Outcome::Divergence
    } else if any_fails {
        Outcome::Refuted
    } else if any_holds {
        Outcome::Held(best)
    } else {
        Outcome::Inconclusive
    }
}

/// Logic mirror of [`basis_rank`].
#[cfg(creusot)]
#[logic(open)]
pub fn basis_rank_logic(b: Basis) -> Int {
    pearlite! {
        match b {
            Basis::NotFalsified => 1,
            Basis::ModelCheckedBounded => 2,
            Basis::Proven => 3,
        }
    }
}

/// Logic mirror of [`rank_opt_exec`]: the rank of an optional basis, absent ranking below every
/// present rung.
#[cfg(creusot)]
#[logic(open)]
pub fn rank_opt(o: Option<Basis>) -> Int {
    pearlite! {
        match o {
            Some(b) => basis_rank_logic(b),
            None => 0,
        }
    }
}

/// The strongest basis present in the first `n` entries of `s` — the highest rung that appears.
/// Because the basis scale is a finite three-rung lattice, the maximum is a closed form (the highest
/// present rung), so no recursion or sub-sequence lemma is needed: three existentials decide it. The
/// program fold's invariant `best == strongest_prefix(outcomes@, i@)` lets Creusot step this by one.
#[cfg(creusot)]
#[logic(open)]
pub fn strongest_prefix(s: Seq<(Status, Option<Basis>)>, n: Int) -> Option<Basis> {
    pearlite! {
        if exists<j: Int> 0 <= j && j < n && (s[j]).1 == Some(Basis::Proven) {
            Some(Basis::Proven)
        } else if exists<j: Int> 0 <= j && j < n && (s[j]).1 == Some(Basis::ModelCheckedBounded) {
            Some(Basis::ModelCheckedBounded)
        } else if exists<j: Int> 0 <= j && j < n && (s[j]).1 == Some(Basis::NotFalsified) {
            Some(Basis::NotFalsified)
        } else {
            None
        }
    }
}

/// Logic mirror of the fold: REQ030's rule over the whole `Seq`. The program fold is
/// `#[ensures]`-linked to this, so discharging that link is the genuine ∀-length proof; `open` lets
/// the solver unfold it against the loop invariants and the soundness mirror.
#[cfg(creusot)]
#[logic(open)]
pub fn combine_outcome_logic(s: Seq<(Status, Option<Basis>)>) -> Outcome {
    pearlite! {
        let any_holds = exists<j: Int> 0 <= j && j < s.len() && (s[j]).0 == Status::Holds;
        let any_fails = exists<j: Int> 0 <= j && j < s.len() && (s[j]).0 == Status::Fails;
        if any_holds && any_fails {
            Outcome::Divergence
        } else if any_fails {
            Outcome::Refuted
        } else if any_holds {
            Outcome::Held(strongest_prefix(s, s.len()))
        } else {
            Outcome::Inconclusive
        }
    }
}

/// REQ030's sort: a bounded ensemble of `(polarity, earned basis)` outcomes. A `Vec` so Creusot's
/// proof ranges over every length; Kani's `Arbitrary` builds it from a fixed 3-element array, keeping
/// the bounded check in play.
#[cfg(any(kani, creusot, prusti))]
pub struct ProofEnsemble(pub Vec<(Status, Option<Basis>)>);

#[cfg(kani)]
impl kani::Arbitrary for Status {
    fn any() -> Self {
        match kani::any::<u8>() % 3 {
            0 => Status::Holds,
            1 => Status::Fails,
            _ => Status::Unknown,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for Basis {
    fn any() -> Self {
        match kani::any::<u8>() % 3 {
            0 => Basis::NotFalsified,
            1 => Basis::ModelCheckedBounded,
            _ => Basis::Proven,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for ProofEnsemble {
    fn any() -> Self {
        let outcomes: [(Status, Option<Basis>); 3] = kani::any();
        ProofEnsemble(outcomes.to_vec())
    }
}

/// REQ030 stated as a property and checked against the real [`combine_outcome`]: divergence when a
/// hold and a refutation co-occur; refutation when something refutes and nothing holds; inconclusive
/// when nothing holds and nothing refutes; otherwise a hold whose basis is **present and dominates**
/// every present basis (the maximum, stated independently of the fold's own selection). Written with
/// integer ranks (`rank_opt_exec`) so basis equality is rank equality — the ranks are injective.
///
/// `#[trusted]` under Creusot: this is the executable twin Kani runs, built from iterator `any`/`all`
/// that Creusot has no contract for. The Creusot proof runs against the pearlite mirror
/// [`combine_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked fold.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn combine_is_sound(e: &ProofEnsemble) -> bool {
    let outcomes = &e.0;
    let any_holds = outcomes.iter().any(|(s, _)| matches!(s, Status::Holds));
    let any_fails = outcomes.iter().any(|(s, _)| matches!(s, Status::Fails));
    match combine_outcome(outcomes) {
        Outcome::Divergence => any_holds && any_fails,
        Outcome::Refuted => any_fails && !any_holds,
        Outcome::Inconclusive => !any_holds && !any_fails,
        Outcome::Held(b) => {
            let bd = rank_opt_exec(b);
            any_holds
                && !any_fails
                && outcomes.iter().all(|(_, x)| rank_opt_exec(*x) <= bd)
                && outcomes.iter().any(|(_, x)| rank_opt_exec(*x) == bd)
        }
    }
}

/// Logic mirror of [`combine_is_sound`]; `with_mirrors` redirects the harness onto this. It restates
/// REQ030 with pearlite quantifiers over `e.0@`, reading the fold's result through
/// [`combine_outcome_logic`] — tied to the real decision for every length by the program fold's
/// `#[ensures]`.
#[cfg(creusot)]
#[logic(open)]
pub fn combine_is_sound_logic(e: &ProofEnsemble) -> bool {
    pearlite! {
        let s = e.0@;
        let any_holds = exists<j: Int> 0 <= j && j < s.len() && (s[j]).0 == Status::Holds;
        let any_fails = exists<j: Int> 0 <= j && j < s.len() && (s[j]).0 == Status::Fails;
        match combine_outcome_logic(s) {
            Outcome::Divergence => any_holds && any_fails,
            Outcome::Refuted => any_fails && !any_holds,
            Outcome::Inconclusive => !any_holds && !any_fails,
            Outcome::Held(b) =>
                any_holds
                && !any_fails
                && (forall<k: Int> 0 <= k && k < s.len() ==> rank_opt((s[k]).1) <= rank_opt(b))
                && (exists<j: Int> 0 <= j && j < s.len() && rank_opt((s[j]).1) == rank_opt(b)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(b: Basis) -> (Status, Option<Basis>) {
        (Status::Holds, Some(b))
    }

    #[test]
    fn refutation_wins_over_holds_as_divergence() {
        // A hold and a refutation together → divergence.
        assert!(matches!(
            combine_outcome(&[h(Basis::Proven), (Status::Fails, None)]),
            Outcome::Divergence
        ));
    }

    #[test]
    fn a_lone_refutation_refutes() {
        assert!(matches!(
            combine_outcome(&[(Status::Fails, None)]),
            Outcome::Refuted
        ));
    }

    #[test]
    fn holds_report_the_strongest_present_basis() {
        // NotFalsified and Proven both present, no refutation → Held(Proven).
        assert!(matches!(
            combine_outcome(&[h(Basis::NotFalsified), h(Basis::Proven)]),
            Outcome::Held(Some(Basis::Proven))
        ));
    }

    #[test]
    fn no_evidence_is_inconclusive() {
        assert!(matches!(combine_outcome(&[]), Outcome::Inconclusive));
        // An unknown with no basis contributes nothing.
        assert!(matches!(
            combine_outcome(&[(Status::Unknown, None)]),
            Outcome::Inconclusive
        ));
    }
}
