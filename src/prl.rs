//! PRL — the Provable Requirement Language. This is the **mechanical gate** (D11):
//! it turns the untrusted LLM's candidate PRL text into a typed [`ast::Requirement`]
//! and rejects malformed or ill-typed candidates before any human reads them.
//!
//! Part 1 (this slice) is **parse → type/name-check**. Vacuity/triviality sanity and
//! the generate-then-repair loop are part 2; the D12 read-back renderer and D13
//! grounding are later slices that consume the AST this module produces.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

pub mod ast;
mod check;
pub mod error;
mod lexer;
mod parser;

pub use ast::Requirement;
pub use error::GateError;

/// Run the mechanical gate over a candidate PRL block: parse it, then type/name-check
/// the AST. Returns the checked [`Requirement`], or every gate error found. Parse
/// failures short-circuit — checking a half-built tree would only add noise.
pub fn gate(src: &str) -> Result<Requirement, Vec<GateError>> {
    let req = parser::parse(src)?;
    let errors = check::check(&req);
    if errors.is_empty() {
        Ok(req)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ016 — a well-formed, well-typed candidate clears the gate.
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
        let req = gate(src).expect("clean candidate should clear the gate");
        assert_eq!(req.name, "no_message_lost");
        assert_eq!(req.category.len(), 2);
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
