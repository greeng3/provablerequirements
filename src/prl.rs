//! PRL — the Provable Requirement Language. This is the **mechanical gate** (D11):
//! it turns the untrusted LLM's candidate PRL text into a typed [`ast::Requirement`]
//! and rejects malformed or ill-typed candidates before any human reads them.
//!
//! The gate has two verdicts. Malformed or ill-typed candidates are **rejected** with
//! [`GateError`]s (which drive the generate-then-repair loop in [`crate::formalize`]).
//! A well-formed, well-typed candidate is **accepted** as a [`GateOutcome`], possibly
//! carrying vacuity/triviality [`GateWarning`]s — it is valid but suspicious, and the
//! warnings flag it for the human gate (D12) rather than blocking it.
//!
//! The D12 read-back renderer and D13 grounding are later slices that consume the AST.
//!
//! Implements: REQ016 (parse + type/name-check), REQ017 (vacuity + accept-with-warnings).

pub mod ast;
mod check;
pub mod error;
mod lexer;
mod parser;
mod readback;
mod vacuity;

pub use ast::Requirement;
pub use error::{GateError, GateWarning};
pub use readback::render;

/// A candidate that cleared the gate: the checked AST plus any vacuity/triviality
/// warnings (empty = clean). The warnings do not block admission — they are surfaced
/// to the human reviewer (D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateOutcome {
    pub requirement: Requirement,
    pub warnings: Vec<GateWarning>,
}

/// Run the mechanical gate over a candidate PRL block: parse it, type/name-check the
/// AST, then run vacuity/triviality sanity. Returns the accepted [`GateOutcome`], or
/// every gate error found. Parse failures short-circuit the checker, and vacuity runs
/// only on a clean type-check — vacuity findings on an ill-typed tree would be noise.
pub fn gate(src: &str) -> Result<GateOutcome, Vec<GateError>> {
    let requirement = parser::parse(src)?;
    let errors = check::check(&requirement);
    if !errors.is_empty() {
        return Err(errors);
    }
    let warnings = vacuity::warnings(&requirement);
    Ok(GateOutcome {
        requirement,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ016/REQ017 — a well-formed, well-typed candidate clears the gate
    // with no vacuity warnings.
    #[test]
    fn gate_accepts_a_clean_candidate() {
        let src = "requirement no_message_lost {
            category: 2a + 2b
            vocabulary {
                event accepted(m: Message)
                state succeeded(m), dead_lettered(m: Message, reason: String)
            }
            assume { retries_bounded(N = 5) }
            require {
                each m: Message .
                    accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != \"\") within 30s
            }
            strength: model_checked over Model, monitored(deadline = 30s)
            evidence: tla+ (bounded: |Message| <= 8), monpoly(stream = queue.events)
        }";
        let outcome = gate(src).expect("clean candidate should clear the gate");
        assert_eq!(outcome.requirement.name, "no_message_lost");
        assert_eq!(outcome.requirement.category.len(), 2);
        assert!(
            outcome.warnings.is_empty(),
            "clean candidate should not warn"
        );
    }

    // Verifies: REQ017 — a valid but vacuous candidate is accepted (not rejected) and
    // carries a warning for the human gate.
    #[test]
    fn gate_accepts_but_warns_on_vacuity() {
        let src = "requirement r {
            vocabulary { state p(x) }
            require { each m: X . p(m) leads_to p(m) }
        }";
        let outcome = gate(src).expect("a vacuous-but-valid candidate still clears the gate");
        assert!(outcome
            .warnings
            .iter()
            .any(|w| matches!(w, GateWarning::SelfLeadsTo { .. })));
    }

    // Verifies: REQ016 — a candidate using an undeclared predicate is rejected, and
    // the error carries a source line for the repair loop to quote.
    #[test]
    fn gate_rejects_undeclared_predicate_with_a_line() {
        let src = "requirement r {
            vocabulary { event accepted(m: Message) }
            require { each m: Message . accepted(m) leads_to vanished(m) }
        }";
        let errs = gate(src).unwrap_err();
        assert!(errs.iter().any(|e| matches!(
            e,
            GateError::UndeclaredPredicate { name, line } if name == "vanished" && *line >= 1
        )));
        // Errors render with their line for repair feedback.
        assert!(errs.iter().any(|e| e.to_string().contains("vanished")));
    }

    // Verifies: REQ016 — malformed input is rejected at parse and never reaches the
    // checker (no spurious type errors piled on a broken tree).
    #[test]
    fn gate_rejects_malformed_input_at_parse() {
        let errs = gate("this is not prl").unwrap_err();
        assert!(errs.iter().all(|e| matches!(
            e,
            GateError::MissingSection { .. } | GateError::Parse { .. }
        )));
    }
}
