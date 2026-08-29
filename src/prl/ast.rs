//! The PRL abstract syntax tree — the concrete shape a candidate formalization takes
//! once it clears the parser. This is the artifact the D12 read-back renderer and D13
//! grounding (later slices) consume; part 1 only produces and type/name-checks it.
//!
//! Deliberately shallow at the leaves: atom arguments, `within` durations, `with`
//! guards, `assume` entries, and `strength`/`evidence` bodies are kept as raw text.
//! Part 1 needs predicate name + arity, not a full term/relational grammar.
//! `// ponytail:` — those leaves get parsed when D13 grounding actually needs them.
//!
//! Implements: REQ016 (mechanical gate part 1 — parse + type/name-check).

/// A whole candidate requirement. `category` is empty when the author omitted it
/// (rule-based inference is a later slice — the gate does not guess here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    pub category: Vec<Category>,
    pub vocabulary: Vec<Decl>,
    /// Environment/fairness assumptions, kept as raw entries — a namespace distinct
    /// from the domain vocabulary, so they are parsed but not name-checked.
    pub assume: Vec<String>,
    pub require: Vec<Property>,
    pub strength: Option<String>,
    pub evidence: Option<String>,
}

/// Engine-routing category: `1`=code pre/post, `2a`=model, `2b`=runtime monitor, `3`=UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Code,
    Model,
    Runtime,
    Ui,
}

impl Category {
    /// The surface label the author writes (`1`, `2a`, `2b`, `3`) — so a message quotes
    /// the category as it appears in the source, not an internal variant name.
    pub fn as_label(&self) -> &'static str {
        match self {
            Category::Code => "1",
            Category::Model => "2a",
            Category::Runtime => "2b",
            Category::Ui => "3",
        }
    }
}

/// One `vocabulary` declaration. `Identity` is kept raw (`identity Message = m.id`);
/// only events and states contribute name-checkable predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    Sort {
        name: String,
        line: usize,
    },
    Event {
        name: String,
        params: Vec<Param>,
        line: usize,
    },
    State {
        name: String,
        params: Vec<Param>,
        line: usize,
    },
    Identity {
        raw: String,
        line: usize,
    },
}

/// A typed slot of an event/state declaration, e.g. `m: Message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: String,
}

/// One claim in the `require` block: an optional `each x: Sort` quantifier, a temporal
/// pattern, and a scope (defaults to `Globally`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    pub quantifier: Option<Quantifier>,
    pub pattern: Pattern,
    pub scope: Scope,
    pub line: usize,
}

/// `each <var>: <sort>` — first-order quantification over a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantifier {
    pub var: String,
    pub sort: String,
}

/// The specification patterns from the working set (Dwyer/Avrunin/Corbett lineage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Never(Expr),
    Always(Expr),
    Eventually(Expr),
    LeadsTo {
        from: Expr,
        to: Expr,
        /// Raw duration text (`30s`), unparsed for now.
        within: Option<String>,
    },
    Precedes {
        first: Expr,
        then: Expr,
    },
    OccursAtMost {
        event: Expr,
        k: u32,
    },
    CanReach(Expr),
}

/// Where a pattern applies (Dwyer scopes). Scope-boundary atoms are name-checked too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Globally,
    Before(Atom),
    After(Atom),
    Between(Atom, Atom),
}

/// A boolean combination of predicate applications inside a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Atom(Atom),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

/// A predicate/event application: a name, its argument terms (raw, counted for arity),
/// and an optional raw `with` guard. `line` anchors name/arity errors to source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Atom {
    pub name: String,
    pub args: Vec<String>,
    pub guard: Option<String>,
    pub line: usize,
}

/// One variable a claim ranges over, and the sort it ranges over (REQ059).
///
/// `sort` is `None` when the requirement does not say: the variable is applied where the
/// vocabulary declares no type for that parameter, or two applications give it different types.
/// Nothing is guessed — a consumer that needs a domain (lowering) refuses, and one that only
/// describes (the read-back) says the sort is undeclared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binder {
    pub var: String,
    pub sort: Option<String>,
    /// Whether the operator wrote this binder as an `each` quantifier, rather than it being closed
    /// over implicitly. The read-back needs the difference: a claim the operator did not write a
    /// quantifier for still ranges over its free variables, and D12 is only faithful if it says so.
    pub explicit: bool,
}

impl Requirement {
    /// Every variable one of this requirement's claims ranges over, in the order a reader meets
    /// them: the `each` binder first when there is one, then each free variable at its first
    /// application (REQ059).
    ///
    /// A category-1 invariant is a claim about **all** states, so a variable it applies a
    /// predicate to is universally quantified whether or not the operator wrote a binder for it.
    /// Requiring one binder per variable would mean restating, in every property, a sort the
    /// vocabulary already declares — and today only one binder can be written at all, so a
    /// predicate of arity > 1 could never be checked (#136).
    ///
    /// The sort comes from the **vocabulary declaration** (`state p(d: EngineStatus)`), which is
    /// where a predicate's signature belongs and where it is said once. An explicit `each` binder
    /// wins over it: the operator wrote that one deliberately.
    pub fn binders(&self, prop: &Property) -> Vec<Binder> {
        let mut binders: Vec<Binder> = prop
            .quantifier
            .iter()
            .map(|q| Binder {
                var: q.var.clone(),
                sort: Some(q.sort.clone()),
                explicit: true,
            })
            .collect();
        if !self.closes_over_free_variables(prop) {
            return binders;
        }
        prop.for_each_atom(&mut |atom| {
            let declared = self.declared_params(&atom.name);
            for (i, arg) in atom.args.iter().enumerate() {
                let var = arg.trim();
                if !is_variable(var) {
                    continue;
                }
                let sort = declared
                    .and_then(|params| params.get(i))
                    .map(|p| p.ty.trim())
                    .filter(|ty| !ty.is_empty())
                    .map(str::to_string);
                match binders.iter_mut().find(|b| b.var == var) {
                    // Already bound. An explicit binder is left alone; otherwise a second
                    // application that disagrees about the sort leaves it undeclared, because a
                    // variable cannot range over two domains and the requirement, not this
                    // function, has to settle which one was meant.
                    Some(seen) => {
                        if !seen.explicit && seen.sort != sort {
                            seen.sort = None;
                        }
                    }
                    None => binders.push(Binder {
                        var: var.to_string(),
                        sort,
                        explicit: false,
                    }),
                }
            }
        });
        binders
    }

    /// Whether this claim's free variables are universally quantified (REQ059). Two conditions,
    /// and both are load-bearing:
    ///
    /// - **The claim is an invariant** (`always`/`never`). In `accepted(m) leads_to
    ///   (dead_lettered(m, r) with r != "")`, the reason `r` is the one there *happens to be*, not
    ///   every reason there could be — closing over it would state a different requirement than
    ///   the operator wrote.
    /// - **The requirement is routed to the code fragment.** Closure is what the cat-1 harness
    ///   does; a model or monitor claim is lowered by another path that does no such thing, and a
    ///   read-back that showed a closure nobody performs would be exactly the D12 failure the
    ///   deterministic renderer exists to prevent. An undeclared category defaults to code, the
    ///   same rule [`crate::grounding::default_category`] applies.
    fn closes_over_free_variables(&self, prop: &Property) -> bool {
        let invariant = matches!(prop.pattern, Pattern::Always(_) | Pattern::Never(_));
        let code = self.category.is_empty() || self.category.contains(&Category::Code);
        invariant && code
    }

    /// The declared parameters of an event/state predicate, or `None` when the name is not one.
    fn declared_params(&self, name: &str) -> Option<&[Param]> {
        self.vocabulary.iter().find_map(|d| match d {
            Decl::Event {
                name: n, params, ..
            }
            | Decl::State {
                name: n, params, ..
            } if n == name => Some(params.as_slice()),
            _ => None,
        })
    }
}

/// Whether an argument term is a **variable** — something a claim can range over. Atom arguments
/// are raw text (the parser keeps the leaves shallow on purpose), so a term that is not a plain
/// identifier is a value or an expression, and closing over it would bind a name the harness
/// cannot declare. `true` and `false` are identifiers and are excluded for the same reason: they
/// are values, not variables.
fn is_variable(term: &str) -> bool {
    !matches!(term, "true" | "false")
        && !term.is_empty()
        && !term.starts_with(|c: char| c.is_ascii_digit())
        && term.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl Property {
    /// Visit every predicate application in this property — pattern operands and scope
    /// boundaries alike. The one walk both the gate's name/arity check and D13 grounding use,
    /// so neither can miss an application the other sees.
    pub fn for_each_atom(&self, f: &mut impl FnMut(&Atom)) {
        match &self.pattern {
            Pattern::Never(e)
            | Pattern::Always(e)
            | Pattern::Eventually(e)
            | Pattern::CanReach(e) => e.for_each_atom(f),
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
        match &self.scope {
            Scope::Globally => {}
            Scope::Before(a) | Scope::After(a) => f(a),
            Scope::Between(a, b) => {
                f(a);
                f(b);
            }
        }
    }
}

impl Expr {
    /// Visit every [`Atom`] in this expression tree (for name/arity checking).
    pub fn for_each_atom(&self, f: &mut impl FnMut(&Atom)) {
        match self {
            Expr::Atom(a) => f(a),
            Expr::Not(e) => e.for_each_atom(f),
            Expr::And(l, r) | Expr::Or(l, r) => {
                l.for_each_atom(f);
                r.for_each_atom(f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parser::parse;

    fn binders_of(src: &str) -> Vec<(String, Option<String>, bool)> {
        let req = parse(src).expect("should parse");
        req.binders(&req.require[0])
            .into_iter()
            .map(|b| (b.var, b.sort, b.explicit))
            .collect()
    }

    // Verifies: REQ059 — every variable an invariant applies a predicate to is quantified, over
    // the sort the vocabulary declares for that parameter, in the order a reader meets them. This
    // is what lets a predicate of arity > 1 be checked at all (#136): a property carries at most
    // one `each` binder, so three of these four variables could never have been supplied.
    #[test]
    fn free_variables_of_an_invariant_are_bound_by_their_declared_sorts() {
        assert_eq!(
            binders_of(
                "requirement r { category: 1
                 vocabulary { state proceeds(d: Decision, p: Flag, q: Flag, c: Flag) }
                 require { always proceeds(d, p, q, c) } }"
            ),
            vec![
                ("d".into(), Some("Decision".into()), false),
                ("p".into(), Some("Flag".into()), false),
                ("q".into(), Some("Flag".into()), false),
                ("c".into(), Some("Flag".into()), false),
            ]
        );
    }

    // Verifies: REQ059 — an explicit `each` comes first and keeps the sort the operator wrote,
    // even where the vocabulary declares another. They wrote that binder deliberately.
    #[test]
    fn an_explicit_binder_leads_and_keeps_its_own_sort() {
        assert_eq!(
            binders_of(
                "requirement r { category: 1
                 vocabulary { state p(u: Declared, v: Other) }
                 require { each u: Written . always p(u, v) } }"
            ),
            vec![
                ("u".into(), Some("Written".into()), true),
                ("v".into(), Some("Other".into()), false),
            ]
        );
    }

    // Verifies: REQ059 — the sort is `None`, never a guess, when the requirement does not say:
    // no declared type, or two applications that disagree about one. A consumer that needs a
    // domain refuses; nothing is invented for it.
    #[test]
    fn a_variable_the_requirement_does_not_type_has_no_sort() {
        assert_eq!(
            binders_of(
                "requirement r { category: 1
                 vocabulary { state p(u) }
                 require { always p(u) } }"
            ),
            vec![("u".into(), None, false)]
        );
        assert_eq!(
            binders_of(
                "requirement r { category: 1
                 vocabulary { state p(x: A) state q(x: B) }
                 require { always (p(u) and q(u)) } }"
            ),
            vec![("u".into(), None, false)]
        );
    }

    // Verifies: REQ059 — closure binds variables, not values. A literal argument is not something
    // a claim ranges over, and binding one would put `true` in a harness's binder list.
    #[test]
    fn literals_and_expressions_are_not_bound() {
        assert!(
            binders_of(
                "requirement r { category: 1
             vocabulary { state p(u: Flag) }
             require { always p(true) } }"
            )
            .is_empty()
        );
    }

    // Verifies: REQ059 — closure is the code fragment's invariant reading, and applies nowhere
    // else. In `accepted(m) leads_to dead_lettered(m, r)`, `r` is the reason there happens to be,
    // not every reason there could be; and a model requirement is lowered by a path that performs
    // no closure at all, so claiming one would misdescribe what the tool does.
    #[test]
    fn closure_applies_only_to_an_invariant_in_the_code_fragment() {
        let leads_to = binders_of(
            "requirement r {
             vocabulary { event accepted(m: Message) state dead_lettered(m: Message, why: Reason) }
             require { each m: Message . accepted(m) leads_to dead_lettered(m, r) } }",
        );
        assert_eq!(leads_to, vec![("m".into(), Some("Message".into()), true)]);

        let model = binders_of(
            "requirement r { category: 2a
             vocabulary { state p(u: Thing, v: Thing) }
             require { always p(u, v) } }",
        );
        assert!(
            model.is_empty(),
            "a model claim closes over nothing: {model:?}"
        );
    }
}
