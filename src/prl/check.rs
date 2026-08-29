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
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check), REQ066 (a bare name
//! the claim ranges over is that variable, not an undeclared predicate).

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

    // Every predicate applied in `require` must resolve to a declared event/state — unless it is
    // not a predicate application at all. A bare name that is a variable the claim already ranges
    // over is that variable used as a condition (REQ066): `always (not proceeds(d, p, q, c) or p)`
    // says the claim holds whenever `p` is true. Only a name the vocabulary does NOT declare is
    // read this way, so a declared predicate always wins and nothing that used to gate stops
    // gating. Whether the variable's sort is actually boolean is not knowable here — the gate
    // resolves names and arities, not types — so it is enforced at lowering, against the subject.
    for prop in &req.require {
        let bound: Vec<String> = req.binders(prop).into_iter().map(|b| b.var).collect();
        prop.for_each_atom(&mut |atom| match arity.get(atom.name.as_str()) {
            None if atom.args.is_empty() && bound.iter().any(|v| v == &atom.name) => {}
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
        assert!(
            errs.iter().any(
                |e| matches!(e, GateError::UndeclaredPredicate { name, .. } if name == "gone")
            )
        );
    }

    // Verifies: REQ066 — a bare name that is a variable the claim ranges over is that variable
    // used as a condition, not an undeclared predicate. This is REQ047's shape: `p` is the second
    // argument of `install_proceeds`, so the claim already binds it.
    #[test]
    fn a_variable_used_as_a_condition_is_not_an_undeclared_predicate() {
        let src = "requirement r { category: 1
            vocabulary { state install_proceeds(d: EngineState, p: Flag) }
            require { always (not install_proceeds(d, p) or p) }
        }";
        assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
    }

    // Verifies: REQ066 — the reading is narrow. A bare name the claim does NOT bind is still an
    // undeclared predicate, so a misspelling cannot slip through as a variable.
    #[test]
    fn a_bare_name_the_claim_does_not_bind_is_still_undeclared() {
        let src = "requirement r { category: 1
            vocabulary { state install_proceeds(d: EngineState, p: Flag) }
            require { always (not install_proceeds(d, p) or q) }
        }";
        assert!(
            errors_of(src)
                .iter()
                .any(|e| matches!(e, GateError::UndeclaredPredicate { name, .. } if name == "q"))
        );
    }

    // Verifies: REQ066 — a declared predicate wins. A nullary state of that name is still a
    // predicate application, so nothing that gated before stops gating.
    #[test]
    fn a_declared_nullary_predicate_is_still_a_predicate() {
        let src = "requirement r { category: 1
            vocabulary { state supported
                         state install_proceeds(d: EngineState, supported: Flag) }
            require { always (not install_proceeds(d, supported) or supported) }
        }";
        assert!(errors_of(src).is_empty(), "{:?}", errors_of(src));
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
        assert!(
            errs.iter()
                .any(|e| matches!(e, GateError::DuplicateDecl { name, .. } if name == "done"))
        );
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
