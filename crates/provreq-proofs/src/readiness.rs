//! REQ051 — readiness never overclaims. `engine::EngineStatus::is_ready` delegates to [`is_ready`].

#[cfg(creusot)]
use creusot_std::macros::*;

/// The readiness-relevant discriminants of an engine, with the version/reason strings dropped so
/// Kani and Creusot can range over the type. `engine::EngineStatus` maps onto this (one variant
/// each) via its `readiness_class`, and its `is_ready` delegates to [`is_ready`] below — so a proof
/// over every `Readiness` is a proof about the readiness provreq actually reports.
// Plain items — gated out under `verus_only`, where the `verus! { }` mirror below owns these names
// (a Verus `spec fn` matches only a Verus-visible datatype and calls only another `spec fn`, #596).
// Unlike #592/#594 the predicate and sort here are plain (they compile on an ordinary build), so all
// three gate out — the `code_drift.rs` style.
#[cfg(not(verus_only))]
#[derive(Clone, Copy, Debug)]
pub enum Readiness {
    NotWired,
    Available,
    Missing,
    Unusable,
    Incompatible,
}

#[cfg(kani)]
impl kani::Arbitrary for Readiness {
    fn any() -> Self {
        match kani::any::<u8>() % 5 {
            0 => Readiness::NotWired,
            1 => Readiness::Available,
            2 => Readiness::Missing,
            3 => Readiness::Unusable,
            _ => Readiness::Incompatible,
        }
    }
}

/// Whether an engine in this state can back a verdict: only `Available`. `EngineStatus::is_ready`
/// delegates here, so the property proved by [`readiness_is_honest`] is the one provreq enforces.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == is_ready_logic(r)))]
pub fn is_ready(r: Readiness) -> bool {
    matches!(r, Readiness::Available)
}

/// REQ051, as a property checked against [`is_ready`]: `Available` is ready and every other state
/// is not, so a readiness answer can never overclaim what provreq can run.
#[cfg(not(verus_only))]
pub fn readiness_is_honest(r: &Readiness) -> bool {
    match r {
        Readiness::Available => is_ready(*r),
        Readiness::NotWired
        | Readiness::Missing
        | Readiness::Unusable
        | Readiness::Incompatible => !is_ready(*r),
    }
}

#[cfg(creusot)]
#[logic(open)]
pub fn is_ready_logic(r: Readiness) -> bool {
    pearlite! { match r { Readiness::Available => true, _ => false } }
}

#[cfg(creusot)]
#[logic(open)]
pub fn readiness_is_honest_logic(r: &Readiness) -> bool {
    pearlite! {
        match *r {
            Readiness::Available => is_ready_logic(*r),
            _ => !is_ready_logic(*r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_available_is_ready_and_honesty_holds_for_every_variant() {
        let all = [
            Readiness::NotWired,
            Readiness::Available,
            Readiness::Missing,
            Readiness::Unusable,
            Readiness::Incompatible,
        ];
        for r in all {
            assert_eq!(is_ready(r), matches!(r, Readiness::Available));
            assert!(readiness_is_honest(&r));
        }
    }
}

// REQ093/#596 — the Verus mirror: independent deductive corroboration of REQ051 alongside Creusot.
// Under `verus_only` (the cfg `cargo verus` sets during verification, and no ordinary build sets)
// the payload-free `Readiness`, `is_ready`, and the honesty predicate are authored in the Verus
// subset as `spec fn`s, so the engine's generated `assert(forall|r| readiness_is_honest(&r))`
// harness discharges through Verus/Z3 exactly as the pearlite mirror above discharges through
// Creusot/Why3. Both top out at `proven`; two unrelated provers agreeing is the point.
//
// Unlike REQ030/REQ076 (#593/#595) this core is loop-free: `is_ready` is a single `==` and
// `readiness_is_honest` a five-variant case analysis, the easy REQ071 shape. The harness body is a
// function call (`readiness_is_honest(&r)`), which triggers on itself — no `#[trigger]` needed.
//
// The three plain items above are gated `not(verus_only)` so these own the names the harness reaches
// (`crate::readiness::{Readiness, readiness_is_honest}`). Here the predicate and sort are plain (they
// compile on an ordinary build), so all three gate out — the `code_drift.rs` style, not the
// `cfg(any(kani, creusot, prusti))` split REQ030/REQ076 used.
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub enum Readiness { NotWired, Available, Missing, Unusable, Incompatible }

pub open spec fn is_ready(r: Readiness) -> bool {
    r == Readiness::Available
}

pub open spec fn readiness_is_honest(r: &Readiness) -> bool {
    match *r {
        Readiness::Available => is_ready(*r),
        _ => !is_ready(*r),
    }
}

} // verus!
