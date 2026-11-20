//! REQ083 — the `expectsCodeTrace` guard. A source item may declare `expectsCodeTrace: false`, which
//! rules out exactly the formalizable-now bucket. A deterministic guard declines any formalizable-now
//! the triage model returns for such an item — leaving it unclassified rather than presenting a hint
//! that contradicts the source's own declaration — while every other bucket the model returns passes
//! through unchanged. `llm::parse_buckets` delegates the per-item decision here, so the proof is about
//! the rule provreq applies.
//!
//! The decision reads only the model's bucket and the item's `expectsCodeTrace` declaration, both
//! payload-free and bounded (`Option<Classification>` has four states, `Option<bool>` three), so the
//! proved core is loop-free: Creusot discharges it as a finite case analysis and Kani checks it over
//! the twelve combinations.

// Only the macros — not the whole `prelude`, whose glob re-exports a `Clone` derive macro that
// collides with std's on the `#[derive(Clone)]` proxy below (E0659).
#[cfg(creusot)]
use creusot_std::macros::*;

/// The triage bucket of one requirement, payload-free. `source::Classification` maps onto this.
/// No `PartialEq` derive (the proofs match on it).
// Plain proxy — gated out under `verus_only`, where the `verus! { }` mirror owns the name (#604).
#[cfg(not(verus_only))]
#[derive(Debug, Clone, Copy)]
pub enum Classification {
    FormalizableNow,
    FalsifiableOnly,
    StaysProse,
}

/// The guard decision: the model's bucket for one item, filtered by the item's `expectsCodeTrace`
/// declaration. A formalizable-now the model returns for an item that declared it does **not** trace
/// to code is declined (`None`); every other bucket passes through unchanged. `llm::parse_buckets`
/// delegates here, so this is the real decision. The `#[ensures]` links it to [`decide_logic`].
// Plain decision — the app delegates to it; gated out under `verus_only`.
#[cfg(not(verus_only))]
#[cfg_attr(creusot, ensures(result == decide_logic(bucket, expects)))]
pub fn decide(bucket: Option<Classification>, expects: Option<bool>) -> Option<Classification> {
    match bucket {
        Some(Classification::FormalizableNow) => match expects {
            // `expectsCodeTrace: false` rules out formalizable-now → decline.
            Some(b) => {
                if b {
                    Some(Classification::FormalizableNow)
                } else {
                    None
                }
            }
            None => Some(Classification::FormalizableNow),
        },
        other => other,
    }
}

/// REQ083's sort: the model's bucket for one item and the item's `expectsCodeTrace` declaration.
#[cfg(any(kani, creusot, prusti))]
pub struct GuardCase {
    pub bucket: Option<Classification>,
    pub expects: Option<bool>,
}

#[cfg(kani)]
impl kani::Arbitrary for Classification {
    fn any() -> Self {
        match kani::any::<u8>() % 3 {
            0 => Classification::FormalizableNow,
            1 => Classification::FalsifiableOnly,
            _ => Classification::StaysProse,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for GuardCase {
    fn any() -> Self {
        GuardCase {
            bucket: if kani::any() { Some(kani::any()) } else { None },
            expects: if kani::any() { Some(kani::any()) } else { None },
        }
    }
}

/// Structural equality on the proxy without a `PartialEq` derive (which would trigger Creusot's
/// `DeepModel` demand). Used only by the executable twin [`decide_is_sound`].
#[cfg(any(kani, creusot, prusti))]
fn opt_class_eq(a: Option<Classification>, b: Option<Classification>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => matches!(
            (x, y),
            (
                Classification::FormalizableNow,
                Classification::FormalizableNow
            ) | (
                Classification::FalsifiableOnly,
                Classification::FalsifiableOnly
            ) | (Classification::StaysProse, Classification::StaysProse)
        ),
        _ => false,
    }
}

/// REQ083 stated as the rule the requirement reads and checked against the real [`decide`]: a
/// ruled-out formalizable-now (the model returned formalizable-now **and** the source declared
/// `expectsCodeTrace: false`) is declined; every other case passes the model's bucket through
/// unchanged. Written off the declaration, not `decide`'s own branch, so the two agree only if the
/// guard is right.
///
/// `#[trusted]` under Creusot: the Creusot proof runs against the pearlite mirror
/// [`decide_is_sound_logic`] (via `with_mirrors`), whose content is the `#[ensures]`-linked decision.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn decide_is_sound(c: &GuardCase) -> bool {
    let got = decide(c.bucket, c.expects);
    let ruled_out =
        matches!(c.bucket, Some(Classification::FormalizableNow)) && c.expects == Some(false);
    if ruled_out {
        matches!(got, None)
    } else {
        opt_class_eq(got, c.bucket)
    }
}

/// Logic mirror of [`decide`]; the program decision's `#[ensures]` is stated through it.
#[cfg(creusot)]
#[logic(open)]
pub fn decide_logic(
    bucket: Option<Classification>,
    expects: Option<bool>,
) -> Option<Classification> {
    pearlite! {
        match bucket {
            Some(Classification::FormalizableNow) => match expects {
                Some(b) => if b { Some(Classification::FormalizableNow) } else { None },
                None => Some(Classification::FormalizableNow),
            },
            other => other,
        }
    }
}

/// Logic mirror of [`decide_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn decide_is_sound_logic(c: &GuardCase) -> bool {
    pearlite! {
        let got = decide_logic(c.bucket, c.expects);
        let ruled_out = match c.bucket {
            Some(Classification::FormalizableNow) => c.expects == Some(false),
            _ => false,
        };
        if ruled_out {
            match got { None => true, _ => false }
        } else {
            got == c.bucket
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_fnow(o: Option<Classification>) -> bool {
        matches!(o, Some(Classification::FormalizableNow))
    }

    #[test]
    fn a_ruled_out_formalizable_now_is_declined() {
        assert!(decide(Some(Classification::FormalizableNow), Some(false)).is_none());
    }

    #[test]
    fn formalizable_now_survives_when_not_ruled_out() {
        assert!(is_fnow(decide(
            Some(Classification::FormalizableNow),
            Some(true)
        )));
        assert!(is_fnow(decide(Some(Classification::FormalizableNow), None)));
    }

    #[test]
    fn other_buckets_pass_through_even_when_ruled_out() {
        assert!(matches!(
            decide(Some(Classification::FalsifiableOnly), Some(false)),
            Some(Classification::FalsifiableOnly)
        ));
        assert!(matches!(
            decide(Some(Classification::StaysProse), Some(false)),
            Some(Classification::StaysProse)
        ));
        assert!(decide(None, Some(false)).is_none());
    }
}

// REQ093/#604 — the Verus mirror: independent deductive corroboration of REQ083 alongside Creusot.
// Loop-free finite case analysis over `Option<Classification>` × `Option<bool>`; the generated
// harness body `decide_is_sound(&c)` is a function call that triggers on itself — no `#[trigger]`. The
// plain `Classification`/`decide` are gated `not(verus_only)` so these own the names under Verus;
// `GuardCase`/`decide_is_sound` are `cfg(any(kani, creusot, prusti))`, excluded under Verus, so the
// `verus!` versions are the only ones it sees. Verus spec `==` is structural, so the proxy needs no
// `PartialEq` here (the manual `opt_class_eq` above is only for the Kani executable twin).
#[cfg(verus_only)]
use vstd::prelude::*;

#[cfg(verus_only)]
verus! {

pub enum Classification { FormalizableNow, FalsifiableOnly, StaysProse }

pub open spec fn decide(bucket: Option<Classification>, expects: Option<bool>) -> Option<Classification> {
    match bucket {
        Some(Classification::FormalizableNow) => match expects {
            Some(b) => if b { Some(Classification::FormalizableNow) } else { None },
            None => Some(Classification::FormalizableNow),
        },
        other => other,
    }
}

pub struct GuardCase { pub bucket: Option<Classification>, pub expects: Option<bool> }

pub open spec fn decide_is_sound(c: &GuardCase) -> bool {
    let got = decide(c.bucket, c.expects);
    let ruled_out = match c.bucket {
        Some(Classification::FormalizableNow) => c.expects == Some(false),
        _ => false,
    };
    if ruled_out {
        got is None
    } else {
        got == c.bucket
    }
}

} // verus!
