//! REQ028 — a model binding resolves only on an unambiguous, arity-agreeing match. The IO-bound
//! `tla_adapter::resolve` delegates its decision to [`classify_binding`].

#[cfg(creusot)]
use creusot_std::macros::*;

/// The payload-free shape of a model resolution — what [`classify_binding`] decides before the
/// IO-bound resolver reattaches the matched definition(s). `WrongArity` keeps the declared arity it
/// read (a plain `usize`, so no heap). Deliberately no `PartialEq` derive: the proofs match on it.
#[derive(Debug, Clone, Copy)]
pub enum BindingKind {
    NotFound,
    Resolved,
    Ambiguous,
    WrongArity { declared: usize },
}

/// The resolution a binding earns from the only facts the decision reads: how many definitions
/// carry the name, and — for a lone match — the arity that line declares (`None` when it states
/// none). Zero is not-found; several is ambiguous (the declared arity is never consulted — ambiguity
/// is reported first); exactly one resolves unless its line declares an arity that disagrees.
#[cfg_attr(creusot, ensures(result == classify_binding_logic(match_count, lone_declared_arity, expected_arity)))]
pub fn classify_binding(
    match_count: usize,
    lone_declared_arity: Option<usize>,
    expected_arity: usize,
) -> BindingKind {
    match match_count {
        0 => BindingKind::NotFound,
        1 => match lone_declared_arity {
            Some(declared) if declared != expected_arity => BindingKind::WrongArity { declared },
            _ => BindingKind::Resolved,
        },
        _ => BindingKind::Ambiguous,
    }
}

/// REQ028's model-binding resolver stated as a proof sort: how many definitions share the name, the
/// lone match's declared arity, and the requirement's expected arity — the only facts
/// [`classify_binding`] reads.
#[cfg(any(kani, creusot, prusti))]
pub struct ModelBindingCase {
    pub match_count: usize,
    pub lone_declared_arity: Option<usize>,
    pub expected_arity: usize,
}

#[cfg(kani)]
impl kani::Arbitrary for ModelBindingCase {
    fn any() -> Self {
        // Small bounds: the decision only branches on 0 / 1 / several and on arity (dis)agreement,
        // so 0..4 exercises every case with room to spare.
        ModelBindingCase {
            match_count: (kani::any::<u8>() % 4) as usize,
            lone_declared_arity: if kani::any() {
                Some((kani::any::<u8>() % 4) as usize)
            } else {
                None
            },
            expected_arity: (kani::any::<u8>() % 4) as usize,
        }
    }
}

/// REQ028 stated as a property and checked against the real [`classify_binding`]: every outcome
/// keeps the promise its variant makes. Written as the guarantee each variant owes, not as the
/// classifier's own branches, so the two agree only if the rule is right.
#[cfg(any(kani, creusot, prusti))]
pub fn model_binding_is_sound(case: &ModelBindingCase) -> bool {
    let kind = classify_binding(
        case.match_count,
        case.lone_declared_arity,
        case.expected_arity,
    );
    let lone = case.match_count == 1;
    let arity_agrees = match case.lone_declared_arity {
        Some(declared) => declared == case.expected_arity,
        None => true,
    };
    match kind {
        BindingKind::Resolved => lone && arity_agrees,
        BindingKind::NotFound => case.match_count == 0,
        BindingKind::Ambiguous => case.match_count >= 2,
        BindingKind::WrongArity { declared } => {
            lone && case.lone_declared_arity == Some(declared) && declared != case.expected_arity
        }
    }
}

/// Logic mirror of [`classify_binding`]. Written with `if`/`else` on the counts rather than a match
/// on integer literals, which pearlite does not accept.
#[cfg(creusot)]
#[logic(open)]
pub fn classify_binding_logic(
    match_count: usize,
    lone_declared_arity: Option<usize>,
    expected_arity: usize,
) -> BindingKind {
    pearlite! {
        if match_count == 0usize {
            BindingKind::NotFound
        } else if match_count == 1usize {
            match lone_declared_arity {
                Some(declared) => {
                    if declared != expected_arity {
                        BindingKind::WrongArity { declared }
                    } else {
                        BindingKind::Resolved
                    }
                }
                None => BindingKind::Resolved,
            }
        } else {
            BindingKind::Ambiguous
        }
    }
}

/// Logic mirror of [`model_binding_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn model_binding_is_sound_logic(case: &ModelBindingCase) -> bool {
    pearlite! {
        let kind = classify_binding_logic(case.match_count, case.lone_declared_arity, case.expected_arity);
        let lone = case.match_count == 1usize;
        let arity_agrees = match case.lone_declared_arity {
            Some(declared) => declared == case.expected_arity,
            None => true,
        };
        match kind {
            BindingKind::Resolved => lone && arity_agrees,
            BindingKind::NotFound => case.match_count == 0usize,
            BindingKind::Ambiguous => case.match_count >= 2usize,
            BindingKind::WrongArity { declared } => {
                lone && case.lone_declared_arity == Some(declared) && declared != case.expected_arity
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_binding_keeps_every_variant_promise() {
        // not found
        assert!(matches!(
            classify_binding(0, None, 2),
            BindingKind::NotFound
        ));
        // lone, arity agrees (or absent) -> resolved
        assert!(matches!(
            classify_binding(1, None, 2),
            BindingKind::Resolved
        ));
        assert!(matches!(
            classify_binding(1, Some(2), 2),
            BindingKind::Resolved
        ));
        // lone, arity disagrees -> wrong arity carrying the declared value
        assert!(matches!(
            classify_binding(1, Some(3), 2),
            BindingKind::WrongArity { declared: 3 }
        ));
        // several -> ambiguous, arity never consulted
        assert!(matches!(
            classify_binding(2, Some(9), 2),
            BindingKind::Ambiguous
        ));
    }
}
