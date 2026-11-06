//! REQ051 — readiness never overclaims. `engine::EngineStatus::is_ready` delegates to [`is_ready`].

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
/// is not, so a readiness answer can never overclaim what provreq can run.
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
