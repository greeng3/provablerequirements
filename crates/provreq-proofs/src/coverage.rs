//! REQ043 — the coverage funnel's verdict axis. A requirement with a stored verdict counts toward
//! exactly one coverage bucket: a **drifted** verdict is `stale` regardless of its polarity (a
//! holds-that-moved and a fails-that-moved are both stale); a **fresh holding** verdict is `verified`;
//! a fresh verdict that does not hold counts toward neither. `status::coverage` delegates the
//! per-verdict bucketing here, so the proof is about the rule provreq applies.
//!
//! The decision reads only two facts about a present verdict — whether it is fresh, and whether its
//! status is `holds` — both payload-free, so the proved core is loop-free: Creusot discharges it as a
//! finite case analysis and Kani checks it over the four boolean combinations. Absence (a requirement
//! that was never verified) is handled by the caller's `verdict_view` guard, so this decides only the
//! present case.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxy below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// Which coverage axis a present verdict counts toward (REQ043). `Neither` is a fresh verdict that
/// does not hold — counted by no axis. Payload-free; `status::coverage` increments the matching total.
// Plain proxy — gated out under `verus_only`, where the `verus! { }` mirror owns the name (#606).
#[cfg(not(verus_only))]
#[derive(Debug, Clone, Copy)]
pub enum CoverageContribution {
    Neither,
    Stale,
    Verified,
}

/// The verdict-axis decision: a drifted verdict is stale (whatever its polarity), a fresh holding
/// verdict is verified, and a fresh non-holding verdict is neither. `status::coverage` delegates here
/// with `(view.fresh, view.status == "holds")`, so this is the real decision. The `#[ensures]` links
/// it to [`classify_logic`].
// Plain decision — the app delegates to it; gated out under `verus_only`.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == classify_logic(fresh, is_holds)))]
pub fn classify(fresh: bool, is_holds: bool) -> CoverageContribution {
    if !fresh {
        CoverageContribution::Stale
    } else if is_holds {
        CoverageContribution::Verified
    } else {
        CoverageContribution::Neither
    }
}

/// REQ043's sort: the two facts the verdict-axis decision reads about a present verdict.
#[cfg(any(kani, creusot, prusti))]
pub struct VerdictAxis {
    pub fresh: bool,
    pub is_holds: bool,
}

#[cfg(kani)]
impl kani::Arbitrary for VerdictAxis {
    fn any() -> Self {
        VerdictAxis {
            fresh: kani::any(),
            is_holds: kani::any(),
        }
    }
}

/// REQ043 stated as the rule the requirement reads and checked against the real [`classify`]: a
/// drifted verdict is stale independent of its polarity (the `!fresh` arm never reads `is_holds`); a
/// fresh holding verdict is verified; a fresh non-holding verdict is neither. Written as the case
/// table, not off `classify`'s own body, so the two agree only if the decision is right.
///
/// `#[trusted]` under Creusot: the Creusot proof runs against the pearlite mirror
/// [`classify_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn classify_is_sound(a: &VerdictAxis) -> bool {
    let got = classify(a.fresh, a.is_holds);
    if !a.fresh {
        matches!(got, CoverageContribution::Stale)
    } else if a.is_holds {
        matches!(got, CoverageContribution::Verified)
    } else {
        matches!(got, CoverageContribution::Neither)
    }
}

/// Logic mirror of [`classify`]; the program decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn classify_logic(fresh: bool, is_holds: bool) -> CoverageContribution {
    pearlite! {
        if !fresh {
            CoverageContribution::Stale
        } else if is_holds {
            CoverageContribution::Verified
        } else {
            CoverageContribution::Neither
        }
    }
}

/// Logic mirror of [`classify_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn classify_is_sound_logic(a: &VerdictAxis) -> bool {
    pearlite! {
        let got = classify_logic(a.fresh, a.is_holds);
        if !a.fresh {
            got == CoverageContribution::Stale
        } else if a.is_holds {
            got == CoverageContribution::Verified
        } else {
            got == CoverageContribution::Neither
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drifted_verdict_is_stale_whatever_its_polarity() {
        assert!(matches!(classify(false, true), CoverageContribution::Stale));
        assert!(matches!(
            classify(false, false),
            CoverageContribution::Stale
        ));
    }

    #[test]
    fn a_fresh_verdict_is_verified_only_when_it_holds() {
        assert!(matches!(
            classify(true, true),
            CoverageContribution::Verified
        ));
        assert!(matches!(
            classify(true, false),
            CoverageContribution::Neither
        ));
    }
}

// REQ093/#606 — the Verus mirror: independent deductive corroboration of REQ043 alongside Creusot.
// Loop-free finite case analysis over two bools; the generated harness body `classify_is_sound(&a)`
// is a function call that triggers on itself — no `#[trigger]`. The plain `CoverageContribution`/
// `classify` are gated `not(verus_only)` so these own the names under Verus; `VerdictAxis`/
// `classify_is_sound` are `cfg(any(kani, creusot, prusti))`, excluded under Verus, so the `verus!`
// versions are the only ones it sees.
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub enum CoverageContribution { Neither, Stale, Verified }

pub open spec fn classify(fresh: bool, is_holds: bool) -> CoverageContribution {
    if !fresh {
        CoverageContribution::Stale
    } else if is_holds {
        CoverageContribution::Verified
    } else {
        CoverageContribution::Neither
    }
}

pub struct VerdictAxis { pub fresh: bool, pub is_holds: bool }

pub open spec fn classify_is_sound(a: &VerdictAxis) -> bool {
    let got = classify(a.fresh, a.is_holds);
    if !a.fresh {
        got == CoverageContribution::Stale
    } else if a.is_holds {
        got == CoverageContribution::Verified
    } else {
        got == CoverageContribution::Neither
    }
}

} // verus!
