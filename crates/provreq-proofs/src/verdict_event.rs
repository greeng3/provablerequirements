//! REQ078 — the verdict review-log is an event stream. A `provreq-verdict` breadcrumb is appended
//! only on the **first** verdict a requirement earns, or on any later verdict that differs from the
//! one on record in its status, its basis, OR its correspondence; an identical re-verify records
//! nothing. `verify::is_verdict_event` delegates the decision here, so the proof is about the rule
//! provreq applies.
//!
//! The decision reads only two facts about the `(status, basis, correspondence)` string tuples it is
//! handed: whether a prior verdict exists, and whether each of the three fields equals the new one.
//! So — like REQ071's `code_drift` — the inputs are reduced to a payload-free [`VerdictEventInputs`]
//! (presence + three equalities, the only facts the decision reads) at the `&str` boundary in
//! `verify.rs`; the proved core carries no strings and no `==` on verdicts. The decision is
//! loop-free, so the Creusot proof is a finite case analysis; Kani checks the same core over the
//! sixteen boolean combinations.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxy below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// Whether a verdict is a breadcrumb event: an event unless there is a prior verdict equal to the
/// new one in all three fields. `verify::is_verdict_event` reduces its `(status, basis,
/// correspondence)` tuples to these four facts and calls this, so this is the real decision, not a
/// copy. The `#[ensures]` links it to [`is_event_logic`] so the whole-case equality is discharged
/// deductively.
// Plain decision — the app delegates to it; gated out under `verus_only`, where the `verus! { }`
// mirror owns the name (#600).
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == is_event_logic(present, status_eq, basis_eq, corr_eq)))]
pub fn is_event(present: bool, status_eq: bool, basis_eq: bool, corr_eq: bool) -> bool {
    !present || !(status_eq && basis_eq && corr_eq)
}

/// REQ078's sort: the facts the event decision reads — whether a prior verdict is present, and
/// whether its status, basis, and correspondence each equal the new verdict's.
#[cfg(any(kani, creusot, prusti))]
pub struct VerdictEventInputs {
    pub present: bool,
    pub status_eq: bool,
    pub basis_eq: bool,
    pub corr_eq: bool,
}

#[cfg(kani)]
impl kani::Arbitrary for VerdictEventInputs {
    fn any() -> Self {
        VerdictEventInputs {
            present: kani::any(),
            status_eq: kani::any(),
            basis_eq: kani::any(),
            corr_eq: kani::any(),
        }
    }
}

/// REQ078 stated as the rule the requirement reads and checked against the real [`is_event`]: with no
/// prior verdict it is an event (the first verdict); with a prior verdict equal in all three fields
/// it is **not** an event (an identical re-verify); with any field changed it is an event — which
/// covers a basis- or correspondence-only change under a **steady status** (#535). Written as the
/// case table, not off `is_event`'s own branch, so the two agree only if the decision is right.
///
/// `#[trusted]` under Creusot: the Creusot proof runs against the pearlite mirror
/// [`is_event_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn is_event_is_sound(i: &VerdictEventInputs) -> bool {
    let got = is_event(i.present, i.status_eq, i.basis_eq, i.corr_eq);
    if !i.present {
        got
    } else if i.status_eq && i.basis_eq && i.corr_eq {
        !got
    } else {
        got
    }
}

/// Logic mirror of [`is_event`]; `with_mirrors` redirects the harness onto it and the program
/// decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn is_event_logic(present: bool, status_eq: bool, basis_eq: bool, corr_eq: bool) -> bool {
    pearlite! { !present || !(status_eq && basis_eq && corr_eq) }
}

/// Logic mirror of [`is_event_is_sound`]; `with_mirrors` redirects the harness onto this. It restates
/// REQ078's case table reading the decision through [`is_event_logic`], tied to the real code by the
/// program decision's `#[ensures]`.
#[cfg(creusot)]
#[logic(open)]
pub fn is_event_is_sound_logic(i: &VerdictEventInputs) -> bool {
    pearlite! {
        let got = is_event_logic(i.present, i.status_eq, i.basis_eq, i.corr_eq);
        if !i.present {
            got
        } else if i.status_eq && i.basis_eq && i.corr_eq {
            !got
        } else {
            got
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_verdict_is_an_event() {
        // No prior verdict → event, whatever the (don't-care) equality flags.
        assert!(is_event(false, false, false, false));
    }

    #[test]
    fn an_identical_verdict_is_not_an_event() {
        assert!(!is_event(true, true, true, true));
    }

    #[test]
    fn any_single_field_change_is_an_event() {
        assert!(is_event(true, false, true, true)); // status changed
        assert!(is_event(true, true, false, true)); // basis changed (#535, steady status)
        assert!(is_event(true, true, true, false)); // correspondence changed (#535, steady status)
    }
}

// REQ093/#600 — the Verus mirror: independent deductive corroboration of REQ078 alongside Creusot.
// Loop-free finite case analysis over four bools; the generated harness body
// `is_event_is_sound(&i)` is a function call that triggers on itself — no `#[trigger]`. The plain
// `is_event` is gated `not(verus_only)` so this owns the name under Verus; `VerdictEventInputs`/
// `is_event_is_sound` are `cfg(any(kani, creusot, prusti))`, excluded under Verus, so the `verus!`
// versions are the only ones it sees.
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub open spec fn is_event(present: bool, status_eq: bool, basis_eq: bool, corr_eq: bool) -> bool {
    !present || !(status_eq && basis_eq && corr_eq)
}

pub struct VerdictEventInputs {
    pub present: bool,
    pub status_eq: bool,
    pub basis_eq: bool,
    pub corr_eq: bool,
}

pub open spec fn is_event_is_sound(i: &VerdictEventInputs) -> bool {
    let got = is_event(i.present, i.status_eq, i.basis_eq, i.corr_eq);
    if !i.present {
        got
    } else if i.status_eq && i.basis_eq && i.corr_eq {
        !got
    } else {
        got
    }
}

} // verus!
