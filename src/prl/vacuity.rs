//! Vacuity / triviality sanity — the mechanical guard against "verifies but
//! meaningless". A **bounded, precise** set of structural anti-patterns, run only
//! after a candidate is well-formed and well-typed. Each check is exact (no fuzzy
//! heuristics): it names one concrete shape and flags exactly that shape.
//!
//! `// ponytail:` — this is *not* a satisfiability decision procedure. The immediate
//! tautology/contradiction checks catch the surface `P or not P` / `P and not P`
//! forms, not arbitrary unsatisfiable formulas; full SAT stays deferred (D11).
//!
//! Findings are [`GateWarning`]s, not errors — the requirement is valid, just
//! suspicious, so they flag it for the human gate (D12) rather than driving repair.
//!
//! Implements: REQ017 (gate part 2 — vacuity/triviality sanity).

use super::ast::*;
use super::error::GateWarning;
use std::collections::HashSet;

/// Collect every vacuity/triviality warning for a checked requirement.
pub fn warnings(req: &Requirement) -> Vec<GateWarning> {
    let mut out = Vec::new();

    for prop in &req.require {
        let line = prop.line;
        match &prop.pattern {
            Pattern::LeadsTo { from, to, .. } if expr_shape_eq(from, to) => {
                out.push(GateWarning::SelfLeadsTo { line })
            }
            Pattern::Precedes { first, then } if expr_shape_eq(first, then) => {
                out.push(GateWarning::SelfPrecedes { line })
            }
            Pattern::OccursAtMost { k: 0, .. } => out.push(GateWarning::OccursAtMostZero { line }),
            _ => {}
        }
        for expr in pattern_operands(&prop.pattern) {
            if is_immediate_tautology(expr) {
                out.push(GateWarning::ImmediateTautology { line });
            } else if is_immediate_contradiction(expr) {
                out.push(GateWarning::ImmediateContradiction { line });
            }
        }
    }

    out.extend(unused_vocabulary(req));
    out
}

/// The top-level operand expressions of a pattern (where a tautology/contradiction
/// would sit).
fn pattern_operands(p: &Pattern) -> Vec<&Expr> {
    match p {
        Pattern::Never(e) | Pattern::Always(e) | Pattern::Eventually(e) | Pattern::CanReach(e) => {
            vec![e]
        }
        Pattern::LeadsTo { from, to, .. } => vec![from, to],
        Pattern::Precedes { first, then } => vec![first, then],
        Pattern::OccursAtMost { event, .. } => vec![event],
    }
}

/// `P or not P` — an immediate tautology.
fn is_immediate_tautology(e: &Expr) -> bool {
    matches!(e, Expr::Or(l, r) if complementary(l, r))
}

/// `P and not P` — an immediate contradiction.
fn is_immediate_contradiction(e: &Expr) -> bool {
    matches!(e, Expr::And(l, r) if complementary(l, r))
}

/// Whether one operand is exactly the negation of the other.
fn complementary(l: &Expr, r: &Expr) -> bool {
    match (l, r) {
        (Expr::Not(a), b) | (b, Expr::Not(a)) => expr_shape_eq(a, b),
        _ => false,
    }
}

/// Structural equality of two expressions, ignoring source positions (the `line`
/// field on atoms differs even for identical predicates).
fn expr_shape_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Atom(x), Expr::Atom(y)) => {
            x.name == y.name && x.args == y.args && x.guard == y.guard
        }
        (Expr::Not(x), Expr::Not(y)) => expr_shape_eq(x, y),
        (Expr::And(x1, x2), Expr::And(y1, y2)) | (Expr::Or(x1, x2), Expr::Or(y1, y2)) => {
            expr_shape_eq(x1, y1) && expr_shape_eq(x2, y2)
        }
        _ => false,
    }
}

/// Predicates declared in `vocabulary` but never applied anywhere in `require`.
fn unused_vocabulary(req: &Requirement) -> Vec<GateWarning> {
    let mut used: HashSet<String> = HashSet::new();
    for prop in &req.require {
        collect_used(prop, &mut used);
    }
    req.vocabulary
        .iter()
        .filter_map(|decl| match decl {
            Decl::Event { name, line, .. } | Decl::State { name, line, .. }
                if !used.contains(name) =>
            {
                Some(GateWarning::UnusedVocabulary {
                    name: name.clone(),
                    line: *line,
                })
            }
            _ => None,
        })
        .collect()
}

/// Record every predicate name applied in a property (pattern operands + scope).
fn collect_used(prop: &Property, used: &mut HashSet<String>) {
    for expr in pattern_operands(&prop.pattern) {
        expr.for_each_atom(&mut |a| {
            used.insert(a.name.clone());
        });
    }
    match &prop.scope {
        Scope::Globally => {}
        Scope::Before(a) | Scope::After(a) => {
            used.insert(a.name.clone());
        }
        Scope::Between(a, b) => {
            used.insert(a.name.clone());
            used.insert(b.name.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn warns(src: &str) -> Vec<GateWarning> {
        warnings(&parse(src).expect("should parse"))
    }

    #[test]
    fn self_leads_to_is_flagged() {
        let src = "requirement r {
            vocabulary { state p(x) }
            require { each m: X . p(m) leads_to p(m) }
        }";
        assert!(warns(src)
            .iter()
            .any(|w| matches!(w, GateWarning::SelfLeadsTo { .. })));
    }

    #[test]
    fn self_precedes_is_flagged() {
        let src = "requirement r {
            vocabulary { state p(x) }
            require { p(a) precedes p(a) }
        }";
        assert!(warns(src)
            .iter()
            .any(|w| matches!(w, GateWarning::SelfPrecedes { .. })));
    }

    #[test]
    fn immediate_tautology_is_flagged() {
        let src = "requirement r {
            vocabulary { state p(x) }
            require { always (p(m) or not p(m)) }
        }";
        assert!(warns(src)
            .iter()
            .any(|w| matches!(w, GateWarning::ImmediateTautology { .. })));
    }

    #[test]
    fn immediate_contradiction_is_flagged() {
        let src = "requirement r {
            vocabulary { state p(x) }
            require { eventually (p(m) and not p(m)) }
        }";
        assert!(warns(src)
            .iter()
            .any(|w| matches!(w, GateWarning::ImmediateContradiction { .. })));
    }

    #[test]
    fn occurs_at_most_zero_is_flagged() {
        let src = "requirement r {
            vocabulary { event retry(x) }
            require { retry(m) occurs at most 0 times }
        }";
        assert!(warns(src)
            .iter()
            .any(|w| matches!(w, GateWarning::OccursAtMostZero { .. })));
    }

    #[test]
    fn unused_vocabulary_is_flagged() {
        let src = "requirement r {
            vocabulary { state used(x) state orphan(x) }
            require { always used(m) }
        }";
        let ws = warns(src);
        assert!(ws
            .iter()
            .any(|w| matches!(w, GateWarning::UnusedVocabulary { name, .. } if name == "orphan")));
        // The used predicate is not flagged.
        assert!(!ws
            .iter()
            .any(|w| matches!(w, GateWarning::UnusedVocabulary { name, .. } if name == "used")));
    }

    #[test]
    fn scope_boundary_predicate_counts_as_used() {
        let src = "requirement r {
            vocabulary { state p(x) state boundary(x) }
            require { always p(m) after boundary(x) }
        }";
        assert!(!warns(src).iter().any(
            |w| matches!(w, GateWarning::UnusedVocabulary { name, .. } if name == "boundary")
        ));
    }

    #[test]
    fn a_clean_requirement_has_no_warnings() {
        let src = "requirement r {
            vocabulary { event accepted(m: Message) state done(m) }
            require { each m: Message . accepted(m) leads_to done(m) }
        }";
        assert!(warns(src).is_empty());
    }
}
