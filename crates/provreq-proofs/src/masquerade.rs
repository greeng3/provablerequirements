//! REQ076 — the masquerade guard. A verdict's correspondence reads `Asserted` only when there is at
//! least one piece of evidence that *decides* its polarity and every such decisive piece is itself
//! asserted; a single decisive mechanical piece — or no decisive evidence at all — makes it
//! `Mechanical`. `verdict::decisive_correspondence` delegates the fold to [`decisive_correspondence_of`]
//! here, so the proof is about the rule provreq actually applies.
//!
//! Unlike the other cores in this crate, this one folds over an **arbitrary-length** slice, so the
//! fixed-array sort that carries REQ030/REQ011/… does not carry it (#539). The program fold states
//! REQ076 as a `#[cfg(creusot)]` `#[ensures]` over the slice's whole length and discharges it with
//! bounded-quantifier loop invariants — a genuine ∀-length deductive proof (`proven`), not the
//! length-3 reimplementation the leaf-mirror pattern would give. Kani still checks the same core at
//! the length-3 bound, since `ProofDecisive` is `Arbitrary` as a 3-element `Vec`.

// Only the macros and the logic types — not the whole `prelude`, whose glob re-exports a `Clone`
// derive macro that collides with std's on the `#[derive(Clone)]` proxies below (E0659).
#[cfg(creusot)]
use creusot_std::logic::{Int, Seq};
#[cfg(creusot)]
use creusot_std::macros::*;

/// The verdict polarity a correspondence is being decided for. Payload-free proxy for
/// `verdict::Status`; no `PartialEq` derive (the derive is what triggers Creusot's `DeepModel`
/// demand — the proofs match on it instead).
#[derive(Debug, Clone, Copy)]
pub enum Status {
    Holds,
    Fails,
    Unknown,
}

/// Whether one piece of evidence is asserted or merely mechanical. Payload-free proxy for
/// `verdict::Correspondence`.
#[derive(Debug, Clone, Copy)]
pub enum Correspondence {
    Mechanical,
    Asserted,
}

/// Whether a piece's polarity `piece` is **decisive** for a verdict of polarity `verdict` — the
/// holders for a `holds`, the refuters for a `fails`, either side of a divergence (`divergence`),
/// the unknowns for an ordinary unknown. The guard reads only this subset. `divergence` collapses
/// the real `Option<UnknownReason>` to the one bit the decision reads. The `#[ensures]` links it to
/// [`is_decisive_logic`] so the fold's loop invariants, written in terms of the logic mirror, can see
/// which branch each iteration takes.
#[cfg_attr(creusot, ensures(result == is_decisive_logic(verdict, divergence, piece)))]
pub fn is_decisive(verdict: Status, divergence: bool, piece: Status) -> bool {
    match verdict {
        Status::Holds => matches!(piece, Status::Holds),
        Status::Fails => matches!(piece, Status::Fails),
        Status::Unknown if divergence => matches!(piece, Status::Holds | Status::Fails),
        Status::Unknown => matches!(piece, Status::Unknown),
    }
}

/// The masquerade guard over the whole evidence slice: `Asserted` only when at least one piece is
/// decisive and every decisive piece is asserted, else `Mechanical`. Pure; `verdict::
/// decisive_correspondence` reduces each `Evidence` to its `(Status, Correspondence)` and calls this,
/// so this is the real decision, not a copy.
///
/// The `#[ensures]` links the fold to [`decisive_correspondence_of_logic`], which states REQ076
/// directly over the whole `Seq` as `∃ decisive ∧ ∀ decisive asserted`; the loop invariants make
/// `saw`/`all_asserted` the running `∃`/`∀` over the prefix, so Creusot discharges the whole-length
/// property by prefix extension — no array bound.
#[cfg_attr(creusot, ensures(
    result == decisive_correspondence_of_logic(verdict, divergence, pieces@)
))]
pub fn decisive_correspondence_of(
    verdict: Status,
    divergence: bool,
    pieces: &[(Status, Correspondence)],
) -> Correspondence {
    let mut saw = false;
    let mut all_asserted = true;
    let mut i = 0;
    #[cfg_attr(creusot, invariant(0 <= i@ && i@ <= pieces@.len()))]
    #[cfg_attr(creusot, invariant(saw
        == (exists<j: Int> 0 <= j && j < i@
            && is_decisive_logic(verdict, divergence, (pieces@[j]).0))))]
    #[cfg_attr(creusot, invariant(all_asserted
        == (forall<j: Int> 0 <= j && j < i@
            && is_decisive_logic(verdict, divergence, (pieces@[j]).0)
            ==> (pieces@[j]).1 == Correspondence::Asserted)))]
    while i < pieces.len() {
        let (s, c) = pieces[i];
        if is_decisive(verdict, divergence, s) {
            saw = true;
            if let Correspondence::Mechanical = c {
                all_asserted = false;
            }
        }
        i += 1;
    }
    if saw && all_asserted {
        Correspondence::Asserted
    } else {
        Correspondence::Mechanical
    }
}

/// Logic mirror of [`is_decisive`]; `with_mirrors` redirects the harness onto it, and the program
/// fold's `#[ensures]`/invariants are stated in terms of it.
#[cfg(creusot)]
#[logic(open)]
pub fn is_decisive_logic(verdict: Status, divergence: bool, piece: Status) -> bool {
    pearlite! {
        match verdict {
            Status::Holds => match piece { Status::Holds => true, _ => false },
            Status::Fails => match piece { Status::Fails => true, _ => false },
            Status::Unknown => if divergence {
                match piece { Status::Holds => true, Status::Fails => true, _ => false }
            } else {
                match piece { Status::Unknown => true, _ => false }
            },
        }
    }
}

/// Logic mirror of [`decisive_correspondence_of`]: REQ076's rule over the whole `Seq`. `Asserted`
/// exactly when at least one piece is decisive and every decisive piece is asserted, else
/// `Mechanical`. The program fold is `#[ensures]`-linked to this, so discharging that link is the
/// genuine ∀-length proof; `open` lets the solver unfold it against the loop invariants.
#[cfg(creusot)]
#[logic(open)]
pub fn decisive_correspondence_of_logic(
    verdict: Status,
    divergence: bool,
    pieces: Seq<(Status, Correspondence)>,
) -> Correspondence {
    pearlite! {
        if (exists<i: Int> 0 <= i && i < pieces.len()
                && is_decisive_logic(verdict, divergence, (pieces[i]).0))
            && (forall<i: Int> 0 <= i && i < pieces.len()
                && is_decisive_logic(verdict, divergence, (pieces[i]).0)
                ==> (pieces[i]).1 == Correspondence::Asserted)
        {
            Correspondence::Asserted
        } else {
            Correspondence::Mechanical
        }
    }
}

/// REQ076's masquerade-guard sort: a verdict polarity, whether it is a divergence, and the evidence
/// pieces' `(polarity, correspondence)`. A `Vec` so Creusot's proof ranges over every length; Kani's
/// `Arbitrary` builds it from a fixed 3-element array, keeping the bounded check in play.
#[cfg(any(kani, creusot, prusti))]
pub struct ProofDecisive {
    pub verdict: Status,
    pub divergence: bool,
    pub pieces: Vec<(Status, Correspondence)>,
}

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
impl kani::Arbitrary for Correspondence {
    fn any() -> Self {
        if kani::any() {
            Correspondence::Mechanical
        } else {
            Correspondence::Asserted
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for ProofDecisive {
    fn any() -> Self {
        let pieces: [(Status, Correspondence); 3] = kani::any();
        ProofDecisive {
            verdict: kani::any(),
            divergence: kani::any(),
            pieces: pieces.to_vec(),
        }
    }
}

/// REQ076 stated as a property and checked against the real [`decisive_correspondence_of`]: a verdict
/// reads `Asserted` only when there is at least one decisive piece and *every* decisive piece is
/// asserted — otherwise `Mechanical`. The expected answer is written as "present and all-asserted",
/// the same shape as the guard's `#[ensures]`, so a pass means the fold meets it over every length
/// (Creusot) and at the length-3 bound (Kani).
///
/// `#[trusted]` under Creusot: this is the executable twin Kani runs, built from iterator `any`/`all`
/// that Creusot has no contract for. The Creusot proof runs against the pearlite mirror
/// [`decisive_correspondence_is_sound_logic`] (via `with_mirrors`), whose real content is the
/// `#[ensures]`-linked fold — so Creusot need not verify this body.
#[cfg(any(kani, creusot, prusti))]
#[cfg_attr(creusot, trusted)]
pub fn decisive_correspondence_is_sound(d: &ProofDecisive) -> bool {
    let got = decisive_correspondence_of(d.verdict, d.divergence, &d.pieces);
    let any_decisive = d
        .pieces
        .iter()
        .any(|(s, _)| is_decisive(d.verdict, d.divergence, *s));
    let all_decisive_asserted = d
        .pieces
        .iter()
        .filter(|(s, _)| is_decisive(d.verdict, d.divergence, *s))
        .all(|(_, c)| matches!(c, Correspondence::Asserted));
    let asserted = matches!(got, Correspondence::Asserted);
    asserted == (any_decisive && all_decisive_asserted)
}

/// Logic mirror of [`decisive_correspondence_is_sound`]; `with_mirrors` redirects the harness onto
/// this. It restates the property with pearlite quantifiers over `d.pieces@`; the program fold's
/// `#[ensures]` is what ties this to the real decision for every length.
#[cfg(creusot)]
#[logic(open)]
pub fn decisive_correspondence_is_sound_logic(d: &ProofDecisive) -> bool {
    pearlite! {
        let any_decisive = exists<i: Int> 0 <= i && i < d.pieces@.len()
            && is_decisive_logic(d.verdict, d.divergence, (d.pieces@[i]).0);
        let all_decisive_asserted = forall<i: Int> 0 <= i && i < d.pieces@.len()
            && is_decisive_logic(d.verdict, d.divergence, (d.pieces@[i]).0)
            ==> (d.pieces@[i]).1 == Correspondence::Asserted;
        (decisive_correspondence_of_logic(d.verdict, d.divergence, d.pieces@)
            == Correspondence::Asserted)
            == (any_decisive && all_decisive_asserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(c: Correspondence) -> (Status, Correspondence) {
        (Status::Holds, c)
    }

    #[test]
    fn asserted_only_when_every_decisive_piece_is_asserted() {
        use Correspondence::*;
        // All decisive (holders for a holds) asserted → asserted.
        assert!(matches!(
            decisive_correspondence_of(Status::Holds, false, &[h(Asserted), h(Asserted)]),
            Asserted
        ));
        // One decisive mechanical → mechanical.
        assert!(matches!(
            decisive_correspondence_of(Status::Holds, false, &[h(Asserted), h(Mechanical)]),
            Mechanical
        ));
        // A mechanical piece that is NOT decisive for this polarity does not pull it down.
        assert!(matches!(
            decisive_correspondence_of(
                Status::Holds,
                false,
                &[h(Asserted), (Status::Unknown, Mechanical)]
            ),
            Asserted
        ));
        // No decisive evidence at all → mechanical.
        assert!(matches!(
            decisive_correspondence_of(Status::Holds, false, &[]),
            Mechanical
        ));
    }
}
