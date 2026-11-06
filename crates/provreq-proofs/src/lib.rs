//! Payload-free proof cores for provreq's own category-1 self-requirements.
//!
//! Creusot compiles the whole crate before running any harness and intercepts
//! `#[derive(PartialEq/Eq)]` to demand `DeepModel` on reachable types — a wall the app crate's ~96
//! derives turn into an open-ended arc (#531). So the decision a self-requirement proves lives here
//! as a payload-free core the app delegates to: `engine::EngineStatus::is_ready` calls [`is_ready`],
//! and `verify.crates` points REQ051 at this crate (#483), so Creusot only has to compile this.
//! `Readiness` deliberately derives nothing that triggers the `DeepModel` demand.

#[cfg(creusot)]
use creusot_std::macros::*;

/// The readiness-relevant discriminants of an engine, with the version/reason strings dropped so
/// Kani and Creusot can range over the type. `engine::EngineStatus` maps onto this (one variant
/// each) via its `readiness_class`, and its `is_ready` delegates to [`is_ready`] below — so a proof
/// over every `Readiness` is a proof about the readiness provreq actually reports.
#[derive(Clone, Copy, Debug)]
pub enum Readiness {
    NotWired,
    Available,
    Missing,
    Unusable,
    Incompatible,
}

/// Let Kani quantify over the five discriminants. Costs nothing off `cargo kani`.
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
#[cfg_attr(creusot, ensures(result == is_ready_logic(r)))]
pub fn is_ready(r: Readiness) -> bool {
    matches!(r, Readiness::Available)
}

/// REQ051, as a property checked against [`is_ready`]: `Available` is ready and every other state
/// is not, so a readiness answer can never overclaim what provreq can run. Written out per variant
/// independently of `is_ready`, so the two agree only if readiness is honest.
pub fn readiness_is_honest(r: &Readiness) -> bool {
    match r {
        Readiness::Available => is_ready(*r),
        Readiness::NotWired
        | Readiness::Missing
        | Readiness::Unusable
        | Readiness::Incompatible => !is_ready(*r),
    }
}

/// Creusot logic mirror of [`is_ready`] — pearlite specifications may only call `#[logic]` items.
#[cfg(creusot)]
#[logic(open)]
pub fn is_ready_logic(r: Readiness) -> bool {
    pearlite! { match r { Readiness::Available => true, _ => false } }
}

/// Creusot logic mirror of [`readiness_is_honest`]; `creusot::with_mirrors` redirects the proof
/// harness onto this (`fn readiness_is_honest_logic` is the name it looks for).
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
