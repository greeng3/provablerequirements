# Dogfooding: provreq adopted as its own subject

Record of the first run of provreq against a real repository rather than a hand-built scratch
subject. Subject: this repo, at `0b5a2be`. Issue [#125](https://github.com/greeng3/provablerequirements/issues/125).

Why this subject: it is a Rust cargo project, so the category-1 ensemble genuinely applies, and
`requirements-doorstop/` holds 51 requirements written by a human for a reason — spanning
mechanically checkable claims, runtime-observable ones, and prose that will never be formalized.
That spread is what the triage funnel exists to sort, and nothing hand-built had tested it.

The companion tree `ProvableRequirements-doorstop/` is committed rather than ignored: it is a peer
of the requirements directory by design, so the repo carries its own, and a reader gets a worked
example — including one that honestly parks.

## What the run established

**The verdict path does not overclaim.** Every honesty guarantee the tool advertises held at every
step where it was exercised. Grounding parked with a precise reason instead of guessing; `verify`
returned `unknown (missing-grounding)` with full provenance instead of a verdict; the funnel
counted an ungrounded formalization as formalized-but-not-verified. Nothing was fabricated
anywhere in the chain from requirement to verdict.

**Every defect found is in the LLM triage path, and all three are the same disease** — reporting
work that was not done:

| Issue | Defect |
| --- | --- |
| [#127](https://github.com/greeng3/provablerequirements/issues/127) | A failed LLM triage is reported as a confident all-prose classification of the whole backlog |
| [#128](https://github.com/greeng3/provablerequirements/issues/128) | Bulk triage is one all-or-nothing request; 51 items ran 10 minutes and lost everything |
| [#126](https://github.com/greeng3/provablerequirements/issues/126) | `triage` announces an LLM classification it does not perform |

That clustering is itself a result. The parts of the tool that were designed around the honesty
thesis hold up under contact; the part that delegates a judgement to a model is where the thesis
had not been carried through.

**One modelling gap, which is not a bug:**
[#129](https://github.com/greeng3/provablerequirements/issues/129) — category-1 grounding binds a
predicate to a function written `-> bool`, but this subject (like most idiomatic Rust) expresses
decisions as enums. The whole codebase has 11 bool-returning public free functions; everything
carrying a decision is an `InstallDecision`, `EngineStatus`, `VerifyOutcome`, `GateStatus`. The
more carefully a subject models its states, the less of it the binder can reach.

**One papercut:** [#130](https://github.com/greeng3/provablerequirements/issues/130).

## The run, step by step

### 1. Adopt

```text
$ provreq init .
Discovered Doorstop layout under .:
  ./requirements-doorstop (REQ) — 51 item(s)
Proposed companion tree: ./ProvableRequirements-doorstop
Create companion tree? [y/N] Aborted; nothing written.
```

Discovery, name derivation and the confirmation gate all behaved as specified. Declining wrote
nothing. `--yes` then scaffolded the tree.

### 2. Triage

Seeding with no `llm:` block classified all 51 items `stays-prose` — the honest floor, and
honestly labelled as such. Worth noting that the floor is maximally pessimistic by construction:
the funnel starts by telling the operator nothing, which is truthful but is also the state the
bulk pre-sort exists to escape.

Configuring a real model and re-running produced the three defects above. The genuine attempt ran
10m17s against `qwen3:32b` and failed with a timeout — loudly and correctly (REQ042), leaving the
triage state untouched. That correct failure is the only reason the run did not silently produce
a fabricated classification.

Two items were then triaged by hand to carry on.

### 3. Formalize

REQ047 — *"on a platform the engine's upstream does not support, the tool reports that plainly
... rather than attempting an install that cannot succeed"* — formalized as a category-1
invariant:

```text
requirement kani_install_platform_gated { category: 1
  vocabulary { state platform_supported
               state install_proceeds(d, p, q, c) }
  require { always (not install_proceeds(d, p, q, c) or platform_supported) } }
```

Gate: PASSED (clean). The D12 read-back rendered it faithfully:

```text
Requirement `kani_install_platform_gated` — category: code (1).
It requires that:
  • ((not install_proceeds) or platform_supported) always holds
```

Admitted as `admitted-but-ungrounded`.

### 4. Ground

```text
platform_supported → `kani_platform_supported` resolves to src/provision.rs:246
install_proceeds:   `decide_install` at src/provision.rs:79 returns `InstallDecision`, not `bool`
                    — a state predicate must be a boolean over program state
```

Parked. This is [#129](https://github.com/greeng3/provablerequirements/issues/129): the invariant
is true of the code and provable in principle — `decide_install` returns `UnsupportedPlatform`
exactly when the platform is unsupported — but the predicate is a variant test, and grounding can
only name a function.

Also worth recording, because it is the tool teaching rather than failing: declaring the predicate
with the wrong arity first produced

```text
`decide_install` at src/provision.rs:79 takes 4 parameter(s), but the requirement declares
install_proceeds with 0 — one of the two is wrong
```

which names both candidates for the error instead of assuming which side is at fault.

### 5. Verify

```text
REQ047: unknown (missing-grounding) — the requirement is not grounded; no engine can run until
        every symbol binds to a confirmed observable
    - install_proceeds: `decide_install` returns `InstallDecision`, not `bool`
  provenance: requirement@17cf6ac8 subject@0b5a2be7 provreq@0.0.1
```

The honest refusal, carrying the reason and the provenance. No engine ran, and none should have.

### 6. Living loop

Not exercised: nothing reached a stored verdict, so there was nothing to drift. This is the one
step of the spine the run did not reach, and it stays open for a follow-up once
[#129](https://github.com/greeng3/provablerequirements/issues/129) lets a real requirement ground.

## What this did not cover

- No requirement was verified end to end, so the living loop and the five drift axes were not
  exercised against real data.
- Only two of 51 items were triaged, both by hand.
- The web UI was not driven against this subject; only the CLI was.

---

## Second pass (2026-07-29, issue #143)

The first pass ended parked at grounding, on a predicate the binder could not name. Four slices
later — enum/method binding ([#129](https://github.com/greeng3/provablerequirements/issues/129) /
REQ055), the parameter-type cross-check
([#118](https://github.com/greeng3/provablerequirements/issues/118) / REQ057), primitive sorts
([#140](https://github.com/greeng3/provablerequirements/issues/140) / REQ058), and free-variable
closure ([#136](https://github.com/greeng3/provablerequirements/issues/136) / REQ059) — REQ047 was
supposed to become the first requirement this repo proves about itself.

It did not. It got two steps further and stopped somewhere more interesting.

### What worked

**The read-back named the gap before anything ran.** The candidate as committed declares no
parameter types, and REQ059's closure says so out loud:

```text
• for each d (no declared sort) and each p (no declared sort) and each q (no declared sort)
  and each c (no declared sort), ((not install_proceeds(d, p, q, c)) or platform_supported)
  always holds — every variable the claim mentions is quantified, over the sort the vocabulary
  declares for it
```

Declaring the sorts (`install_proceeds(d: EngineState, p: Flag, q: Flag, c: Flag)`) and binding them
(`EngineState=EngineStatus`, `Flag=bool`) grounds the requirement — a four-argument predicate, an
enum variant test, a `&`-taking parameter, and a primitive sort, all confirmed against real code:

```text
EngineState (sort) → `EngineStatus` resolves to src/engine.rs:67  pub enum EngineStatus {
Flag (sort) → `bool` is the Rust primitive `bool` — the language's own type, not one the subject
    declares, so there is no source location to confirm it against
install_proceeds → `decide_install::Proceed` resolves to src/provision.rs:79
    (checked as `matches!(decide_install(…), InstallDecision::Proceed)`)
REQ047: GROUNDED — every symbol binds to a confirmed observable.
```

REQ057's parameter-type cross-check passed on real code here, silently and correctly: `d` ranges
over `EngineState`→`EngineStatus` against a parameter written `&EngineStatus`, and `p`/`q`/`c` over
`Flag`→`bool` against three `bool` parameters.

### Finding 1 — resolution walked the build directory (fixed here, REQ060)

The first dry-run **timed out at two minutes**. `adopt` and `doorstop` have always pruned `.git`,
`target`, `node_modules`, and `.venv`; the two *adapters* that resolve bindings never did, so every
resolution traversed this repo's 2.6 GB `target/` — for 57 source files.

Four walks with four opinions about which files count. Now one rule, `subject_tree::is_pruned_dir`,
shared by all four: prune by name, **and** prune any directory carrying a `CACHEDIR.TAG` with the
standard signature (which cargo writes into `target/`, and which catches a `CARGO_TARGET_DIR`
pointed somewhere a name list can never anticipate).

Not only speed: a name declared under a build directory is a copy or a generated artifact, and
finding it would park a correct binding as `Ambiguous` against a file the operator never wrote.

**34 s → 4.5 s** for one dry-run. The residual is a separate papercut — each lookup re-parses every
file in the subject, roughly ten full parses for REQ047's four bindings —
[#144](https://github.com/greeng3/provablerequirements/issues/144).

### Finding 2 — lowering ignores the module path ([#145](https://github.com/greeng3/provablerequirements/issues/145))

The harness that grounding earned:

```rust
let d: provreq::EngineStatus = kani::any();
assert!(!(matches!(provreq::decide_install(&d, p, q, c), provreq::InstallDecision::Proceed { .. }))
        || provreq::kani_platform_supported());
```

```text
error[E0412]: cannot find type `EngineStatus` in crate `provreq`
error[E0425]: cannot find function `decide_install` in crate `provreq`
```

The items are at `provreq::engine::EngineStatus` and `provreq::provision::decide_install`.
`lowering::qualify` writes `{prefix}::{name}` with no module path — correct for every scratch
subject cat-1 was validated against, because each had a flat `src/lib.rs`, and wrong for the first
real multi-module crate it met. Verdict: `unknown (inconclusive)`, carrying the compiler error.
Honest, and from the wrong surface.

**This is the pattern the last three slices each hit in their own way**: a binding confirmed at
grounding, then a harness that cannot be built, so the operator learns from rustc what the tool
already knew. REQ057 fixed it for parameter types, REQ058 for primitive prefixing, and this is the
same shape again for module paths.

### Finding 3 — the requirement's formalization says something the code does not guarantee ([#146](https://github.com/greeng3/provablerequirements/issues/146))

The most valuable finding, and it would have stayed hidden if #145 had not stopped the run.

REQ047 is about `decide_install`'s **second argument**: it returns `Proceed` only when
`platform_supported` is true. Written directly that is `always (not install_proceeds(d, p, q, c) or
p)` — and `p` alone is parsed as an atom with no arguments, so the gate rejects it as an undeclared
predicate. **A PRL atom is always a predicate applied to terms, and a predicate binds to a function
of the subject**, so there is no way to say "this boolean variable is true".

The committed formalization works around that by binding a nullary `platform_supported` to
`kani_platform_supported()` — which reads the *host OS*. That is a different proposition, and the
resulting claim is not true of `decide_install`: nothing stops a caller passing
`platform_supported = true` on an unsupported host. It is true only of the call site that composes
them, and that call site does I/O, so the cat-1 fragment cannot reach it.

So REQ047's formalization has never been checkable, and the moment it becomes checkable it will
report `fails` about a claim nobody means.

**And the validation subject hid it.** The scratch subject used to prove out REQ059 contained
`pub fn is_supported(s: bool) -> bool { s }` — an identity function existing purely so a boolean
argument could be named as a predicate. That is the tool bending the subject, the exact thing #129
rejected, and writing it is what made the closure work look complete.

### Where REQ047 stands

Left as the pass produced it: sorts declared and bound, grounded, `unknown (inconclusive)`. The
formalization is **not** silently corrected to something weaker-but-provable — choosing an
expressible sub-claim is a real option, but it narrows what the requirement asserts, and that is the
operator's call to make knowingly rather than the tool's to make quietly.

### What this pass did not cover

- Still no requirement verified end to end about this repo, so the living loop and the five drift
  axes remain unexercised against real data.
- Creusot and Prusti reported their standing subject-side inconclusives (no `creusot-std`
  dependency; Prusti's pinned nightly cannot read a v4 lockfile), so Kani was the only engine that
  reached the claim.
- The web UI was again not driven against this subject.

### Follow-up: module paths fixed (#145 / REQ061)

`lowering` now names every item through the module the adapter found it in, so the harness for
REQ047 against this repo writes `provreq::provision::decide_install` and
`provreq::engine::EngineStatus`. The module comes from where the item was **resolved**, not from a
convention applied at generation time, and each named thing carries its own placement — the enum a
variant test checks need not live where the function does.

Three things this deliberately does not paper over:

- An item in a separate crate target (`tests/`, `benches/`, `examples/`) or a binary still
  **resolves** — it really is declared there — but no path a harness can write reaches it, so
  lowering declines and names the file. Existence and reachability are different questions, and
  confirming the first does not answer the second.
- Two enums of one name are now an `Ambiguous` park rather than a pooled variant list. Each would be
  reachable by a different path, so pooling described something that exists in neither place.
- Nothing guesses at `pub use` re-exports or `#[path]` attributes; `syn` cannot resolve either, and a
  subject that relies on them degrades honestly.

**The verdict moved one stage further, to a third real reason:**

```text
error[E0277]: the trait bound `provreq::engine::EngineStatus: kani::Arbitrary` is not satisfied
```

`EngineStatus` carries an `Available { version: String }` variant, and Kani needs a *value* for every
quantified variable. Nothing is wrong with the binding, the sort, or the claim — this is Kani's own
precondition, and a deductive engine's logical `forall` would not need it. Tracked as
[#148](https://github.com/greeng3/provablerequirements/issues/148), which is about saying so in the
operator's terms instead of handing them the trait error.

So REQ047 has now been `unknown` for three distinct reasons in sequence — the free-variable gap
(#136), the module path (#145), and instantiability (#148) — and every one of them was real. That
sequence is what a tool that refuses to guess looks like from the outside.
