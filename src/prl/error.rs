//! Structured gate errors. Every rejection carries a best-effort source line so the
//! later generate-then-repair loop (part 2) can quote precise, actionable feedback
//! back to the LLM. One flat enum: parse failures and type/name-check failures alike.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

use std::fmt;

/// A single reason a candidate PRL was rejected by the gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// The grammar did not accept the input at `line`.
    Parse { message: String, line: usize },
    /// A required section (`require`, or the `requirement <name>` header) is absent.
    MissingSection { section: &'static str },
    /// `require { }` parsed but holds no property — a requirement that claims nothing.
    EmptyRequire,
    /// A `category:` token was not one of `1 | 2a | 2b | 3`.
    BadCategory { value: String, line: usize },
    /// The same event/state/sort name was declared twice in `vocabulary`.
    DuplicateDecl { name: String, line: usize },
    /// A predicate used in `require` was never declared in `vocabulary`.
    UndeclaredPredicate { name: String, line: usize },
    /// A predicate was applied with the wrong number of arguments.
    ArityMismatch {
        name: String,
        expected: usize,
        found: usize,
        line: usize,
    },
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GateError::Parse { message, line } => write!(f, "line {line}: {message}"),
            GateError::MissingSection { section } => {
                write!(f, "missing required `{section}`")
            }
            GateError::EmptyRequire => write!(f, "`require` block is empty — it claims nothing"),
            GateError::BadCategory { value, line } => {
                write!(
                    f,
                    "line {line}: category `{value}` is not one of 1, 2a, 2b, 3"
                )
            }
            GateError::DuplicateDecl { name, line } => {
                write!(
                    f,
                    "line {line}: `{name}` is declared more than once in vocabulary"
                )
            }
            GateError::UndeclaredPredicate { name, line } => write!(
                f,
                "line {line}: `{name}` is used in require but not declared in vocabulary"
            ),
            GateError::ArityMismatch {
                name,
                expected,
                found,
                line,
            } => write!(
                f,
                "line {line}: `{name}` takes {expected} argument(s) but was given {found}"
            ),
        }
    }
}

impl std::error::Error for GateError {}
