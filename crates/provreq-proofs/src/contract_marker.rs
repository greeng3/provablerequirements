//! REQ033 — the A6 proof-carrier **marker-draft guard**. When a grounded category-1 predicate
//! resolves to an opaque `fn`, provreq drafts the missing deductive marker onto it — but only in a
//! dialect where the marker is transparent *in place*. Prusti's `#[pure]` is: the function stays in
//! the program, keeps its callers, and becomes spec-callable. Creusot's `#[logic]` is not: it
//! declares a logical function, moving the item out of the program namespace so every real call site
//! stops compiling. So the marker channel drafts for Prusti and never for Creusot — Creusot reaches
//! the same predicate through a `#[logic]` mirror instead ([`crate::mirror_draft`], #158).
//! `contract_draft::Marker::drafts_markers` delegates the guard here, so the proof is about the rule
//! provreq applies.
//!
//! The decision reads only which dialect's marker it is — payload-free — so the proved core is
//! loop-free: Creusot discharges it as a two-case analysis and Kani checks it over both markers.
//! Scope is marker-only; the sibling `drafts_contracts` channel (#164) is a separate decision.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxy below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// The deductive transparency marker a subject's verifier reads. Payload-free proxy for
/// `contract_draft::Marker`; no `PartialEq` derive (the proofs match on it). Serves as REQ033's sort
/// directly — the decision reads only which variant it is.
// Plain proxy — gated out under `verus_only`, where the `verus! { }` mirror owns the name (#608).
#[cfg(not(verus_only))]
#[derive(Debug, Clone, Copy)]
pub enum Marker {
    /// Creusot's logical-function marker (`#[logic]`).
    Logic,
    /// Prusti's pure-function marker (`#[pure]`).
    Pure,
}

/// Whether drafting this marker onto the subject's own predicate fn is a route to a proof: yes for
/// Prusti's in-place `#[pure]`, no for Creusot's program-removing `#[logic]`.
/// `contract_draft::Marker::drafts_markers` delegates here, so this is the real decision. The
/// `#[ensures]` links it to [`drafts_markers_logic`].
// Plain decision — the app delegates to it; gated out under `verus_only`.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == drafts_markers_logic(m)))]
pub fn drafts_markers(m: Marker) -> bool {
    match m {
        Marker::Logic => false,
        Marker::Pure => true,
    }
}

#[cfg(kani)]
impl kani::Arbitrary for Marker {
    fn any() -> Self {
        if kani::any() {
            Marker::Logic
        } else {
            Marker::Pure
        }
    }
}

/// REQ033 stated as the rule the requirement reads and checked against the real [`drafts_markers`]: a
/// marker is drafted for Prusti (`Pure`) and never for Creusot (`Logic`). Written as the per-dialect
/// case, not off `drafts_markers`'s own body, so the two agree only if the decision is right.
///
/// `#[trusted]` under Creusot: the Creusot proof runs against the pearlite mirror
/// [`drafts_markers_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked
/// decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn drafts_markers_is_sound(m: &Marker) -> bool {
    let got = drafts_markers(*m);
    match m {
        Marker::Logic => !got,
        Marker::Pure => got,
    }
}

/// Logic mirror of [`drafts_markers`]; the program decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn drafts_markers_logic(m: Marker) -> bool {
    pearlite! {
        match m {
            Marker::Logic => false,
            Marker::Pure => true,
        }
    }
}

/// Logic mirror of [`drafts_markers_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn drafts_markers_is_sound_logic(m: &Marker) -> bool {
    pearlite! {
        let got = drafts_markers_logic(*m);
        match m {
            Marker::Logic => !got,
            Marker::Pure => got,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_is_drafted_for_prusti_and_never_for_creusot() {
        assert!(drafts_markers(Marker::Pure));
        assert!(!drafts_markers(Marker::Logic));
    }
}

// REQ093/#608 — the Verus mirror: independent deductive corroboration of REQ033 alongside Creusot.
// Loop-free two-case analysis over the marker; the generated harness body
// `drafts_markers_is_sound(&m)` is a function call that triggers on itself — no `#[trigger]`. The
// plain `Marker`/`drafts_markers` are gated `not(verus_only)` so these own the names under Verus;
// `drafts_markers_is_sound` is `cfg(any(kani, creusot, prusti))`, excluded under Verus, so the
// `verus!` version is the only one it sees.
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub enum Marker { Logic, Pure }

pub open spec fn drafts_markers(m: Marker) -> bool {
    match m {
        Marker::Logic => false,
        Marker::Pure => true,
    }
}

pub open spec fn drafts_markers_is_sound(m: &Marker) -> bool {
    let got = drafts_markers(*m);
    match m {
        Marker::Logic => !got,
        Marker::Pure => got,
    }
}

} // verus!
