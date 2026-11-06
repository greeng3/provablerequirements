//! REQ011 — the coverage funnel's triage counts partition the discovered items. `status::coverage`
//! delegates its per-item bucketing to [`triage_bucket`].

#[cfg(creusot)]
use creusot_std::macros::*;

/// The payload-free triage classification of one item. `status::Classification` maps onto this.
/// No `PartialEq` derive: the proofs match on it.
#[derive(Debug, Clone, Copy)]
pub enum Classification {
    FormalizableNow,
    FalsifiableOnly,
    StaysProse,
}

/// The four honest triage buckets of the coverage funnel. `None` classification is untriaged — its
/// own state, never folded into stays-prose or either formalizable bucket.
#[derive(Debug, Clone, Copy)]
pub enum TriageBucket {
    Untriaged,
    FormalizableNow,
    FalsifiableOnly,
    StaysProse,
}

/// Which bucket one item falls in, from its classification alone. Pure and total, so the funnel's
/// counts partition the items.
#[cfg_attr(creusot, ensures(result == triage_bucket_logic(classification)))]
pub fn triage_bucket(classification: Option<Classification>) -> TriageBucket {
    match classification {
        None => TriageBucket::Untriaged,
        Some(Classification::FormalizableNow) => TriageBucket::FormalizableNow,
        Some(Classification::FalsifiableOnly) => TriageBucket::FalsifiableOnly,
        Some(Classification::StaysProse) => TriageBucket::StaysProse,
    }
}

/// REQ011's honest-funnel rule as a proof sort: one item's triage classification (`None` =
/// untriaged).
#[cfg(any(kani, creusot, prusti))]
pub struct FunnelCase {
    pub classification: Option<Classification>,
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
impl kani::Arbitrary for FunnelCase {
    fn any() -> Self {
        FunnelCase {
            classification: if kani::any() { Some(kani::any()) } else { None },
        }
    }
}

/// REQ011 stated as a property and checked against the real [`triage_bucket`]: the four buckets are
/// mutually exclusive and exhaustive, so they partition the discovered items. The membership
/// predicates are written straight off the classification — not off `triage_bucket`'s arms — so the
/// two agree only if the real classifier keeps the honest states distinct.
#[cfg(any(kani, creusot, prusti))]
pub fn triage_partition_is_honest(case: &FunnelCase) -> bool {
    let c = case.classification;
    let untriaged = matches!(c, None);
    let formalizable = matches!(c, Some(Classification::FormalizableNow));
    let falsifiable = matches!(c, Some(Classification::FalsifiableOnly));
    let prose = matches!(c, Some(Classification::StaysProse));
    // Exactly one honest state holds (exhaustive + mutually exclusive). Counted with arithmetic,
    // not an iterator, so Kani's unwind bound is never in play.
    let exactly_one = untriaged as u8 + formalizable as u8 + falsifiable as u8 + prose as u8 == 1;
    // ...and the bucket the funnel counts under is that same state.
    let bucket_matches = match triage_bucket(c) {
        TriageBucket::Untriaged => untriaged,
        TriageBucket::FormalizableNow => formalizable,
        TriageBucket::FalsifiableOnly => falsifiable,
        TriageBucket::StaysProse => prose,
    };
    exactly_one && bucket_matches
}

/// Logic mirror of [`triage_bucket`].
#[cfg(creusot)]
#[logic(open)]
pub fn triage_bucket_logic(classification: Option<Classification>) -> TriageBucket {
    pearlite! {
        match classification {
            None => TriageBucket::Untriaged,
            Some(Classification::FormalizableNow) => TriageBucket::FormalizableNow,
            Some(Classification::FalsifiableOnly) => TriageBucket::FalsifiableOnly,
            Some(Classification::StaysProse) => TriageBucket::StaysProse,
        }
    }
}

/// Logic mirror of [`triage_partition_is_honest`]; `with_mirrors` redirects the harness onto this.
/// The four cases are disjoint and exhaustive, so "exactly one state holds" is structural; what the
/// proof carries is that `triage_bucket` lands each case in its own bucket.
#[cfg(creusot)]
#[logic(open)]
pub fn triage_partition_is_honest_logic(case: &FunnelCase) -> bool {
    // pearlite `==` is logical equality, defined on any type, so no `PartialEq` derive is needed on
    // `TriageBucket` (and the `DeepModel` demand that derive would trigger is avoided).
    pearlite! {
        match case.classification {
            None => triage_bucket_logic(case.classification) == TriageBucket::Untriaged,
            Some(Classification::FormalizableNow) => triage_bucket_logic(case.classification) == TriageBucket::FormalizableNow,
            Some(Classification::FalsifiableOnly) => triage_bucket_logic(case.classification) == TriageBucket::FalsifiableOnly,
            Some(Classification::StaysProse) => triage_bucket_logic(case.classification) == TriageBucket::StaysProse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_classification_lands_in_its_own_bucket() {
        assert!(matches!(triage_bucket(None), TriageBucket::Untriaged));
        assert!(matches!(
            triage_bucket(Some(Classification::FormalizableNow)),
            TriageBucket::FormalizableNow
        ));
        assert!(matches!(
            triage_bucket(Some(Classification::FalsifiableOnly)),
            TriageBucket::FalsifiableOnly
        ));
        assert!(matches!(
            triage_bucket(Some(Classification::StaysProse)),
            TriageBucket::StaysProse
        ));
    }
}
