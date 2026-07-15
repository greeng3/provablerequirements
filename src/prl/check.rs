//! Type/name-check over a parsed [`Requirement`]. Part 1 of the gate checks exactly
//! what it can honestly enforce without a full term/sort system: required sections
//! present, category values well-formed (already caught in parse), no duplicate
//! vocabulary declarations, and every predicate used in `require` is declared with
//! matching arity. Full variable-binding and sort-checking wait for D13 grounding.
//! `// ponytail:` — this is name+arity resolution, not a type system, by design.
//!
//! `assume` entries are a distinct environment namespace, so they are not name-checked
//! against the domain vocabulary.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

use super::ast::*;
use super::error::GateError;
use std::collections::HashMap;

/// Check a parsed requirement, returning every type/name error found (empty = clean).
pub fn check(req: &Requirement) -> Vec<GateError> {
    let mut errors = Vec::new();

    if req.require.is_empty() {
        errors.push(GateError::EmptyRequire);
    }

    // Declared predicate names → arity, flagging any duplicate declaration.
    let mut arity: HashMap<&str, usize> = HashMap::new();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for decl in &req.vocabulary {
        let (name, params, line) = match decl {
            Decl::Sort { name, line } => (name.as_str(), None, *line),
            Decl::Event { name, params, line } | Decl::State { name, params, line } => {
                (name.as_str(), Some(params.len()), *line)
            }
            Decl::Identity { .. } => continue,
        };
        if seen.insert(name, line).is_some() {
            errors.push(GateError::DuplicateDecl {
                name: name.to_string(),
                line,
            });
        }
        if let Some(n) = params {
            arity.insert(name, n);
        }
    }

    // Every predicate applied in `require` must resolve to a declared event/state.
    for prop in &req.require {
        visit_property_atoms(prop, &mut |atom| match arity.get(atom.name.as_str()) {
            None => errors.push(GateError::UndeclaredPredicate {
                name: atom.name.clone(),
                line: atom.line,
            }),
            Some(&expected) if expected != atom.args.len() => {
                errors.push(GateError::ArityMismatch {
                    name: atom.name.clone(),
                    expected,
                    found: atom.args.len(),
                    line: atom.line,
                })
            }
            Some(_) => {}
        });
    }

    errors
}

/// Visit every predicate application in a property — pattern operands and scope
/// boundaries alike.
fn visit_property_atoms(prop: &Property, f: &mut impl FnMut(&Atom)) {
    match &prop.pattern {
        Pattern::Never(e) | Pattern::Always(e) | Pattern::Eventually(e) | Pattern::CanReach(e) => {
            e.for_each_atom(f)
        }
        Pattern::LeadsTo { from, to, .. } => {
            from.for_each_atom(f);
            to.for_each_atom(f);
        }
        Pattern::Precedes { first, then } => {
            first.for_each_atom(f);
            then.for_each_atom(f);
        }
        Pattern::OccursAtMost { event, .. } => event.for_each_atom(f),
    }
    match &prop.scope {
        Scope::Globally => {}
        Scope::Before(a) | Scope::After(a) => f(a),
        Scope::Between(a, b) => {
            f(a);
            f(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn errors_of(src: &str) -> Vec<GateError> {
        check(&parse(src).expect("should parse"))
    }

    #[test]
    fn clean_requirement_has_no_check_errors() {
        let src = "requirement r {
            vocabulary { event accepted(m: Message) state done(m) }
            require { each m: Message . accepted(m) leads_to done(m) }
        }";
        assert!(errors_of(src).is_empty());
    }

    #[test]
    fn undeclared_predicate_is_flagged() {
        let src = "requirement r {
            vocabulary { event accepted(m: Message) }
            require { accepted leads_to gone }
        }";
        // `accepted` is arity-1 declared but used with 0 args → arity mismatch;
        // `gone` is undeclared.
        let errs = errors_of(src);
        assert!(errs
            .iter()
            .any(|e| matches!(e, GateError::UndeclaredPredicate { name, .. } if name == "gone")));
    }

    #[test]
    fn arity_mismatch_is_flagged() {
        let src = "requirement r {
            vocabulary { event accepted(m: Message) state done(m) }
            require { each m: Message . accepted(m, extra) leads_to done(m) }
        }";
        let errs = errors_of(src);
        assert!(errs.iter().any(|e| matches!(
            e,
            GateError::ArityMismatch { name, expected: 1, found: 2, .. } if name == "accepted"
        )));
    }

    #[test]
    fn duplicate_declaration_is_flagged() {
        let src = "requirement r {
            vocabulary { state done(m) state done(x) }
            require { done leads_to done }
        }";
        let errs = errors_of(src);
        assert!(errs
            .iter()
            .any(|e| matches!(e, GateError::DuplicateDecl { name, .. } if name == "done")));
    }

    #[test]
    fn empty_require_is_flagged() {
        let errs = errors_of("requirement r { require { } }");
        assert!(errs.iter().any(|e| matches!(e, GateError::EmptyRequire)));
    }

    #[test]
    fn scope_boundary_predicates_are_checked() {
        let src = "requirement r {
            vocabulary { state p(x) state a(x) }
            require { always p after missing_event }
        }";
        let errs = errors_of(src);
        assert!(errs.iter().any(
            |e| matches!(e, GateError::UndeclaredPredicate { name, .. } if name == "missing_event")
        ));
    }
}
