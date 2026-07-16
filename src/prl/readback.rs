//! D12 read-back: a **deterministic** renderer from the PRL AST to readable CNL. This
//! is a pure pretty-printer — **never an LLM call**. That independence is the whole
//! point (D12): the forward translation is the untrusted LLM, so if the same model
//! also rendered the read-back it would faithfully restate its own misreading and the
//! human would rubber-stamp a spec gap. A trusted renderer surfaces the *actual formal
//! meaning* of what the gate accepted, so the human confirms intent against the real
//! artifact.
//!
//! Faithfulness over prose polish: boolean operands are parenthesized whenever
//! compound (never changing meaning), predicate applications and raw leaves
//! (durations, `with` guards, assume/strength/evidence) are shown as-is.
//!
//! Implements: REQ018 (D12 deterministic AST→CNL read-back renderer).

use super::ast::*;

/// Render a checked requirement to a canonical CNL read-back for human confirmation.
pub fn render(req: &Requirement) -> String {
    let mut lines = Vec::new();

    let category = if req.category.is_empty() {
        "unspecified".to_string()
    } else {
        req.category
            .iter()
            .map(category_word)
            .collect::<Vec<_>>()
            .join(" + ")
    };
    lines.push(format!(
        "Requirement `{}` — category: {category}.",
        req.name
    ));

    if !req.assume.is_empty() {
        lines.push(format!("Assuming {}.", req.assume.join("; ")));
    }

    lines.push("It requires that:".to_string());
    for prop in &req.require {
        lines.push(format!("  • {}", render_property(prop)));
    }

    if let Some(strength) = &req.strength {
        lines.push(format!("Expected verdict: {strength}."));
    }
    if let Some(evidence) = &req.evidence {
        lines.push(format!("Checked by: {evidence}."));
    }

    lines.join("\n")
}

fn category_word(c: &Category) -> String {
    match c {
        Category::Code => "code (1)",
        Category::Model => "model (2a)",
        Category::Runtime => "runtime monitor (2b)",
        Category::Ui => "UI (3)",
    }
    .to_string()
}

fn render_property(p: &Property) -> String {
    let claim = format!("{}{}", render_pattern(&p.pattern), render_scope(&p.scope));
    match &p.quantifier {
        Some(q) => format!("for each {} of type {}, {claim}", q.var, q.sort),
        None => claim,
    }
}

fn render_pattern(p: &Pattern) -> String {
    match p {
        // Pattern operands use `parenthesized` so a compound operand is bracketed and
        // never runs ambiguously into the surrounding "… always holds" phrasing.
        Pattern::Never(e) => format!("{} never holds", parenthesized(e)),
        Pattern::Always(e) => format!("{} always holds", parenthesized(e)),
        Pattern::Eventually(e) => format!("eventually {} holds", parenthesized(e)),
        Pattern::LeadsTo { from, to, within } => {
            let base = format!(
                "once {} holds, {} eventually holds",
                parenthesized(from),
                parenthesized(to)
            );
            match within {
                Some(t) => format!("{base} within {t}"),
                None => base,
            }
        }
        Pattern::Precedes { first, then } => format!(
            "every {} is preceded by {}",
            parenthesized(then),
            parenthesized(first)
        ),
        Pattern::OccursAtMost { event, k } => format!(
            "{} occurs at most {k} time{}",
            parenthesized(event),
            if *k == 1 { "" } else { "s" }
        ),
        Pattern::CanReach(e) => {
            format!("a state where {} holds is reachable", parenthesized(e))
        }
    }
}

fn render_scope(s: &Scope) -> String {
    match s {
        Scope::Globally => String::new(),
        Scope::Before(a) => format!(", before {}", render_atom(a)),
        Scope::After(a) => format!(", after {}", render_atom(a)),
        Scope::Between(a, b) => format!(", between {} and {}", render_atom(a), render_atom(b)),
    }
}

fn render_expr(e: &Expr) -> String {
    match e {
        Expr::Atom(a) => render_atom(a),
        Expr::Not(inner) => format!("not {}", parenthesized(inner)),
        Expr::And(l, r) => format!("{} and {}", parenthesized(l), parenthesized(r)),
        Expr::Or(l, r) => format!("{} or {}", parenthesized(l), parenthesized(r)),
    }
}

/// Render an operand, wrapping it in parentheses when it is compound. Atoms need no
/// parens; anything with a connective does, so the read-back is never ambiguous about
/// grouping.
fn parenthesized(e: &Expr) -> String {
    match e {
        Expr::Atom(a) => render_atom(a),
        _ => format!("({})", render_expr(e)),
    }
}

fn render_atom(a: &Atom) -> String {
    let base = if a.args.is_empty() {
        a.name.clone()
    } else {
        format!("{}({})", a.name, a.args.join(", "))
    };
    match &a.guard {
        Some(g) => format!("{base} where {g}"),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;
    use super::*;

    fn readback(src: &str) -> String {
        render(&parse(src).expect("should parse"))
    }

    #[test]
    fn renders_header_with_category_words() {
        let out = readback("requirement r { category: 2a + 2b\n require { always ok } }");
        assert!(out.contains("Requirement `r`"));
        assert!(out.contains("category: model (2a) + runtime monitor (2b)"));
    }

    #[test]
    fn renders_the_worked_leads_to_example_faithfully() {
        let src = "requirement no_message_lost {
            vocabulary {
                event accepted(m: Message)
                state succeeded(m), dead_lettered(m: Message, reason: String)
            }
            assume { retries_bounded(N = 5) }
            require {
                each m: Message .
                    accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != \"\") within 30s
            }
        }";
        let out = readback(src);
        assert!(out.contains("Assuming retries_bounded(N = 5)."));
        assert!(out.contains("for each m of type Message,"));
        assert!(out.contains(
            "once accepted(m) holds, (succeeded(m) or dead_lettered(m, r) where r != \"\") eventually holds within 30s"
        ));
    }

    #[test]
    fn renders_never_always_eventually() {
        assert!(readback("requirement r { require { never boom } }").contains("boom never holds"));
        assert!(readback("requirement r { require { always ok } }").contains("ok always holds"));
        assert!(readback("requirement r { require { eventually done } }")
            .contains("eventually done holds"));
    }

    #[test]
    fn renders_precedes_in_the_faithful_direction() {
        // `S precedes P` means every P is preceded by an S.
        let out = readback("requirement r { require { grant precedes use } }");
        assert!(out.contains("every use is preceded by grant"), "got: {out}");
    }

    #[test]
    fn renders_scopes() {
        let out = readback("requirement r { require { always p between open and close } }");
        assert!(
            out.contains("p always holds, between open and close"),
            "got: {out}"
        );
    }

    #[test]
    fn renders_occurs_at_most_with_pluralization() {
        assert!(
            readback("requirement r { require { retry occurs at most 1 times } }")
                .contains("retry occurs at most 1 time")
        );
        assert!(
            readback("requirement r { require { retry occurs at most 5 times } }")
                .contains("retry occurs at most 5 times")
        );
    }

    #[test]
    fn renders_can_reach() {
        assert!(readback("requirement r { require { can_reach shutdown } }")
            .contains("a state where shutdown holds is reachable"));
    }

    #[test]
    fn parenthesizes_compound_boolean_operands() {
        let out = readback("requirement r { require { always (a or (b and c)) } }");
        // The nested `and` is parenthesized; the outer `or` operands too.
        assert!(out.contains("(a or (b and c)) always holds"), "got: {out}");
    }

    #[test]
    fn renders_strength_and_evidence_footers() {
        let src = "requirement r {
            require { always ok }
            strength: model_checked over Model
            evidence: tla+ (bounded: |M| <= 8)
        }";
        let out = readback(src);
        assert!(out.contains("Expected verdict: model_checked over Model."));
        assert!(out.contains("Checked by: tla+ (bounded: |M| <= 8)."));
    }
}
