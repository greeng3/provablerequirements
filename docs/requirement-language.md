# Requirement Language (PRL) — Design

Design of the unified requirement language that sits at the core of the project (the
trusted requirement layer of the layered-hybrid architecture). Working name: **PRL**
(Provable Requirement Language). Tracked in issue #2; background and architecture in the
top-level [README](../README.md).

Status: **in design.** This document records decisions as they are made; open items are
listed at the end.

## Guiding principle

The DSL mirrors the layered-hybrid architecture ("unify the meaning, federate the
engines"): one stable core *meaning*, with lowering to each engine. Three layers:

- **Surface layer** (human- & LLM-facing) — readable, pattern-based authoring.
- **Core layer** (formal semantics) — one precise logic that every requirement means.
- **Grounding layer** (per-category adapters) — binds abstract predicates/events to real
  observables (code state, model variables, runtime trace events, UI probes).

One meaning of a requirement; grounding + modality decide which engine runs it.

## Anatomy of a requirement

Every requirement, regardless of category, shares one skeleton:

```text
requirement <name> {
  category:   1 | 2a | 2b | 3          // routes to engine (declared, or inferred — see D3)
  vocabulary { … }                     // predicates/events it speaks about (grounding)
  assume     { … }                     // fairness, environment, delivery semantics
  require    { … }                     // the claim itself (pattern-based temporal)
  strength:  <expected verdict>        // proven∀ | model-checked∀ | monitored | …
  evidence:  <engine + params>         // bound, N runs, deadline, fairness
}
```

This discharges the accumulated constraints: `category` + `strength` are first-class;
`assume` carries fairness / delivery semantics; `vocabulary` is the atomic
operations/events notion; `require` is where temporal logic lives.

## Decisions

### D1 — Surface style: patterns primary, sugar as escape hatch, CNL as read-back

Priority: **precision over ambiguity, without writing raw logical notation.** Ranking:

1. **Specification patterns** (Dwyer/Avrunin/Corbett) — *chosen default.* Each pattern
   (`P leads_to Q within T`, `S precedes P`, `never P between A and B`) is a named
   template with one fixed formal meaning; precision is by construction (ambiguity is
   resolved when the pattern is chosen, not left in prose), yet no `□`/`◇`/`E`/`A` is
   ever written. Also the best LLM target: pick a pattern, fill typed slots.
2. **Logic-with-sugar** (`always(P implies eventually Q)`) — maximally precise but still
   "writing logic," so it fails the not-raw preference. Kept as an **expert escape
   hatch** for the rare property no pattern covers.
3. **Controlled natural language (CNL)** — most readable, but precision degrades at
   nesting/composition/quantifiers. Kept as a **read-back / rendering** layer, not an
   authoring layer.

Rejected as primary: raw temporal logic (precise but unwanted); free natural language
(that is the untrusted LLM front-end, not the precise surface).

**Round-trip requirement:** whatever the surface, the tool renders the chosen requirement
back in unambiguous readable form (CNL read-back) so the author confirms intent before
checking — the "review the statement, not the proof" principle applied to authoring.

### D2 — One unified core, structured as fragments/profiles

Chosen: **one core semantics**, organized as a family of delimited fragments, each a
subset of the one meaning and each exactly what one engine checks. (Not federated cores
glued only by shared vocabulary.)

Why unify: a requirement means one thing; a cross-category property is provably the *same*
claim everywhere; cross-category composition and Event-B-style refinement need one logic;
one LLM target; uniform verdicts; one spec-gap review.

What unifying costs (accepted): real upfront work to define the core semantics (a logic
that is first-order, metric, branching, and linear is largely undecidable) and its
checkable fragments; some engine-native power that does not map cleanly; and the core is
added on top of — not instead of — the N engine back-ends.

How the fragment structure controls that cost:

- One semantic domain → single meaning preserved.
- Profiles: *code fragment* (temporal-free Hoare) → Viper; *linear monitorable fragment*
  (MFOTL) → MonPoly; *branching fragment* (CTL) → NuSMV; etc.
- A requirement's fragment is declared/inferred; using an operator outside the target
  engine's fragment is a **typed error surfaced to the author**, never a silent
  approximation.
- An **engine-native raw block** is allowed as a clearly-marked escape hatch for the rare
  property that does not fit the core — flagged as *not covered by the unified
  semantics*, preserving honesty.

Analogy: SQL / a typed IR — one semantics, target-specific lowerings, engines support
subsets.

#### D2a — Differential cross-verification (validation strategy for D2)

Optionally verify a requirement **both** ways — via the unified core's lowering **and** via
an engine's native encoding — and compare verdicts. This is *differential
cross-verification* (a cousin of N-version programming / compiler differential testing),
and it is best understood as a **validation strategy for the D2 bet**, not a rival to it.

Separate two things it could check — they differ sharply in cost/benefit:

- **(a) the lowering** — "did core → engine produce the right formula?" (most of the value)
- **(b) the specification** — "does the formula mean the intent?" (only works if the two
  encodings are *genuinely independent*)

Benefits: catches lowering bugs (differential testing of the compiler); corroboration
across engines with *different* TCBs; the native engine acts as a fidelity oracle for where
the core fragment only approximates; **de-risks D2** during the period the core is not yet
trusted (dial back to spot-checks once it is); divergent counterexamples localize the fault.

Costs (beyond labor): the **oracle/reconciliation problem** (which is right on
disagreement? — and disagreement is not always a bug: logics legitimately differ, e.g. CTL
vs LTL reading of a pattern); the **semantic-alignment burden** (same claim / assumptions /
bounds / grounding, or comparisons are meaningless); **correlated errors → false
confidence** (agreement is only as good as independence; shared upstream defeats the
redundancy); **verdict-model complexity** (must represent two results + a divergence
policy); **partial coverage** (not every requirement has a native counterpart — a
spot-check, not a universal guarantee); **desync drift** between the two encodings.

Decision: use it as a **development-time test oracle** (validate the core's lowering against
a corpus of known-good native encodings) and an **opt-in high-assurance mode** for critical
requirements — not a permanent universal double-run. Prefer **translation validation** of
the lowering where feasible (rigorous version of (a); sidesteps reconciliation). Divergence
policy is conservative: **divergence ⇒ unknown / needs-review**, never silently pick a
winner. Bake two caveats into the verdict model: agreement raises confidence but never
*proves* correctness (correlated errors); disagreement is not always a bug (logic
differences).

#### D2b — Per-language tool ensembles and soundness-aware verdicts

Beyond core-vs-native, several languages already have **multiple native tools** with
*different formalisms*, so an ensemble is cheap-ish to assemble:

- **C** (richest): Frama-C (WP deductive + EVA abstract-interp), VeriFast (sep. logic),
  CBMC/ESBMC (bounded MC), CPAchecker, Ultimate, SeaHorn, Astrée (abstract-interp), Infer,
  Klee (symbolic). **Java**: KeY / OpenJML (both JML), VerCors, VeriFast, JBMC, Java
  Pathfinder. **Rust**: Prusti, Verus, Creusot, Kani (bounded), Aeneas, Flux, Miri.
  **Python**: Nagini, CrossHair. **Go**: Gobra, Goose, race detector.
- **Pre-built cross-check infrastructure exists:** **SV-COMP** runs ~40+ C verifiers on
  shared benchmarks and standardized **verification witnesses** (violation / correctness)
  that a *different* tool can validate — exactly the machinery for locating disagreement.

**The gradient (determines free-upside vs reconciliation cost):**

- **Solver portfolio** (Why3/SPARK/Dafny → many SMT solvers): *pure upside, no
  reconciliation* — a discharged obligation is a proof regardless of which solver closed
  it. Always do this.
- **Same-formalism tools:** mostly upside; disagreement usually means one is unsound/buggy
  or uses a different memory model.
- **Different-formalism tools:** disagreement is **expected and informative, not a
  contradiction** — they answer subtly different questions.

**Interpret every verdict by its soundness direction (the key lens):**

- **Over-approximating & sound** (abstract interpretation): *PASS trustworthy; FAIL may be
  a false alarm.*
- **Under-approximating & bug-finding** (bounded MC, symbolic, concolic, dynamic): *FAIL is
  a real bug; PASS only means "no bug within the bound."*
- **Deductive & spec-relative** (WP, KeY, Verus): *PASS trustworthy relative to spec +
  assumptions; may return unknown.*

So a **majority vote is wrong** — "3 PASS, 1 FAIL" is meaningless if the FAIL is a sound
over-approximator reaching what the under-approximators could not. The ensemble's value is
as an **epistemic map**: agreement across *diverse* formalisms is strong corroboration;
disagreement is diagnostic (often expected, e.g. bounded "no bug to depth 20" vs. deductive
"no bug ever" is not a contradiction).

**"Convenient" ≈ shared spec/witness language.** Cheap ensembles: KeY + OpenJML (JML),
Frama-C plugins (ACSL), Viper front-ends, SV-COMP tools (witness format). Expensive:
bespoke, mutually-incompatible spec languages. Policy: use every tool that already speaks
the shared spec/witness language for free; add bespoke-spec tools only for high-assurance
requirements.

Decision: support **per-language tool ensembles** as a first-class option, aggregated as a
structured epistemic map rather than a boolean or a vote. The verdict model records, **per
tool**: `{ tool, formalism, soundness_direction (over | under | exact), bounded? + bound,
assumptions/simplifications, verdict }`. Example aggregate: *"proven unbounded by Verus;
corroborated by CBMC to depth 20; Frama-C EVA raised a likely-false alarm at line X
(over-approx). Overall: proven, high confidence."* This extends the verdict-strength idea
from categories down to individual tools.

### D3 — Category: declared, with inference as a labeled hint

Chosen: authors **declare** the category; PRL may **infer** it as a hint when omitted.

1. **Inferred routing is labeled** in the output (`category: 2b [inferred]`) and travels
   with the verdict alongside the verdict-strength field.
2. **Inference is rule-based, transparent, deterministic — not an LLM guess** — derived
   from vocabulary and property shape (runtime event streams → 2b; model variables → 2a;
   pre/post over code → 1; UI probes → 3) and **explainable** ("inferred 2b because the
   vocabulary binds a live event stream").
3. **Mis-routing is a correctness risk, so inferred routing is provisional** — prompt for
   confirmation or treat as lower-confidence until the author accepts it, after which it
   is promoted to declared.

Declared is authoritative; inference is a labeled, explainable convenience that never
silently determines a verdict's strength.

### D4 — Separate reusable vocabulary from per-environment groundings

Chosen: the **vocabulary (signature)** — abstract, typed, category-independent — is a
**shared, named module** (a domain ontology reused across requirements); **groundings** are
separate per-(category, environment) adapter modules that bind each symbol to a concrete
observable. Inline vocabulary is allowed for one-offs. See *Grounding layer*.

### D5 — Binding fidelity is first-class and feeds verdict strength

Each grounding declares `fidelity ∈ {definitional, observed, probed}`:

- `definitional` — true by construction (2a model variables/actions).
- `observed` — a runtime observation that can be wrong (2b event/log/DB queries).
- `probed` — a UI probe, flaky (3 selectors/driver).

`observed` and `probed` bindings are **three-valued**: a missing observation is
`unobserved ⇒ unknown`, never silently `false`. **A verdict can never be stronger than its
weakest binding's fidelity.**

### D6 — Mandatory correspondence for multi-category symbols; identity + time mandatory

- **Identity/correlation keys and time sources are mandatory** parts of runtime/UI
  bindings (a quantified variable needs an identity field; a timing bound needs a named
  clock).
- **Cross-category coherence is mandatory (fork 2 = a):** whenever a symbol is grounded in
  ≥2 categories, the author must state a **refinement mapping** asserting the bindings
  correspond (the runtime/UI binding is a faithful observation of the model binding). To
  keep this practical, a **trivial/identity correspondence may be explicitly acknowledged**
  when the bindings are definitionally the same — the author must confront and record it,
  but need not write a heavyweight mapping. Generated consistency checks (run the 2b
  grounding, confirm observed events match the 2a model) are opt-in high-assurance,
  mirroring D2a. This is where "2b monitoring closes the design gap" becomes concrete.

## Grounding layer (signature / interpretation)

Grounding binds abstract predicates/events to real observables. It is where the
specification gap bites hardest and most invisibly — a wrong binding silently makes a
true-looking requirement meaningless — so bindings are typed, reviewable, and
trust-annotated. The mechanism is the logic **signature / interpretation** split, the same
structure as TLA+ **refinement mappings**, model-based-testing **adapters**, Cucumber
**step-definitions**, and MonPoly **log schemas** (proven patterns, synthesized — not
invented).

Three parts:

1. **Vocabulary (signature)** — abstract, typed, category-independent:

   ```text
   vocabulary MessageLifecycle {
     sort  Message
     event accepted(m: Message)
     state succeeded(m: Message)
     state dead_lettered(m: Message, reason: String)
     identity Message = m.id          // correlation key
   }
   ```

2. **Grounding modules** — per category and environment, mapping each symbol to an
   observable:

   ```text
   grounding MessageLifecycle @kafka-prod for 2b {
     time = event.timestamp
     accepted(m)         ↦ span "queue.accept"    where attr["msg.id"] = m.id
     succeeded(m)        ↦ span "handler.complete" where attr["msg.id"] = m.id and status = OK
     dead_lettered(m, r) ↦ row  "dead_letters"     where msg_id = m.id and r = reason
     fidelity = observed              // 3-valued: missing span ⇒ unknown, not false
   }

   grounding MessageLifecycle @tla-model for 2a {
     accepted(m)  ↦ action Accept(m)
     succeeded(m) ↦ status[m] = "Succeeded"
     fidelity = definitional
   }
   ```

3. **Adapters** — what each grounding compiles to: **1 (code)** a state predicate at a
   source location (ACSL/JML/Viper); **2a (model)** a direct model variable/action
   reference (definitional); **2b (runtime)** a query over an event/telemetry stream
   (span/log/DB row); **3 (UI)** a probe (selector + assertion via Selenium/Playwright).

Three hard sub-problems the binding must handle: **identity/correlation** (which field is
the quantified variable's identity — parametric-monitoring trace-slicing); **time source**
(the named clock for timing bounds); and **partial observability ⇒ three-valued**
(`unobserved` distinct from `observed-false`, feeding the honest *unknown*).

## Core layer (working direction)

One **first-order metric temporal logic** with both linear and branching operators
(CTL\*/μ-calculus-grade + data quantification + real-time bounds) — the TLA+ lineage plus
metric time plus branching. Undecidable in general; each fragment checks a tractable
slice:

- **2a** → TLA+ / NuSMV / mCRL2 over bounded data domains (finite model checking).
- **2b** → parametric runtime monitors — **MFOTL** monitored by **MonPoly** (per-message,
  timed, quantified).
- **1** → the temporal-free fragment (pre/post/invariants) → Viper/deductive.
- **3** → UI probes as atomic predicates under response/timing patterns.

## Surface vocabulary (working set)

Specification patterns + scopes, plus the two extensions this project specifically needs:

- Patterns: `never P`, `always P`, `eventually P`, `P leads_to Q` (+ `within T`),
  `S precedes P`, `P occurs at most k times`.
- Scopes: `globally`, `before R`, `after Q`, `between Q and R`.
- **First-order quantification:** `each m: Message · …` (queues, DB records).
- **Possibility (branching):** `can_reach P` / `always can_reach P` (CTL `EF` / `AG EF`)
  — the branching-time need LTL cannot express.

## Worked example — message reliability

```text
requirement no_message_lost {
  category: 2a + 2b
  vocabulary {
    event  accepted(m: Message)
    state  in_flight(m), retrying(m), succeeded(m)
    state  dead_lettered(m: Message, reason: String)
  }
  assume { retries_bounded(N = 5) }          // makes liveness unconditional
  require {
    // conservation (safety)
    always  each m: Message · exactly_one_of {
              in_flight(m), retrying(m), succeeded(m), dead_lettered(m, _) }
    // disposition (bounded liveness)
    each m: Message ·
      accepted(m) leads_to (succeeded(m) or dead_lettered(m, r) with r != "")
  }
  strength: model_checked ∀ over Model,  monitored(deadline = 30s)
  evidence: tla+ (bounded: |Message| ≤ 8),  monpoly(stream = queue.events)
}
```

## Open items

- Precise grammar for the pattern surface and the sugar escape hatch.
- Formal semantics of the core logic and the exact boundary of each engine fragment.
- Concrete syntax for grounding modules and refinement mappings (the mechanism is decided
  in D4/D5/D6 and *Grounding layer*; the exact adapter syntax per category is still open).
- Verdict object schema (modality + strength + per-tool epistemic profile + inferred-routing
  labels + binding-fidelity + three-valued unknown + counterexample/witness format).
- How the LLM front-end lowers text → patterns, and how round-trip read-back is presented.
