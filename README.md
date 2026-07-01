# Provable Requirements

A space to brainstorm — and eventually build — an approach to software requirements
that make **provable, falsifiable claims** about code and system behavior.

## The Idea

Most software requirements are written in prose. They are ambiguous, untestable in
any rigorous sense, and drift out of sync with the systems they describe. This project
explores an alternative: requirements expressed as **precise statements about code and
system behavior that can be proven or falsified**.

A requirement here is not "the system should be fast" but a claim with a definite
truth value against a given implementation or design — something a tool could
mechanically check, or at least attempt to refute.

## Goals

The project is intended to progress roughly in these stages:

1. **Brainstorm** — explore what it means for a requirement to be provable/falsifiable:
   which languages, logics, or formalisms are appropriate; what "the system" and "the
   code" refer to precisely; and where the boundaries of decidability lie.
2. **Express** — develop a way to write requirements as assertions about behavior that
   carry a definite truth value (provable and/or falsifiable) against a real system.
3. **Prove / Falsify** — build tooling that can evaluate such requirements against
   **real code and/or system designs**, producing a proof, a counterexample, or an
   honest "unknown."

## Scope & Open Questions

This is early-stage and exploratory. Some of the questions we expect to wrestle with:

- What formal foundation fits — formal specification, model checking, type systems,
  property-based testing, SMT/theorem proving, or some blend?
- What can be *proven* versus only *falsified* (found a counterexample), and how do we
  represent "not yet decided" honestly?
- How do requirements attach to artifacts — source code, running systems, or
  higher-level designs and architectures?
- How do we keep requirements and the systems they describe from drifting apart?

## Requirement Categories

Requirements are not all the same *kind* of claim. They differ along one deep axis:
**what makes a verdict true, and how strong that truth is.** The spine runs

> **proof (static, universal) → model-checked (universal over a model) → monitored/tested
> (empirical, existential).**

Moving down that spine trades *universality* for *fidelity to reality*. That tradeoff is
why these are genuinely different categories, not a matter of taste. (Work so far has
focused on category 1.)

| # | Category | Artifact checked | Method / engine | Verdict strength |
| --- | --- | --- | --- | --- |
| 1 | **Code** (functional correctness) | source code | deductive verifier (Viper/Why3/SMT) | **Proof** — holds ∀ executions |
| 2a | **System — design-time** (behavioral/temporal) | a *model* of the system | model checker (TLA+, NuSMV, mCRL2; timing → MTL/TCTL) | **Proof over the model** — ∀ model behaviors (model ≠ reality) |
| 2b | **System — runtime** (behavioral/temporal) | the *running* system's traces | runtime verification: monitors from temporal specs + observability (tracing, queue/DB metrics) | **Empirical** — falsify, or confidence over observed runs |
| 3 | **UI** (acceptance) | the *rendered, running* UI | driver (Selenium/Playwright) exercising scenarios → True/False | **Empirical** — falsify, or confidence over exercised scenarios |

Example mappings (distributed-system flavored):

- **Data lands on queues; ordering; timing** — temporal properties. Ordering is a
  *consistency* property (linearizability / causal consistency); timing needs a *metric*
  temporal logic (MTL / Signal Temporal Logic). Model-check the design (2a); monitor the
  live queues (2b).
- **Data into/out of databases** — distributed consistency. Canonical *empirical* tool:
  **Jepsen** (with **Elle**/Knossos), which exercises real databases under fault
  injection and checks histories against linearizability/serializability (category 2b).
- **Monitors that start/stop/scale/tune components** — control-loop properties: *safety*
  ("never scale below min replicas") + *bounded liveness* ("under load > X, start an
  instance within T"). Model-checkable (2a), monitorable (2b).
- **UI appearance/behavior via Selenium** — category 3, pure black-box empirical.

### The unifying insight

**Temporal logic is the shared specification spine for category 2 (and much of 3).** The
same property — "messages delivered in order within 100 ms" — can be *model-checked*
against a design model (2a, proof over the model) *and* compiled into a *runtime monitor*
checked against the live trace (2b, empirical). Same spec, different engine, different
verdict strength — the layered-hybrid principle ("unify the meaning, federate the
engines") extended past code into behavior.

The categories also **cross-check** each other: code proofs (1) establish what the model
assumes; the design model (2a) proves global behavior *assuming the model is faithful*;
runtime monitoring (2b) checks the real system against the model — **closing the design
gap**; UI acceptance (3) checks the top, user-observable layer. Each layer validates an
assumption the layer below cannot see.

### Honest limits

- **2b and 3 can only falsify or build confidence — never prove** (finitely many observed
  runs). That is the *falsifiable* half of the project, and must be labeled as such.
- **Safety vs liveness at runtime.** Safety ("bad thing never happens") is monitorable;
  liveness ("good thing eventually happens") is *not* falsifiable from a finite trace —
  bound it (MTL "within T") to make it checkable. Most timing requirements are already
  bounded.
- **UI oracle & flakiness.** "Looks right" must reduce to precise assertions; Selenium is
  non-deterministic — strong for falsification/regression, weak as positive evidence.
- **Nondeterminism → probabilistic verdicts** in distributed 2b: "not falsified over N
  runs" is confidence, not proof.

**Non-negotiable consequence:** every requirement carries its **category/modality** and
its **verdict strength**, and the system must *never* render a Selenium green like a proof
green. The uniform verdict format gains an epistemic-strength field: `proven ∀` /
`model-checked ∀ over M` / `not-falsified over N runs` / `falsified: <trace>`.

Architecturally this slots into the layered hybrid untouched: the unified requirement
layer tags each requirement with a category that determines routing (1 → deductive; 2a →
model checker; 2b → monitor synthesis + observability; 3 → UI driver); the LLM front-end
lowers to the matching target per category; the verdict model gains the strength field.

## Inspiration & Prior Art

This work is inspired by two repositories from **Rob Fielding**, both of which treat
temporal-logic requirements as executable, machine-checkable artifacts rather than
prose.

### `ctl` — POBTL\* Model Checker (Python)

- Author: Rob Fielding
- URL: <https://github.com/rfielding/ctl>

A modal / temporal logic model checker written in Python. Systems are described as
collections of states (dictionaries), propositions are plain Python lambdas over those
states, and requirements are expressed with CTL\*-style operators that reason about
possible futures and past states:

- **Future / global:** *Exists Finally* (EF) and *Always Finally* (AF) for properties
  that eventually hold; *Exists Globally* (EG) and *Always Globally* (AG) for properties
  that persist.
- **Past:** *Exists/Always Previously* (EP/AP) and *Exists/Always Historically* (EH/AH)
  for constraints on history.
- **Strong implication:** a combined reachability-plus-guarantee construct — `p` strongly
  implies `q` when `p` is reachable and, whenever `p` holds, `q` always follows.

The checker evaluates a formula against the state space and returns the set of states
that satisfy it, letting you formally state a requirement and mechanically find where it
holds or fails — no specialized symbolic-logic tooling required.

### `go-ctl2` — Kripke Philosophy Calculator (Go)

- Author: Rob Fielding
- URL: <https://github.com/rfielding/go-ctl2>

A successor project, rewritten in Go, aimed at formally verifying **actor-based,
concurrent/distributed systems** with CTL. Its distinguishing ideas:

- **Verify visible behavior only.** CTL assertions range over *visible* state — named
  control states and mailbox/channel contents — rather than actors' internal variables,
  keeping specifications at the level of observable system behavior.
- **LLM-assisted specification.** The intended workflow bridges natural-language
  requirements and machine-checkable models: (1) a language model emits a Lisp-based
  intermediate representation, (2) a compiler turns it into an explicit transition
  system, (3) the developer inspects the states, channels, diagrams, and CTL claims,
  and (4) the model is refined iteratively until the requirements are precise enough to
  verify.
- **Structured messaging.** Properties can assert facts such as a server reaching a
  "done" state, or a client's mailbox containing a specific structured event with a
  particular timestamp and value.

It also ships visualization (JavaScript/CSS webapp) for inspecting the generated
transition systems.

## Proposed Workflow

The working direction is an **LLM-as-untrusted-front-end** pipeline: use an LLM to
translate informal, text-based requirements into formal statements, then hand those to
a trusted checker (a prover and/or model checker) to prove or falsify them.

```text
English requirement
   → [LLM] candidate formal statement   ← HUMAN (or adversarial LLM) reviews THIS
   → [LLM] candidate proof / model
   → [Checker: prover / model checker] verify
        ├─ proved         → done
        ├─ counterexample → report the falsifying witness / trace
        └─ stuck/unknown  → feed the error back to the LLM, refine (bounded loop)
```

This generalizes the `go-ctl2` loop (LLM proposes, a mechanical checker disposes) and
matches the project's goal of producing a proof, a counterexample, or an honest
"unknown."

**Principles that make it sound:**

- **Trust boundary.** The LLM is *untrusted* — a synthesizer, good at bridging fuzzy
  English to formal syntax, unreliable about soundness. The checker is *trusted* — a
  small, sound kernel that validates the LLM's output. Never let the LLM into the
  trusted checking path.
- **The specification gap is the real risk.** A checker verifies that *the proof proves
  the theorem*; it cannot verify that *the theorem faithfully captures your intent*. The
  translation step we are handing the LLM is exactly where meaning can silently leak
  (e.g. a vacuously-true formula that "verifies" but means nothing).
- **Review the statement, not the proof.** Keep the generated formal statement
  human-readable and reviewable so a person (or a second, adversarial LLM pass) can
  confirm it means what was asked. Let the LLM and checker own the proof entirely.
- **Proof *and* falsification.** Provers give proofs but rarely cheap counterexamples;
  model checkers give counterexamples cheaply. Expect a *portfolio* of checkers rather
  than a single tool.

## Tool Landscape (under investigation)

Candidate checkers we are evaluating, grouped by role. To be refined as we dig in.

- **Foundational provers** — Lean 4, Coq, Isabelle/HOL, Agda. Deepest expressiveness;
  reason about objects defined in their own logic, so real code must be modeled inside
  the tool (or written in the prover itself). Weakest automation.
- **Code verifiers (SMT-backed)** — **Verus** (verifies real Rust in place), **Dafny**
  (own language → C#/Java/Go/Python/JS), **F\*** (own ML-family language → OCaml/F#/C/
  Wasm; e.g. HACL\* crypto). Smaller code-to-proof gap; Floyd–Hoare style
  (pre/post/invariants), not temporal.
- **Design-level checkers** — **TLA+** (with TLC / Apalache / TLAPS) and **Alloy**.
  Model designs, not code; both are *linear-time* temporal (LTL-style), and Alloy is a
  *bounded* falsifier. Also in this family: **Z** notation (set-theory/schema-based
  specification, but oriented to *manual/interactive* proof — weak automation) and its
  more actionable descendants **B / Event-B** (refinement down toward code; Rodin /
  Atelier B tooling). Alloy was designed as an *automatically analyzable* answer to Z.
- **Temporal model checkers (the direct lineage of `ctl`/`go-ctl2`)** — **NuSMV/nuXmv**
  (native **CTL and LTL**, symbolic/SMT), **mCRL2** (modal μ-calculus, which subsumes
  CTL\*). These are the mature, industrial versions of branching-time CTL model checking.

Not a checker but shared infrastructure: **Z3** (SMT solver, Microsoft Research) is the
common backend under Verus, Dafny, F\*, and Apalache — the engine that makes the
SMT-backed tools automatic, not a tool one selects directly. (Do not confuse with **Z**
notation above.)

Open selection criterion: **branching-time (CTL) vs linear-time (LTL).** If requirements
need "there exists a future where…" (`EF`) claims, the design-level tools (TLA+/Alloy)
are the wrong logic and a true CTL checker is needed.

### Coverage by language (deductive / functional-correctness tools)

| Language | Coverage | Tools / spec language |
| --- | --- | --- |
| C | Strong | **Frama-C** + **ACSL** (WP→SMT); **VeriFast**; **VCC**; bounded: **CBMC**, **CPAchecker**, **Ultimate** |
| C++ | Weak (subsets only) | **CBMC/ESBMC** (bounded); **Infer**, **Astrée** (static analysis). Full deductive C++ ≈ open problem |
| Java | Strong | **KeY** + **JML**; **OpenJML**; **VerCors**; **VeriFast**; bounded **JBMC** |
| Kotlin | Essentially none | Only its sound null-safety type system; JVM-bytecode tools could apply but nothing dedicated |
| Rust | Strong | **Verus**, **Prusti** |
| Go | Yes | **Gobra** (Viper) |
| Python | Yes | **Nagini** (Viper) |
| Ada | Strong | **SPARK** (a verifiable *subset* of Ada, on Why3) |
| D | None (runtime only) | Built-in `in`/`out`/`invariant` contracts are *runtime* checks, not static proof |
| Dart | None | Sound null-safety only |
| V | None | No formal tooling |
| Zig | None | `comptime` + safety builds; community interest, no deductive verifier |

#### Why C++ is "weak (subsets only)"

C++'s semantics are uniquely hostile to *sound* deductive verification. Tools attempt it
and end up supporting only restricted subsets because:

- **Pervasive undefined behavior (UB).** Hundreds of UB triggers (signed overflow,
  out-of-bounds, use-after-free, strict-aliasing violations, uninitialized reads, invalid
  downcasts, data races, unspecified evaluation order). A sound verifier must *prove UB
  never occurs on any path* — a huge burden on top of the actual property, with no simple
  fallback semantics because UB means "anything may happen."
- **A brutal memory/object model.** Pointer arithmetic, `reinterpret_cast`, unions,
  placement `new`, and intricate **object-lifetime** rules require separation-logic-grade
  aliasing reasoning; there is no small, clean memory model to build on.
- **Templates / metaprogramming.** Turing-complete at compile time; no fixed program
  until instantiation, which itself depends on overload resolution, ADL, SFINAE, concepts.
- **Implicit control flow everywhere.** Almost any operation can throw; unwinding runs
  destructors in reverse construction order (RAII), so every function fans out into many
  invisible exceptional paths.
- **A research-grade concurrency model.** The C++11+ memory model (relaxed atomics,
  happens-before) is one of the hardest formal objects in any mainstream language.
- **Even the front-end is hard.** Only a few complete C++ front-ends exist; a verifier
  must assign formal meaning to that entire, still-growing (C++11/14/17/20/23…) AST.

Contrast: Rust verification (Prusti, Verus) works *because* Rust was co-designed with
verification-friendly invariants (borrow checker restricts aliasing; safe Rust has little
UB). C++ made the opposite trade — zero-cost abstraction and backward compatibility over
analyzability.

**Decision for this project:** accept the limitation. C++ is deprioritized; if it is ever
addressed, the realistic path is a *constrained verifiable subset* (SPARK-for-Ada style,
think MISRA/AUTOSAR-restricted C++), not the whole language. Revisit later to whatever
degree is practical.

### Intermediate verification languages — the multi-language / extensibility pattern

The established way to cover many languages *without rebuilding the prover* is an
**Intermediate Verification Language (IVL)**: a shared VC-generating core, with a
per-language *front-end* translating source (+ spec annotations) into the IVL, and
pluggable SMT/prover *back-ends*.

- **Why3** (WhyML) — front-ends for SPARK/Ada, Java (Krakatoa), C; dispatches to
  Alt-Ergo/Z3/CVC5/Coq/Isabelle.
- **Viper** (ETH; permission/separation logic) — front-ends **Prusti** (Rust),
  **Nagini** (Python), **Gobra** (Go), **VerCors** (Java/C). Cleanest "add a language =
  add a front-end" template.
- **Boogie** (Microsoft) — targeted by Dafny and VCC.

Effort reality for building a wide-spectrum, extensible tool: the IVL core, VC
generation, and SMT back-ends are *already solved* and reusable. The real cost is a
**faithful per-language front-end** — precisely modeling each language's semantics
(memory model, aliasing, overflow, exceptions, generics/templates, concurrency, FFI).
That ranges from tractable (Dart, a Zig subset) to brutal (C++). Prusti and VerCors each
took years. Pragmatic precedent: **SPARK** *constrains* Ada to a verifiable subset rather
than conquering the whole language.

### Language shortlist assessment (related project's target set)

A companion research project targets this language set:
ada, C, C++, C3, C#, D, dart, go, java, kotlin, lua, mojo, ocaml, odin, python, ruby,
rust, swift, systemC, typescript, V, and zig. Assessed against verification reality
(deductive axis):

**Bucket A — first-class existing support (7).** **Ada** (SPARK), **C** (Frama-C/ACSL,
VeriFast, VCC), **Java** (KeY/JML, VerCors), **Rust** (Verus, Prusti, Creusot, Kani),
**Go** (Gobra), **Python** (Nagini — *requires type annotations*), **OCaml** (Cameleer +
GOSPEL). These cluster on the Why3/Viper IVLs — which is exactly why a unified tool is
feasible.

**Bucket B — possible in a unified tool (10), mainly a front-end + semantics effort.**
Cheap wins (managed/typed): **C#** (dead precedent Spec#; CIL front-end feasible),
**Dart**, **Swift** (types + value semantics + ownership), **Kotlin** (managed, typed,
null-safe — verify at JVM-bytecode level or via a Viper front-end; medium because of
coroutines / smart casts / reified generics). C-like systems (need Viper-style
separation logic): **C3**, **Odin**, **Zig**, **D**, **V**. Gated by JS runtime
semantics: **TypeScript** (its type system helps; the underlying JavaScript is the mess).

**Bucket C — impractical (5), and why.** **C++** (see above). **SystemC** — *is* C++ (an
electronic-system-level modeling library), so it inherits everything; its real home is
*hardware model checking* (the temporal axis, not deductive). **Lua** and **Ruby** —
dynamically typed, no static structure; only buildable if a typed dialect
(Luau/Teal/Sorbet) is required first. **Mojo** — immature, unstable, semi-proprietary
moving target; promising ownership design but not pin-down-able yet ("wait, not no").

**Three fault lines that decide the buckets:**

1. **Static typing is the price of admission** — every impractical dynamic language fails
   for the same reason; Python only qualifies *via* mandatory annotations.
2. **Memory model sets the front-end cost** — managed languages are cheap; every
   manual-memory systems language needs the *same* reusable separation-logic machinery
   (argues for the **Viper** IVL if we go unified).
3. **SystemC belongs on the temporal axis** — a reminder the project likely wants both a
   deductive engine *and* a model-checking engine, with the requirement layer routing
   properties to whichever fits.

## Architecture Direction: Layered Hybrid (decided)

**The question.** Should the project be a *scattered* set of best-of-breed tools (a
different verifier per language, plus bespoke tooling for the rest), or *one unified,
extensible* tool built on an existing IVL?

**Reframing.** The two options aren't the real choice. The project's actual contribution
is the **requirement layer** (expressing provable/falsifiable requirements, LLM-assisted
translation, and routing to a checker); the checkers underneath are largely
interchangeable infrastructure. So the real question is *where unification must live*.

### Option A — scattered / federated (per-language native tools)

- **Pros:** fastest to breadth (reuse mature SPARK/Frama-C/Verus/KeY/Gobra/Nagini/… on
  day one); best-in-class per language; low semantic burden (upstream owns the hard
  parts).
- **Cons:** no common meaning of "proved" (N spec dialects + N logics → non-comparable
  verdicts, which undercuts the whole "provable requirements" claim); permanent
  integration tax (N parsers/toolchains/formats/TCBs/failure modes, glue rots); ragged
  expressiveness (collapses to the intersection); you still build from scratch for
  uncovered languages *anyway*; cross-language requirements ≈ impossible; the LLM must
  target N formalisms.

### Option B — unified extensible tool on one IVL (Why3 or Viper)

- **Pros:** one semantics of "proved" (single requirement language, logic, and
  verdict format); one trust story / TCB; extensibility first-class (add a language = a
  front-end; separation-logic machinery reused across all manual-memory languages);
  cross-language requirements become conceivable; the LLM targets *one* formalism; still
  reuses the hard-won IVL core (not writing the prover).
- **Cons:** slower to breadth (one language before many; front-ends are person-months);
  leaves mature tools on the table (re-deriving what SPARK already nails); you own the
  faithful-semantics burden per language; **IVL ceiling risk** (Why3/Viper limits around
  concurrency, higher-order, temporal); and it isn't fully unified anyway — the temporal
  axis needs a separate engine regardless.

### Decision — layered hybrid: unify the *meaning*, federate the *engines*

Deciding principle: **unify the thing that must mean one thing — the requirement and its
verdict — and be pragmatic about the engines underneath.**

1. **Unified requirement layer + one notion of proof, from day one.** Non-negotiable;
   it's the project's identity. A requirement means one thing; a verdict is *proof /
   counterexample / honest-unknown* in one format.
2. **One primary IVL as the workhorse — Viper.** Covers the largest chunk of the
   *buildable* set with shared separation-logic machinery; Prusti/Nagini/Gobra already
   prove the "add a language = add a front-end" template.
3. **Delegate to a mature native tool as a normalized back-end** where it clearly
   dominates (e.g. Ada → SPARK) — *only if* its result maps into the uniform verdict
   model. Federation stays *behind* the uniform layer, never in front of it (no N spec
   dialects exposed to users or the LLM).
4. **A second engine for the temporal axis** (NuSMV/mCRL2 lineage) sits behind the *same*
   requirement layer for CTL / SystemC-style properties. "Unified" = unified interface,
   not one engine.
5. **Sequence it.** Prove the whole architecture end-to-end on **one clean language
   first** (Rust via a Viper front-end, or a managed language like Dart/C# for lower
   semantic drag), then add front-ends and native-tool back-ends.

One-line test for any design choice here: *does a requirement — and the answer to "does
it hold?" — mean the same thing regardless of which engine ran?* If yes, scattered
engines are fine; if no, it isn't really provable requirements.

## Design Q&A (living notes)

A running log of questions worked through and their distilled answers. Refined as we
drill down; fuller treatment lives in the sections above.

- **Should an LLM be part of an automated pipeline?** Yes — but strictly as an
  *untrusted front-end* that translates text requirements into formal statements, with a
  trusted checker validating the output. See *Proposed Workflow*.
- **Do foundational provers (Lean 4, Coq, Isabelle/HOL, Agda) all share a code-to-proof
  gap?** Yes, fundamentally — they reason about objects defined in their own logic, so
  real code must be modeled inside the tool. Mitigations: write the program *in* the
  prover (no gap), or bridge via extraction / embedded semantics (Coq→CompCert,
  Isabelle→seL4) at real cost.
- **Which languages do the code verifiers cover?** Verus → real **Rust** in place;
  Dafny → its own language, compiling to C#/Java/Go/Python/JS; F\* → its own ML-family
  language, extracting to OCaml/F#/C/Wasm.
- **Do any of these subsume `ctl` / `go-ctl2`?** Only true CTL model checkers:
  **NuSMV/nuXmv** (native CTL + LTL) and **mCRL2** (μ-calculus, superset of CTL\*).
  TLA+/Alloy cover the same *use case* but in linear-time logic, not branching-time CTL.
- **Is Z / Z3 useful here?** Two different things. **Z** = a design-level specification
  language (set theory + schemas), weak automation, superseded for our goals by Event-B
  (refinement) or Alloy (automation). **Z3** = the SMT solver already underpinning
  Verus/Dafny/F\*/Apalache — infrastructure, not a competitor.
- **What about C/C++, Java/Kotlin, and D/Dart/V/Zig?** C: strong (Frama-C/ACSL). C++:
  weak, subsets only (full deductive C++ ≈ open problem). Java: strong (KeY/JML,
  VerCors). Kotlin: essentially none. D/Dart/V/Zig: no real deductive verifiers — open
  territory. See *Coverage by language*.
- **Why is C++ specifically so weak?** Its semantics are hostile to sound verification —
  pervasive undefined behavior, a brutal memory/object-lifetime model, Turing-complete
  templates, implicit exception/destructor control flow, and a research-grade concurrency
  model. Rust verifies well because it was co-designed for it; C++ chose the opposite
  trade. **Decision:** deprioritize C++; revisit later only as a constrained verifiable
  subset (SPARK-style). See *Why C++ is "weak (subsets only)"*.
- **Of the companion project's 22 languages, what's supported / buildable / impractical?**
  First-class today (7): Ada, C, Java, Rust, Go, Python, OCaml. Buildable via IVL
  front-ends (10): C#, Dart, Swift, Kotlin, C3, Odin, Zig, D, V, TypeScript.
  Impractical (5):
  C++, SystemC (it *is* C++; belongs on the temporal/model-checking axis), Lua, Ruby
  (dynamic typing — need a typed dialect), Mojo (immature moving target). Deciding
  factors: static typing (price of admission), memory model (front-end cost), and the
  deductive-vs-temporal axis. See *Language shortlist assessment*.
- **Are code, system-behavior, and UI requirements the same kind of claim?** No — they
  sit on one epistemic spine (proof → model-checked → monitored/tested), trading
  universality for fidelity. Four categories: (1) **Code** (deductive, proof ∀), (2a)
  **System design-time** (model checking, proof over a model), (2b) **System runtime**
  (runtime verification/monitoring, empirical), (3) **UI** (Selenium/driver, empirical).
  Temporal logic is the shared spec for 2 (and much of 3); the layers cross-check (2b
  closes the design gap). Every requirement must carry its modality + verdict strength;
  never conflate a Selenium green with a proof. See *Requirement Categories*.
- **Scattered per-language tooling, or one unified extensible tool?** Neither in its pure
  form — **layered hybrid (decided).** Unify the *requirement layer and the meaning of a
  verdict*; federate the *engines* behind it: one primary IVL (**Viper**) as workhorse,
  mature native tools (e.g. SPARK) as normalized back-ends, plus a model checker for the
  temporal axis — all behind one uniform interface. Sequence: one clean language
  end-to-end first. See *Architecture Direction: Layered Hybrid*.
- **How big a deal to build a wider-spectrum, extensible tool?** Not "invent something
  new" — adopt the **IVL pattern** (Why3 or Viper): reuse the solved core (VC generation,
  SMT back-ends) and write a *faithful per-language front-end* per language. Front-ends
  are the real cost (tractable for clean languages, brutal for C++). Keep the requirement
  layer IVL-agnostic; consider *constraining* a language to a verifiable subset (SPARK
  precedent). Note this is the *deductive* axis, distinct from the *temporal/CTL* axis —
  a general tool likely needs a requirement layer that routes properties to the right
  engine.

## Status

Brainstorming. No code yet — ideas, notes, and direction come first; implementation
follows once the concepts are sharp enough to build on.
