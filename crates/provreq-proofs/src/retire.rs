//! REQ079 — the traceability report recommends retiring a redundant tagged test. A requirement's
//! stored verdict may hold on both a mechanical basis (a prover/checker) and an asserted one (a
//! tagged test); the report recommends retiring the asserted test only when a holding mechanical
//! result of **no-weaker** basis already covers it. An asserted test stronger than every mechanical
//! result — or the sole evidence — is never recommended: it is not redundant.
//! `report::recommend_retirements` delegates the redundancy decision here, so the proof is about the
//! rule provreq applies.
//!
//! The decision reads only the two bases' ranks on the three-rung lattice (Proven >
//! ModelCheckedBounded > NotFalsified), so the proved core is payload-free and loop-free: Creusot
//! discharges it as a finite case analysis and Kani checks it over the nine (mechanical, asserted)
//! pairs.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxy below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// The rung of assurance a holding engine earned, weakest-to-strongest. Payload-free proxy for
/// `verdict::Basis`; no `PartialEq` derive (the proofs match on it).
// Plain proxy — gated out under `verus_only`, where the `verus! { }` mirror owns the name (#602).
#[cfg(not(verus_only))]
#[derive(Debug, Clone, Copy)]
pub enum Basis {
    NotFalsified,
    ModelCheckedBounded,
    Proven,
}

/// Rank of a basis on the lattice: higher is stronger. Mirrors `verdict::basis_rank`'s ordering (the
/// values differ, only the order is load-bearing). The `#[ensures]` links it to [`basis_rank_logic`]
/// so the decision's comparison can be read in the proof.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result@ == basis_rank_logic(b)))]
pub fn basis_rank(b: Basis) -> usize {
    match b {
        Basis::NotFalsified => 0,
        Basis::ModelCheckedBounded => 1,
        Basis::Proven => 2,
    }
}

/// Whether an asserted test is redundant and so recommended for retirement: exactly when a holding
/// mechanical result of no-weaker basis covers it (`rank(asserted) <= rank(mechanical)`).
/// `report::recommend_retirements` delegates here, so this is the real decision. The `#[ensures]`
/// links it to [`should_retire_logic`].
// Plain decision — the app delegates to it; gated out under `verus_only`.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == should_retire_logic(mechanical, asserted)))]
pub fn should_retire(mechanical: Basis, asserted: Basis) -> bool {
    basis_rank(asserted) <= basis_rank(mechanical)
}

/// REQ079's sort: the holding mechanical basis and the holding asserted basis whose redundancy is
/// being decided.
#[cfg(any(kani, creusot, prusti))]
pub struct RetirePair {
    pub mechanical: Basis,
    pub asserted: Basis,
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
impl kani::Arbitrary for RetirePair {
    fn any() -> Self {
        RetirePair {
            mechanical: kani::any(),
            asserted: kani::any(),
        }
    }
}

/// REQ079 stated as the rule the requirement reads and checked against the real [`should_retire`]: a
/// no-weaker asserted test is recommended for retirement; a strictly-stronger one is never
/// recommended (the safety property — a tagged test that out-proves the harness is kept). Written as
/// the rank comparison, not off `should_retire`'s own body, so the two agree only if the decision is
/// right.
///
/// `#[trusted]` under Creusot: the Creusot proof runs against the pearlite mirror
/// [`should_retire_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked
/// decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn should_retire_is_sound(p: &RetirePair) -> bool {
    let got = should_retire(p.mechanical, p.asserted);
    if basis_rank(p.asserted) <= basis_rank(p.mechanical) {
        got
    } else {
        !got
    }
}

/// Logic mirror of [`basis_rank`].
#[cfg(creusot)]
#[logic(open)]
pub fn basis_rank_logic(b: Basis) -> creusot_std::logic::Int {
    pearlite! {
        match b {
            Basis::NotFalsified => 0,
            Basis::ModelCheckedBounded => 1,
            Basis::Proven => 2,
        }
    }
}

/// Logic mirror of [`should_retire`]; the program decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn should_retire_logic(mechanical: Basis, asserted: Basis) -> bool {
    pearlite! { basis_rank_logic(asserted) <= basis_rank_logic(mechanical) }
}

/// Logic mirror of [`should_retire_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn should_retire_is_sound_logic(p: &RetirePair) -> bool {
    pearlite! {
        let got = should_retire_logic(p.mechanical, p.asserted);
        if basis_rank_logic(p.asserted) <= basis_rank_logic(p.mechanical) {
            got
        } else {
            !got
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_no_weaker_asserted_test_is_recommended() {
        // mechanical proven, asserted not-falsified → redundant.
        assert!(should_retire(Basis::Proven, Basis::NotFalsified));
        // equal basis is no-weaker → still redundant.
        assert!(should_retire(Basis::Proven, Basis::Proven));
    }

    #[test]
    fn a_stronger_asserted_test_is_kept() {
        // mechanical not-falsified, asserted proven → NOT redundant, keep it.
        assert!(!should_retire(Basis::NotFalsified, Basis::Proven));
        assert!(!should_retire(Basis::ModelCheckedBounded, Basis::Proven));
    }
}

// REQ093/#602 — the Verus mirror: independent deductive corroboration of REQ079 alongside Creusot.
// Loop-free finite case analysis over the 3-rung lattice; the generated harness body
// `should_retire_is_sound(&p)` is a function call that triggers on itself — no `#[trigger]`. The
// plain `Basis`/`basis_rank`/`should_retire` are gated `not(verus_only)` so these own the names under
// Verus; `RetirePair`/`should_retire_is_sound` are `cfg(any(kani, creusot, prusti))`, excluded under
// Verus, so the `verus!` versions are the only ones it sees.
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub enum Basis { NotFalsified, ModelCheckedBounded, Proven }

pub open spec fn basis_rank(b: Basis) -> int {
    match b {
        Basis::NotFalsified => 0,
        Basis::ModelCheckedBounded => 1,
        Basis::Proven => 2,
    }
}

pub open spec fn should_retire(mechanical: Basis, asserted: Basis) -> bool {
    basis_rank(asserted) <= basis_rank(mechanical)
}

pub struct RetirePair { pub mechanical: Basis, pub asserted: Basis }

pub open spec fn should_retire_is_sound(p: &RetirePair) -> bool {
    let got = should_retire(p.mechanical, p.asserted);
    if basis_rank(p.asserted) <= basis_rank(p.mechanical) {
        got
    } else {
        !got
    }
}

} // verus!
