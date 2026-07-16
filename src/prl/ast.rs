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
