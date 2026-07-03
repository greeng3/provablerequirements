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
- What can be _proven_ versus only _falsified_ (found a counterexample), and how do we
  represent "not yet decided" honestly?
- How do requirements attach to artifacts — source code, running systems, or
  higher-level designs and architectures?
- How do we keep requirements and the systems they describe from drifting apart?

## Requirement Categories

Requirements are not all the same _kind_ of claim. They differ along one deep axis:
**what makes a verdict true, and how strong that truth is.** The spine runs

> **proof (static, universal) → model-checked (universal over a model) → monitored/tested
> (empirical, existential).**

Moving down that spine trades _universality_ for _fidelity to reality_. That tradeoff is
why these are genuinely different categories, not a matter of taste. (Work so far has
focused on category 1.)

| #   | Category                                       | Artifact checked              | Method / engine                                                                                | Verdict strength                                                |
| --- | ---------------------------------------------- | ----------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| 1   | **Code** (functional correctness)              | source code                   | deductive verifier (Viper/Why3/SMT)                                                            | **Proof** — holds ∀ executions                                  |
| 2a  | **System — design-time** (behavioral/temporal) | a _model_ of the system       | model checker (TLA+, NuSMV, mCRL2; timing → MTL/TCTL)                                          | **Proof over the model** — ∀ model behaviors (model ≠ reality)  |
| 2b  | **System — runtime** (behavioral/temporal)     | the _running_ system's traces | runtime verification: monitors from temporal specs + observability (tracing, queue/DB metrics) | **Empirical** — falsify, or confidence over observed runs       |
| 3   | **UI** (acceptance)                            | the _rendered, running_ UI    | driver (Selenium/Playwright) exercising scenarios → True/False                                 | **Empirical** — falsify, or confidence over exercised scenarios |

Example mappings (distributed-system flavored):

- **Data lands on queues; ordering; timing** — temporal properties. Ordering is a
  _consistency_ property (linearizability / causal consistency); timing needs a _metric_
  temporal logic (MTL / Signal Temporal Logic). Model-check the design (2a); monitor the
  live queues (2b).
- **Data into/out of databases** — distributed consistency. Canonical _empirical_ tool:
  **Jepsen** (with **Elle**/Knossos), which exercises real databases under fault
  injection and checks histories against linearizability/serializability (category 2b).
- **Monitors that start/stop/scale/tune components** — control-loop properties: _safety_
  ("never scale below min replicas") + _bounded liveness_ ("under load > X, start an
  instance within T"). Model-checkable (2a), monitorable (2b).
- **UI appearance/behavior via Selenium** — category 3, pure black-box empirical.

### The unifying insight

**Temporal logic is the shared specification spine for category 2 (and much of 3).** The
same property — "messages delivered in order within 100 ms" — can be _model-checked_
against a design model (2a, proof over the model) _and_ compiled into a _runtime monitor_
checked against the live trace (2b, empirical). Same spec, different engine, different
verdict strength — the layered-hybrid principle ("unify the meaning, federate the
engines") extended past code into behavior.

The categories also **cross-check** each other: code proofs (1) establish what the model
assumes; the design model (2a) proves global behavior _assuming the model is faithful_;
runtime monitoring (2b) checks the real system against the model — **closing the design
gap**; UI acceptance (3) checks the top, user-observable layer. Each layer validates an
assumption the layer below cannot see.

### Honest limits

- **2b and 3 can only falsify or build confidence — never prove** (finitely many observed
  runs). That is the _falsifiable_ half of the project, and must be labeled as such.
- **Safety vs liveness at runtime.** Safety ("bad thing never happens") is monitorable;
  liveness ("good thing eventually happens") is _not_ falsifiable from a finite trace —
  bound it (MTL "within T") to make it checkable. Most timing requirements are already
  bounded.
- **UI oracle & flakiness.** "Looks right" must reduce to precise assertions; Selenium is
  non-deterministic — strong for falsification/regression, weak as positive evidence.
- **Nondeterminism → probabilistic verdicts** in distributed 2b: "not falsified over N
  runs" is confidence, not proof.

**Non-negotiable consequence:** every requirement carries its **category/modality** and
its **verdict strength**, and the system must _never_ render a Selenium green like a proof
green. The uniform verdict format gains an epistemic-strength field: `proven ∀` /
`model-checked ∀ over M` / `not-falsified over N runs` / `falsified: <trace>`.

Architecturally this slots into the layered hybrid untouched: the unified requirement
layer tags each requirement with a category that determines routing (1 → deductive; 2a →
model checker; 2b → monitor synthesis + observability; 3 → UI driver); the LLM front-end
lowers to the matching target per category; the verdict model gains the strength field.

### Concurrency & parallelism requirements

Yes — expressible, and in fact **concurrency is the best-served domain in formal
methods**: model checking and process algebra were essentially invented for it (and
`go-ctl2` is literally about verifying concurrent actor systems). All such properties are
_temporal_, so they slot into the categories above and split along the classic **safety
vs liveness** line.

| Requirement                        | Class                          | Checkable in               | Representative tools                                                             |
| ---------------------------------- | ------------------------------ | -------------------------- | -------------------------------------------------------------------------------- |
| **Mutual exclusion**               | Safety (`AG ¬(cs₁ ∧ cs₂)`)     | 1, 2a, 2b                  | Rust ownership, VerCors/VeriFast (1); SPIN/TLA+/NuSMV (2a); ThreadSanitizer (2b) |
| **Progress / no deadlock**         | Liveness + global reachability | 2a (best), 1 (partial), 2b | SPIN/TLA+/mCRL2/**CSP+FDR** (2a); lock-order proofs (1); watchdogs (2b)          |
| **Temporal ordering of ops/calls** | Safety / protocol              | 1, 2a, 2b                  | **session types / typestate** (1); LTL/CTL (2a); RV monitors (2b)                |

- **Category 1 (code):** concurrent separation logic (what Viper's permissions are _for_)
  — **VerCors**, **VeriFast** verify data-race freedom; **Iris** (Coq) is the gold
  standard for fine-grained structures; **Rust ownership statically guarantees data-race
  freedom** in safe code. Ordering of operations → **session types / typestate**.
  Deadlock-freedom is the weak spot (needs lock-order discipline).
- **Category 2a (design, model checking) — home turf:** mutual exclusion is _the_
  canonical example; **deadlock detection is built in** (SPIN, TLC, mCRL2, CSP+FDR);
  progress = liveness under **fairness** (SPIN/NuSMV/TLA+ WF/SF).
- **Category 2b (runtime):** ThreadSanitizer / Go `-race` / Helgrind (races), deadlock
  watchdogs, RV monitors over call/event traces.

**Caveats (all consistent with the verdict-strength spine):**

- **State explosion** is the fundamental limit of 2a (interleavings blow up); mitigated by
  partial-order / symbolic / bounded / symmetry reduction, but why real systems get
  _abstracted_ to models.
- **Liveness needs fairness assumptions** — "progress" only holds under a fair scheduler;
  the assumption must be _stated_ (weak/strong fairness).
- **Dynamic detectors have false negatives** — TSan finds a race only on the schedule it
  observed. 2b _falsifies_ concurrency bugs; only 1 and 2a _prove_ their absence.
- **Deadlock vs livelock** — deadlock is directly checkable; livelock (running, no
  progress) is subtler and wants process-algebra/liveness tooling.

**Implication for the requirement language:** it needs, as first-class citizens,
**temporal operators** (□/◇/until, CTL `E`/`A` path quantifiers), **fairness
assumptions**, and a notion of **atomic operations/events** (so "in critical section" and
"ordering of function calls" are expressible).

### Worked example: message reliability ("no message is ever lost")

Requirement: every accepted queue message is either processed successfully, or fails
transiently and is retried, or fails long-term and is recorded somewhere with a reason.
This is the canonical distributed-systems guarantee, and it decomposes cleanly:

- **Safety (conservation — nothing vanishes):** every message is _always_ in exactly one
  of `{in-flight, retrying, succeeded, dead-lettered}`; no transition drops a message
  without recording it. A conservation invariant (like conservation of mass).
- **Liveness (disposition):** `□(accepted(m) → ◇(succeeded(m) ∨ deadLettered(m)))` — every
  accepted message eventually reaches a terminal state.
- **Reason field:** permanent failure → dead-letter store with a non-empty, classified
  reason.
- **Retry-loop hazard:** if transient failures recur forever the message livelocks in
  `retrying` and liveness fails. Two sound fixes — **bounded retries** (after N,
  transient → permanent → dead-lettered; makes liveness _unconditional_, preferred) or an
  explicit **fairness assumption** ("transient conditions eventually clear").

Category: fundamentally **2a/2b (system), not code.** Model-check the design (2a; TLA+ is
tailor-made — failure modes as nondeterministic choices); monitor the deployment (2b;
reconciliation `count_in == succeeded + dead_lettered + in_flight` + bounded-liveness
deadline monitors).

**Honest caveat:** _exactly-once processing is impossible_ under distributed failure. "No
loss" = at-least-once (achievable, checkable, may duplicate → needs idempotency);
at-most-once = no duplicates but possible loss. The language must let you say _which_ you
mean.

### "Requirements become models, which are then checked"

A sound and productive direction — with one fork that is a trap. Four readings:

1. **Standard model checking — model and property are _separate_.** A model `M` (system
   behavior) and property `φ` (the requirement); the checker decides `M ⊨ φ`. The
   requirement is an _independent assertion about_ the model, not the model itself.
2. **Generate the model _from_ requirements (the `go-ctl2` style).** Powerful, but the
   trap: if the _same_ source produces both the model and the property, verification can
   be **vacuous/circular** — the model is built to satisfy the property. **The property
   must be an independent constraint the model could plausibly violate** (the "review the
   statement, not the proof" principle again).
3. **Refinement (richest reading) — Event-B / TLA+.** Treat the requirement as a
   high-level abstract model, check it, then **refine stepwise toward code, proving each
   step preserves the abstract guarantees.** A principled bridge from a category-2a
   requirement-model down to category-1 code — the taxonomy's two ends connected by proof.
4. **Synthesis (advanced) — reactive synthesis / GR(1).** Generate a correct-by-
   construction implementation from a temporal spec. Real but heavier; not a starting
   point.

**Discipline:** keep model and property independent even when an LLM generates the model.
The LLM (untrusted) may _propose_ the model; the property stays an independent, trusted
assertion; the checker is the trusted judge — the same trust boundary as the workflow,
applied to model generation.

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

- **Future / global:** _Exists Finally_ (EF) and _Always Finally_ (AF) for properties
  that eventually hold; _Exists Globally_ (EG) and _Always Globally_ (AG) for properties
  that persist.
- **Past:** _Exists/Always Previously_ (EP/AP) and _Exists/Always Historically_ (EH/AH)
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

- **Verify visible behavior only.** CTL assertions range over _visible_ state — named
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

- **Trust boundary.** The LLM is _untrusted_ — a synthesizer, good at bridging fuzzy
  English to formal syntax, unreliable about soundness. The checker is _trusted_ — a
  small, sound kernel that validates the LLM's output. Never let the LLM into the
  trusted checking path.
- **The specification gap is the real risk.** A checker verifies that _the proof proves
  the theorem_; it cannot verify that _the theorem faithfully captures your intent_. The
  translation step we are handing the LLM is exactly where meaning can silently leak
  (e.g. a vacuously-true formula that "verifies" but means nothing).
- **Review the statement, not the proof.** Keep the generated formal statement
  human-readable and reviewable so a person (or a second, adversarial LLM pass) can
  confirm it means what was asked. Let the LLM and checker own the proof entirely.
- **Proof _and_ falsification.** Provers give proofs but rarely cheap counterexamples;
  model checkers give counterexamples cheaply. Expect a _portfolio_ of checkers rather
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
  Model designs, not code; both are _linear-time_ temporal (LTL-style), and Alloy is a
  _bounded_ falsifier. Also in this family: **Z** notation (set-theory/schema-based
  specification, but oriented to _manual/interactive_ proof — weak automation) and its
  more actionable descendants **B / Event-B** (refinement down toward code; Rodin /
  Atelier B tooling). Alloy was designed as an _automatically analyzable_ answer to Z.
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

| Language | Coverage            | Tools / spec language                                                                                   |
| -------- | ------------------- | ------------------------------------------------------------------------------------------------------- |
| C        | Strong              | **Frama-C** + **ACSL** (WP→SMT); **VeriFast**; **VCC**; bounded: **CBMC**, **CPAchecker**, **Ultimate** |
| C++      | Weak (subsets only) | **CBMC/ESBMC** (bounded); **Infer**, **Astrée** (static analysis). Full deductive C++ ≈ open problem    |
| Java     | Strong              | **KeY** + **JML**; **OpenJML**; **VerCors**; **VeriFast**; bounded **JBMC**                             |
| Kotlin   | Essentially none    | Only its sound null-safety type system; JVM-bytecode tools could apply but nothing dedicated            |
| Rust     | Strong              | **Verus**, **Prusti**                                                                                   |
| Go       | Yes                 | **Gobra** (Viper)                                                                                       |
| Python   | Yes                 | **Nagini** (Viper)                                                                                      |
| Ada      | Strong              | **SPARK** (a verifiable _subset_ of Ada, on Why3)                                                       |
| D        | None (runtime only) | Built-in `in`/`out`/`invariant` contracts are _runtime_ checks, not static proof                        |
| Dart     | None                | Sound null-safety only                                                                                  |
| V        | None                | No formal tooling                                                                                       |
| Zig      | None                | `comptime` + safety builds; community interest, no deductive verifier                                   |

#### Why C++ is "weak (subsets only)"

C++'s semantics are uniquely hostile to _sound_ deductive verification. Tools attempt it
and end up supporting only restricted subsets because:

- **Pervasive undefined behavior (UB).** Hundreds of UB triggers (signed overflow,
  out-of-bounds, use-after-free, strict-aliasing violations, uninitialized reads, invalid
  downcasts, data races, unspecified evaluation order). A sound verifier must _prove UB
  never occurs on any path_ — a huge burden on top of the actual property, with no simple
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

Contrast: Rust verification (Prusti, Verus) works _because_ Rust was co-designed with
verification-friendly invariants (borrow checker restricts aliasing; safe Rust has little
UB). C++ made the opposite trade — zero-cost abstraction and backward compatibility over
analyzability.

**Decision for this project:** accept the limitation. C++ is deprioritized; if it is ever
addressed, the realistic path is a _constrained verifiable subset_ (SPARK-for-Ada style,
think MISRA/AUTOSAR-restricted C++), not the whole language. Revisit later to whatever
degree is practical.

### Intermediate verification languages — the multi-language / extensibility pattern

The established way to cover many languages _without rebuilding the prover_ is an
**Intermediate Verification Language (IVL)**: a shared VC-generating core, with a
per-language _front-end_ translating source (+ spec annotations) into the IVL, and
pluggable SMT/prover _back-ends_.

- **Why3** (WhyML) — front-ends for SPARK/Ada, Java (Krakatoa), C; dispatches to
  Alt-Ergo/Z3/CVC5/Coq/Isabelle.
- **Viper** (ETH; permission/separation logic) — front-ends **Prusti** (Rust),
  **Nagini** (Python), **Gobra** (Go), **VerCors** (Java/C). Cleanest "add a language =
  add a front-end" template.
- **Boogie** (Microsoft) — targeted by Dafny and VCC.

Effort reality for building a wide-spectrum, extensible tool: the IVL core, VC
generation, and SMT back-ends are _already solved_ and reusable. The real cost is a
**faithful per-language front-end** — precisely modeling each language's semantics
(memory model, aliasing, overflow, exceptions, generics/templates, concurrency, FFI).
That ranges from tractable (Dart, a Zig subset) to brutal (C++). Prusti and VerCors each
took years. Pragmatic precedent: **SPARK** _constrains_ Ada to a verifiable subset rather
than conquering the whole language.

### Language shortlist assessment (related project's target set)

A companion research project targets this language set:
ada, C, C++, C3, C#, D, dart, go, java, kotlin, lua, mojo, ocaml, odin, python, ruby,
rust, swift, systemC, typescript, V, and zig. Assessed against verification reality
(deductive axis):

**Bucket A — first-class existing support (7).** **Ada** (SPARK), **C** (Frama-C/ACSL,
VeriFast, VCC), **Java** (KeY/JML, VerCors), **Rust** (Verus, Prusti, Creusot, Kani),
**Go** (Gobra), **Python** (Nagini — _requires type annotations_), **OCaml** (Cameleer +
GOSPEL). These cluster on the Why3/Viper IVLs — which is exactly why a unified tool is
feasible.

**Bucket B — possible in a unified tool (10), mainly a front-end + semantics effort.**
Cheap wins (managed/typed): **C#** (dead precedent Spec#; CIL front-end feasible),
**Dart**, **Swift** (types + value semantics + ownership), **Kotlin** (managed, typed,
null-safe — verify at JVM-bytecode level or via a Viper front-end; medium because of
coroutines / smart casts / reified generics). C-like systems (need Viper-style
separation logic): **C3**, **Odin**, **Zig**, **D**, **V**. Gated by JS runtime
semantics: **TypeScript** (its type system helps; the underlying JavaScript is the mess).

**Bucket C — impractical (5), and why.** **C++** (see above). **SystemC** — _is_ C++ (an
electronic-system-level modeling library), so it inherits everything; its real home is
_hardware model checking_ (the temporal axis, not deductive). **Lua** and **Ruby** —
dynamically typed, no static structure; only buildable if a typed dialect
(Luau/Teal/Sorbet) is required first. **Mojo** — immature, unstable, semi-proprietary
moving target; promising ownership design but not pin-down-able yet ("wait, not no").

**Three fault lines that decide the buckets:**

1. **Static typing is the price of admission** — every impractical dynamic language fails
   for the same reason; Python only qualifies _via_ mandatory annotations.
2. **Memory model sets the front-end cost** — managed languages are cheap; every
   manual-memory systems language needs the _same_ reusable separation-logic machinery
   (argues for the **Viper** IVL if we go unified).
3. **SystemC belongs on the temporal axis** — a reminder the project likely wants both a
   deductive engine _and_ a model-checking engine, with the requirement layer routing
   properties to whichever fits.

## Architecture Direction: Layered Hybrid (decided)

**The question.** Should the project be a _scattered_ set of best-of-breed tools (a
different verifier per language, plus bespoke tooling for the rest), or _one unified,
extensible_ tool built on an existing IVL?

**Reframing.** The two options aren't the real choice. The project's actual contribution
is the **requirement layer** (expressing provable/falsifiable requirements, LLM-assisted
translation, and routing to a checker); the checkers underneath are largely
interchangeable infrastructure. So the real question is _where unification must live_.

### Option A — scattered / federated (per-language native tools)

- **Pros:** fastest to breadth (reuse mature SPARK/Frama-C/Verus/KeY/Gobra/Nagini/… on
  day one); best-in-class per language; low semantic burden (upstream owns the hard
  parts).
- **Cons:** no common meaning of "proved" (N spec dialects + N logics → non-comparable
  verdicts, which undercuts the whole "provable requirements" claim); permanent
  integration tax (N parsers/toolchains/formats/TCBs/failure modes, glue rots); ragged
  expressiveness (collapses to the intersection); you still build from scratch for
  uncovered languages _anyway_; cross-language requirements ≈ impossible; the LLM must
  target N formalisms.

### Option B — unified extensible tool on one IVL (Why3 or Viper)

- **Pros:** one semantics of "proved" (single requirement language, logic, and
  verdict format); one trust story / TCB; extensibility first-class (add a language = a
  front-end; separation-logic machinery reused across all manual-memory languages);
  cross-language requirements become conceivable; the LLM targets _one_ formalism; still
  reuses the hard-won IVL core (not writing the prover).
- **Cons:** slower to breadth (one language before many; front-ends are person-months);
  leaves mature tools on the table (re-deriving what SPARK already nails); you own the
  faithful-semantics burden per language; **IVL ceiling risk** (Why3/Viper limits around
  concurrency, higher-order, temporal); and it isn't fully unified anyway — the temporal
  axis needs a separate engine regardless.

### Decision — layered hybrid: unify the _meaning_, federate the _engines_

Deciding principle: **unify the thing that must mean one thing — the requirement and its
verdict — and be pragmatic about the engines underneath.**

1. **Unified requirement layer + one notion of proof, from day one.** Non-negotiable;
   it's the project's identity. A requirement means one thing; a verdict is _proof /
   counterexample / honest-unknown_ in one format.
2. **One primary IVL as the workhorse — Viper.** Covers the largest chunk of the
   _buildable_ set with shared separation-logic machinery; Prusti/Nagini/Gobra already
   prove the "add a language = add a front-end" template.
3. **Delegate to a mature native tool as a normalized back-end** where it clearly
   dominates (e.g. Ada → SPARK) — _only if_ its result maps into the uniform verdict
   model. Federation stays _behind_ the uniform layer, never in front of it (no N spec
   dialects exposed to users or the LLM).
4. **A second engine for the temporal axis** (NuSMV/mCRL2 lineage) sits behind the _same_
   requirement layer for CTL / SystemC-style properties. "Unified" = unified interface,
   not one engine.
5. **Sequence it.** Prove the whole architecture end-to-end on **one clean language
   first** (Rust via a Viper front-end, or a managed language like Dart/C# for lower
   semantic drag), then add front-ends and native-tool back-ends.

One-line test for any design choice here: _does a requirement — and the answer to "does
it hold?" — mean the same thing regardless of which engine ran?_ If yes, scattered
engines are fine; if no, it isn't really provable requirements.

## Design Q&A (living notes)

A running log of questions worked through and their distilled answers. Refined as we
drill down; fuller treatment lives in the sections above.

- **Should an LLM be part of an automated pipeline?** Yes — but strictly as an
  _untrusted front-end_ that translates text requirements into formal statements, with a
  trusted checker validating the output. See _Proposed Workflow_.
- **Do foundational provers (Lean 4, Coq, Isabelle/HOL, Agda) all share a code-to-proof
  gap?** Yes, fundamentally — they reason about objects defined in their own logic, so
  real code must be modeled inside the tool. Mitigations: write the program _in_ the
  prover (no gap), or bridge via extraction / embedded semantics (Coq→CompCert,
  Isabelle→seL4) at real cost.
- **Which languages do the code verifiers cover?** Verus → real **Rust** in place;
  Dafny → its own language, compiling to C#/Java/Go/Python/JS; F\* → its own ML-family
  language, extracting to OCaml/F#/C/Wasm.
- **Do any of these subsume `ctl` / `go-ctl2`?** Only true CTL model checkers:
  **NuSMV/nuXmv** (native CTL + LTL) and **mCRL2** (μ-calculus, superset of CTL\*).
  TLA+/Alloy cover the same _use case_ but in linear-time logic, not branching-time CTL.
- **Is Z / Z3 useful here?** Two different things. **Z** = a design-level specification
  language (set theory + schemas), weak automation, superseded for our goals by Event-B
  (refinement) or Alloy (automation). **Z3** = the SMT solver already underpinning
  Verus/Dafny/F\*/Apalache — infrastructure, not a competitor.
- **What about C/C++, Java/Kotlin, and D/Dart/V/Zig?** C: strong (Frama-C/ACSL). C++:
  weak, subsets only (full deductive C++ ≈ open problem). Java: strong (KeY/JML,
  VerCors). Kotlin: essentially none. D/Dart/V/Zig: no real deductive verifiers — open
  territory. See _Coverage by language_.
- **Why is C++ specifically so weak?** Its semantics are hostile to sound verification —
  pervasive undefined behavior, a brutal memory/object-lifetime model, Turing-complete
  templates, implicit exception/destructor control flow, and a research-grade concurrency
  model. Rust verifies well because it was co-designed for it; C++ chose the opposite
  trade. **Decision:** deprioritize C++; revisit later only as a constrained verifiable
  subset (SPARK-style). See _Why C++ is "weak (subsets only)"_.
- **Of the companion project's 22 languages, what's supported / buildable / impractical?**
  First-class today (7): Ada, C, Java, Rust, Go, Python, OCaml. Buildable via IVL
  front-ends (10): C#, Dart, Swift, Kotlin, C3, Odin, Zig, D, V, TypeScript.
  Impractical (5):
  C++, SystemC (it _is_ C++; belongs on the temporal/model-checking axis), Lua, Ruby
  (dynamic typing — need a typed dialect), Mojo (immature moving target). Deciding
  factors: static typing (price of admission), memory model (front-end cost), and the
  deductive-vs-temporal axis. See _Language shortlist assessment_.
- **Can we express "no queue message is ever lost"?** Yes — it decomposes into a
  _conservation_ safety invariant (every message always in exactly one of
  in-flight/retrying/succeeded/dead-lettered) + a _disposition_ liveness property (every
  accepted message eventually succeeds or is dead-lettered-with-reason). Retry loops need
  bounded retries (preferred) or a stated fairness assumption to avoid livelock. It's a
  2a/2b system property (TLA+ + runtime reconciliation). Caveat: exactly-once is
  impossible; "no loss" = at-least-once + idempotency. See _Worked example: message
  reliability_.
- **Can requirements "become models" that are then checked?** Yes, soundly — but keep the
  _model_ and the _property_ independent, or verification is vacuous/circular. Four
  readings: standard `M ⊨ φ` (separate); generate model from requirements (go-ctl2 style —
  the trap); **refinement (Event-B/TLA+)** — refine a requirement-model toward code
  preserving guarantees (richest, bridges 2a→1); synthesis/GR(1) (advanced). The LLM may
  propose the model but the property stays an independent trusted check. See
  _"Requirements become models, which are then checked"_.
- **Can we express concurrency/parallelism requirements (deadlock-freedom, mutual
  exclusion, ordering)?** Yes — concurrency is the best-served formal-methods domain. All
  are _temporal_ (safety vs liveness): mutual exclusion = safety; progress/no-deadlock =
  liveness + reachability; ordering = safety/protocol. Checkable in category 1 (CSL,
  Rust ownership, VerCors/VeriFast, Iris; session types/typestate for ordering), 2a
  (SPIN/TLA+/NuSMV/mCRL2/CSP+FDR — home turf, built-in deadlock detection), and 2b
  (ThreadSanitizer, watchdogs, RV monitors). Caveats: state explosion, liveness needs
  fairness, dynamic detectors have false negatives. Language must support temporal
  operators + fairness + atomic events. See _Concurrency & parallelism requirements_.
- **Are code, system-behavior, and UI requirements the same kind of claim?** No — they
  sit on one epistemic spine (proof → model-checked → monitored/tested), trading
  universality for fidelity. Four categories: (1) **Code** (deductive, proof ∀), (2a)
  **System design-time** (model checking, proof over a model), (2b) **System runtime**
  (runtime verification/monitoring, empirical), (3) **UI** (Selenium/driver, empirical).
  Temporal logic is the shared spec for 2 (and much of 3); the layers cross-check (2b
  closes the design gap). Every requirement must carry its modality + verdict strength;
  never conflate a Selenium green with a proof. See _Requirement Categories_.
- **Scattered per-language tooling, or one unified extensible tool?** Neither in its pure
  form — **layered hybrid (decided).** Unify the _requirement layer and the meaning of a
  verdict_; federate the _engines_ behind it: one primary IVL (**Viper**) as workhorse,
  mature native tools (e.g. SPARK) as normalized back-ends, plus a model checker for the
  temporal axis — all behind one uniform interface. Sequence: one clean language
  end-to-end first. See _Architecture Direction: Layered Hybrid_.
- **How big a deal to build a wider-spectrum, extensible tool?** Not "invent something
  new" — adopt the **IVL pattern** (Why3 or Viper): reuse the solved core (VC generation,
  SMT back-ends) and write a _faithful per-language front-end_ per language. Front-ends
  are the real cost (tractable for clean languages, brutal for C++). Keep the requirement
  layer IVL-agnostic; consider _constraining_ a language to a verifiable subset (SPARK
  precedent). Note this is the _deductive_ axis, distinct from the _temporal/CTL_ axis —
  a general tool likely needs a requirement layer that routes properties to the right
  engine.

## Design Documents

- [docs/requirement-language.md](docs/requirement-language.md) — design of the unified
  requirement language (PRL). Tracked in issue #2.

## Development

A [dev container](.devcontainer/README.md) provides the toolchain (git, `glab`, Doorstop,
`uv`, and the Markdown/YAML linters and formatters). Open the repo in VS Code and
**"Dev Containers: Reopen in Container"**, then use the Makefile:

```sh
make check-tools           # verify the toolchain is present
make fmt                   # format Markdown + YAML (prettier)
make lint                  # markdownlint + yamllint
make check-requirements    # validate the Doorstop tree
make traceability          # requirements-to-code traceability report
make pre-merge             # full local preflight (there is no CI)
make setup-hooks           # install the pre-commit gate
```

Requirements are managed with Doorstop in [`requirements-doorstop/`](requirements-doorstop)
(there are none yet). See [.devcontainer/README.md](.devcontainer/README.md) for details.

## Status

Brainstorming. No code yet — ideas, notes, and direction come first; implementation
follows once the concepts are sharp enough to build on.
