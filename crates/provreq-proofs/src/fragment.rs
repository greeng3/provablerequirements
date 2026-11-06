//! REQ024 — the category-1 fragment is temporal-free, and `can_reach` is branching-time. The gate
//! in `prl::fragment` delegates its refusal *decision* to [`refuses`] (it keeps the message itself).

#[cfg(creusot)]
use creusot_std::macros::*;

/// The requirement categories, payload-free. `prl::ast::Category` maps onto this.
#[derive(Debug, Clone, Copy)]
pub enum Category {
    Code,
    Model,
    Runtime,
    Ui,
}

/// A pattern with its expression payloads stripped — the only thing the fragment rule reads is the
/// shape. `prl::fragment::PatternKind` maps onto this. No `PartialEq` derive: the proofs match on it.
#[derive(Debug, Clone, Copy)]
pub enum PatternKind {
    Never,
    Always,
    Eventually,
    LeadsTo,
    Precedes,
    OccursAtMost,
    CanReach,
}

/// Whether `category`'s engine cannot express a pattern of this shape — the fragment gate's refusal
/// decision, which `prl::fragment::fragment_rule` delegates here. Rule 1: the code fragment is
/// temporal-free, so only the safety shapes (`always`/`never`) survive at category 1. Rule 2:
/// `can_reach` is branching-time (CTL `EF`) and only a model checker (2a) has the state graph.
#[cfg_attr(creusot, ensures(result == refuses_logic(category, kind)))]
pub fn refuses(category: Category, kind: PatternKind) -> bool {
    match category {
        Category::Code => !matches!(kind, PatternKind::Always | PatternKind::Never),
        Category::Model => false,
        Category::Runtime | Category::Ui => matches!(kind, PatternKind::CanReach),
    }
}

/// REQ024's fragment rule as a proof sort: a category paired with a pattern shape.
#[cfg(any(kani, creusot, prusti))]
pub struct FragmentCase {
    pub category: Category,
    pub kind: PatternKind,
}

#[cfg(kani)]
impl kani::Arbitrary for Category {
    fn any() -> Self {
        match kani::any::<u8>() % 4 {
            0 => Category::Code,
            1 => Category::Model,
            2 => Category::Runtime,
            _ => Category::Ui,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for PatternKind {
    fn any() -> Self {
        match kani::any::<u8>() % 7 {
            0 => PatternKind::Never,
            1 => PatternKind::Always,
            2 => PatternKind::Eventually,
            3 => PatternKind::LeadsTo,
            4 => PatternKind::Precedes,
            5 => PatternKind::OccursAtMost,
            _ => PatternKind::CanReach,
        }
    }
}

#[cfg(kani)]
impl kani::Arbitrary for FragmentCase {
    fn any() -> Self {
        FragmentCase {
            category: kani::any(),
            kind: kani::any(),
        }
    }
}

/// REQ024 stated as a property and checked against the real [`refuses`]: the code fragment
/// (category 1) is temporal-free, so it refuses every shape except the safety ones; `can_reach` is
/// branching-time and refused everywhere except category 2a (the model). The expected answer is
/// rebuilt from those two rules alone — not from `refuses`' own arms — so the two agree only if the
/// real gate is right.
#[cfg(any(kani, creusot, prusti))]
pub fn fragment_is_sound(case: &FragmentCase) -> bool {
    let refused = refuses(case.category, case.kind);
    let is_safety = matches!(case.kind, PatternKind::Always | PatternKind::Never);
    let is_can_reach = matches!(case.kind, PatternKind::CanReach);
    let expected_refused = match case.category {
        Category::Code => !is_safety,
        Category::Model => false,
        Category::Runtime | Category::Ui => is_can_reach,
    };
    refused == expected_refused
}

/// Logic mirror of [`refuses`]. Nested matches (no tuple/or-patterns, which pearlite does not take).
#[cfg(creusot)]
#[logic(open)]
pub fn refuses_logic(category: Category, kind: PatternKind) -> bool {
    pearlite! {
        match category {
            Category::Code => match kind {
                PatternKind::Always => false,
                PatternKind::Never => false,
                _ => true,
            },
            Category::Model => false,
            _ => match kind {
                PatternKind::CanReach => true,
                _ => false,
            },
        }
    }
}

/// Logic mirror of [`fragment_is_sound`]; `with_mirrors` redirects the harness onto this.
#[cfg(creusot)]
#[logic(open)]
pub fn fragment_is_sound_logic(case: &FragmentCase) -> bool {
    pearlite! {
        let refused = refuses_logic(case.category, case.kind);
        let is_safety = match case.kind {
            PatternKind::Always => true,
            PatternKind::Never => true,
            _ => false,
        };
        let is_can_reach = match case.kind {
            PatternKind::CanReach => true,
            _ => false,
        };
        let expected_refused = match case.category {
            Category::Code => !is_safety,
            Category::Model => false,
            _ => is_can_reach,
        };
        refused == expected_refused
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_follows_the_two_design_rules() {
        // Rule 1: category 1 keeps only safety shapes.
        assert!(!refuses(Category::Code, PatternKind::Always));
        assert!(!refuses(Category::Code, PatternKind::Never));
        assert!(refuses(Category::Code, PatternKind::Eventually));
        assert!(refuses(Category::Code, PatternKind::OccursAtMost));
        // Rule 2: can_reach only at the model.
        assert!(refuses(Category::Code, PatternKind::CanReach));
        assert!(!refuses(Category::Model, PatternKind::CanReach));
        assert!(refuses(Category::Runtime, PatternKind::CanReach));
        assert!(!refuses(Category::Runtime, PatternKind::Always));
    }
}
