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

### Where REQ047 landed (2026-08-02)

The sequence terminated. `#148` shipped as REQ065 and the instantiability wall came down, so **REQ047
now reaches a verdict about this repo**:

```text
REQ047: holds — model-checked (bounded)
    - Kani: holds (model-checked (bounded))
    - Creusot: inconclusive   (the compiler crashed translating the subject — the async ICE, #153)
    - Prusti: inconclusive    (its pinned 2023 toolchain cannot read this dependency graph, REQ063)
```

That is the first requirement verified end to end **about this repo**, which the second pass listed
as its largest gap. The gap it closes is narrower than it looks: `holds` here is Kani's bounded
basis, not `proven`, and the two deductive engines are still reporting the standing ceilings named in
REQ063/REQ067 rather than reading the claim. Both now say which of the two it is, in the operator's
terms, instead of handing over a tool error — that is the REQ062–067 arc, and REQ047 is where its
absence was first felt.

`proven` itself arrived elsewhere, on the mirror channel (REQ068), and had to: provreq cannot be its
own Creusot subject while the async ICE stands, so the proof runs against an in-repo fixture. See the
mirror-channel section in `operator-workflow-notes.md`.

**Still uncovered by any dogfood pass:** the living loop's five drift axes against real data, and the
web UI driven against this subject. Both have been open across all three passes.

## Third pass (2026-08-02) — the mirror channel, walked end to end

Not a self-adoption pass: provreq cannot be its own Creusot subject while #153 stands, and the
mirror channel is Creusot's. So this walked a **fresh subject** the way an operator would, start to
finish, with a live model and the real prover — `init` → `triage` → `--translate` → `--set` →
`--admit` → `--ground` → `verify --draft-semantic --repair`. The point was to exercise REQ062–068 as
one journey rather than in pieces, since every piece had only ever been validated on its own.

The subject (`gatekeeper`, two modules, deliberately Creusot-translatable — no `format!`, no `async`):

```rust
pub enum Session { Anonymous, SignedIn { failures: u32 }, LockedOut }
impl Session { pub fn is_trusted(&self) -> bool { … } }          // src/session.rs

pub enum Access { Granted, Refused, NeedsReview }
pub fn decide(session: &Session, flagged: bool) -> Access { … }  // src/access.rs
```

and the requirement: *a request is granted only from a trusted session*, grounded
`trusted=Session::is_trusted`, `granted=decide::Granted`, `Sess=Session`, `Flag=bool`.

### What worked in the third pass

- **Grounding read the subject correctly first time**, across two modules, including the
  inherent-method form and the enum-variant form, and said in each read-back exactly how it would
  check the predicate and what it could not see (`syn` sees no types).
- **All three engines gave honest, actionable inconclusives** at the baseline, each about its own
  limit: Kani named `Session`'s missing `kani::Arbitrary` as a subject-side precondition and offered
  both ways out (REQ065); Prusti named its toolchain ceiling (REQ063); Creusot named the
  call-in-logic-context wall. That is the REQ062–067 arc doing its job on a subject it had never
  seen.
- **The read-back caught a mis-formalization before admission.** Asked to translate the prose, the
  live model proposed a **category 2a + 2b** requirement — TLA+ and MonPoly — for a pure Rust
  subject with neither a spec nor an event stream. The gate passed it (it is well-formed PRL), and
  the read-back stated the category and the expected evidence plainly enough to reject it on sight.
  The formalization written by hand as category 1 is what went on to admission.

### Finding 1 — a dropped mirror is invisible ([#170](https://github.com/greeng3/provablerequirements/issues/170))

Two predicates resolved; **one** mirror was staged, for `is_trusted`. Nothing was staged for
`decide` — the function named in the baseline error — and nothing was said about it. The captured
harness shows the consequence:

```rust
proof_assert! { forall<s: crate::session::Session, f: bool>
  (!(match crate::access::decide(&s, f) { crate::access::Access::Granted { .. } => true, _ => false })
   || crate::session::is_trusted_logic(&s)) };
```

`with_mirrors` skipped `decide` correctly — it looks for `fn decide_logic`, which was never staged —
so the program call stayed in pearlite and the run failed for the reason it started with.

`Mirrorer::draft` drops a target on three paths, each a bare `continue`: an unusable model reply, a
malformed item, or a link provreq cannot build. **Dropping is right** — an unlinked mirror is an
unchecked meaning, the exact false-`proven` hazard the channel exists to prevent. Saying nothing is
not: the operator gets a count of what was staged, which reads as completeness.

### Finding 2 — the Creusot hint recommends what #158 forbids ([#171](https://github.com/greeng3/provablerequirements/issues/171))

Every Creusot build failure still ends "…the predicate may need `#[logic]`", including the
call-in-logic-context error the mirror channel exists to answer. Since #158 that is the one action
that leaves the subject unable to compile in any configuration.

### Finding 3 — a seeded triage classification cannot be told from a real one ([#172](https://github.com/greeng3/provablerequirements/issues/172))

With no provider configured, `triage` seeds the prose-floor default and says so as it runs — but
persists `classification: stays-prose` with no trace that no classifier ran. Here the seed was
**wrong**: configuring the model and re-running gave `formalizable-now`. `stays-prose` means *this
will not be formalized*, and the coverage funnel counts it.

### Friction, not defects

- `--ground` cannot be repeated, so a four-symbol requirement takes four invocations.
- `llm.provider` is required and has no example anywhere; the parse error names the field, which is
  how it was recovered.

### Where this leaves REQ068

The channel proved a claim over ordinary program functions on the in-repo fixture, and CI holds it
there. On a subject it had not seen, it reached the same wall and covered only half of it. The
mirror mechanism is not what failed — the one mirror it staged was correct, linked, and redirected.
What failed is that the tool knew it had given up on the other half and did not say so, which is the
one failure mode this project treats as never acceptable.

### Follow-up: why the mirror was dropped (#170)

Making the drop visible was the fix asked for. Looking for what to report exposed the cause, and it
was provreq's, not the model's.

`parse_mirror` **required the reply to contain an `#[ensures…]` line** and returned nothing without
one — while the caller destructured that line away unread, because provreq builds the real link
itself from the signature (`link_for`, added precisely because a model gets the link wrong). The
prompt justified the requirement as marking where the item ends, which was never true either: the
item is taken through its own balanced brace, and `#[ensures]` lines are explicitly skipped in that
scan. So a mirror that was well-formed, correctly named and perfectly linkable was refused over a
clause nothing consumed — and refused in silence.

A test encoded the belief: `a_mirror_without_its_link_is_refused`, reasoning that "an unlinked
mirror is exactly the unchecked assertion this channel exists to avoid". That confuses the model's
`#[ensures]` line with provreq's own. The mirror was never unlinked. The test now asserts the
opposite.

Re-run against the same subject, same model, after the fix:

```text
--draft-semantic: staged 2 logic mirror(s) for REQ001 — REVIEW THESE FIRST:
  src/access.rs:12   → decide_logic
  src/session.rs:10  → is_trusted_logic
```

Both predicates mirrored, including `decide` — the function the prover had named and the channel had
abandoned. The claim still does not discharge, now for an honest and different reason: the drafted
`is_trusted_logic` writes `failures == 0` where matching a `&Session` binds `failures: &u32`. That is
ordinary mirror-quality variance, the case the design is built to survive — nothing false is proved,
and the mirror is staged for the operator to correct. Where the operator is *pointed* is a separate
defect, filed as [#174](https://github.com/greeng3/provablerequirements/issues/174): a type error
inside a staged mirror is reported as "the proof harness did not compile", sending them to a
generated file they cannot edit instead of the line in their own tree.

### Follow-up: the Creusot compile message (#171, #174)

Both were one function, `build_error`, which had the full rustc diagnostic and used one line of it.

It now reads the `--> file:line:col` it was discarding and says **where** the failure is. The two
cases are different work for the operator, so they read differently:

```text
the proof harness provreq generated did not compile — error: called program function
`access::decide` in logic context (src/provreq_req001.rs:11:74). Pearlite may only call `#[logic]`
functions, and that is an ordinary program function — marking it `#[logic]` is not the fix either,
because that removes the item from the program and breaks every call site. Reach it through a
`#[logic]` mirror instead, which leaves the function untouched: re-run `verify` with
`--draft-semantic`
```

```text
the subject did not compile under Creusot — error[E0308]: mismatched types, at src/session.rs:27:59.
That is the subject's own source, not the generated harness — if a draft was just staged there, it
is the staged edit that needs fixing, and the line above says which
```

Both measured on the same subject, before and after, by staging the mirrors and then removing them.
The second names the exact line the model got wrong (`failures == 0` against a `&u32`), which is
what the operator needs and what the old wording hid.

The `#[logic]` recommendation is gone. It fired on **every** compile error — including plain type
mismatches, where provreq had established no cause at all — and since #158 it is the one action that
leaves the subject unable to compile in any configuration. A test asserted its presence
(`must point at the fix`); that test now asserts its absence, and two more pin the site reporting.

### Follow-up: a seed is recorded as a seed (#172)

`TriageEntry` gains an `origin`, and the four writers of a classification are now distinguishable
because they mean different things: a classifier **judged** it, the prose floor **seeded** it, the
**operator** set it, or it predates this field and its provenance is **unrecorded**. That last one
is not a formality — defaulting an old entry to either real value would assert a provenance nobody
recorded, which is the over-claim the field exists to stop.

The consequence that matters is in `plan`: an ordinary run now re-does the **seeded** items as well
as the untriaged ones. A seed overwrites no judgement, so there is nothing to gate. `--reclassify`
keeps its meaning and its consent prompt — it replaces judgements — but is no longer the only way
out of a backlog seeded before a provider was configured. That choice, between keeping nothing and
re-running everything, was the real cost of the defect.

Measured end to end on the same subject, in the three states that matter:

```text
# the pre-#172 file, which must still load and must claim nothing
REQ001       formalizable-now   (origin not recorded)

# no provider configured
No `llm:` config in provreq.yml — seeding 1 of 1 item(s) with the prose-floor default. A seed is
recorded as a seed, not as a classification: configure a provider and re-run `provreq triage` and
these are re-done, with no `--reclassify` and nothing else of yours touched.
REQ001       stays-prose        (seeded — no classifier ran)
    -> triage.yml: classification: stays-prose / origin: seeded

# provider configured, PLAIN run — no --reclassify
Classifying 1 of 1 item(s) with qwen3:32b …
REQ001       formalizable-now
    -> triage.yml: classification: formalizable-now / origin: classified
```

The third step is the one the third pass could not reach: the seed said `stays-prose`, meaning *this
will not be formalized*, and the model said `formalizable-now`. The annotation disappears once the
bucket is a real classification, because a classification needs no annotation — only the states
carrying less than they appear to do.

---

## Fourth pass (2026-08-03, issue #178) — the living loop and the web UI

The two things every previous pass listed as uncovered. Same subject as the third pass
(`gatekeeper`), same method: drive it as an operator, with the live model and the real prover.

### First `proven` on a subject provreq had never seen

The third pass ended with the mirrors staged and the claim undischarged. Taking the operator's next
step — reading the drafted mirror and correcting it — finished the job:

```text
REQ001: holds — proven: established deductively for every execution (spec-relative), not just the
states a bounded checker explored
    - Kani: inconclusive    (Session cannot be instantiated — REQ065)
    - Creusot: holds (proven)
    - Prusti: inconclusive  (toolchain ceiling — REQ063)
```

Every `proven` before this was on the in-repo fixture, which exists because #153 blocks provreq from
being its own Creusot subject. This one is a subject the tool had never seen, reached through the
whole journey: `init` → `triage` → `--set` → `--admit` → `--ground` → `--draft-semantic` → review →
`verify`.

It took **two** corrections to the drafted mirror, and the second is the finding
([#181](https://github.com/greeng3/provablerequirements/issues/181)): the model wrote
`failures == 0`, which is wrong both because matching a `&Session` binds `failures: &u32` **and**
because an unsuffixed literal in pearlite is `Int`. Correcting the obvious half produced the same
error class one column later; only `*failures == 0u32` compiles. `PEARLITE_RULES` mentioned neither,
and a mirror is drafted once by design, so every miss is an operator correction.

Both are now stated in `PEARLITE_RULES`, and re-running this same journey on a fresh copy of the
subject reached `holds — proven` with **nothing corrected by hand**. Fixing them exposed a third
miss of the same kind, in the same place: told the receiver becomes a parameter of "the SAME type
INCLUDING its reference", the model wrote `s: &Self` — which satisfies that literally, since `&Self`
*is* the receiver's type. A mirror is appended at module level, outside the `impl`, so the subject
stopped compiling on its own source (`error[E0411]: cannot find type Self in this scope`). Saying
where the item lands is what rules it out. The pattern worth carrying: each of these is a mechanical
fact about the language the model is writing, invisible to every unit test, and costing a correction
apiece precisely because a mirror gets no repair round.

### The drift axes, driven for the first time

Four of the five, each against a real edit, each read back from the funnel and the API:

| axis | how it was driven | result |
| --- | --- | --- |
| subject commit | an unrelated commit to `src/lib.rs` | `verified 1 → 0`, `stale 0 → 1` |
| requirement prose | edited `reqs/REQ001.yml` | stale, prose axis named |
| formalization | `--set` a new candidate (un-admits) | stale, formalization axis named |
| proving environment | added `environment: lab-1` | stale, `unlabelled → \`lab-1\`` |
| tool version | **not driven** — needs a differently-versioned binary | uncovered |

The reasons are exact and carry both sides of the change:

```text
the subject code moved since this verdict (commit 6e25bdee… → f4b13e6d…) — re-verify
the declared environment changed since this verdict (unlabelled → `lab-1`) — re-verify
```

### Finding — the re-verify worklist is web-only ([#179](https://github.com/greeng3/provablerequirements/issues/179))

`status` says `stale 1`, tells the operator to re-verify by id, and never says which id. The web UI,
for the same subject at the same moment, names the item and every axis that moved.

The sharp case: with the prose restored and only the code moved, `provreq draft` listed
`REQ001 candidate [gate ok] [admitted]` — completely clean — while the funnel said `stale 1`. Those
are different axes (the draft's prose anchor vs the verdict's), and a CLI operator reading the first
would conclude nothing was owed.

### Finding — #172 never reached the browser ([#180](https://github.com/greeng3/provablerequirements/issues/180))

`classified_by` rides on the API row; no UI component reads it. So a seeded classification is still
indistinguishable from a judged one on the surface built for browsing a whole backlog at once. That
is a gap in the #172 change rather than a new defect, recorded so it is not mistaken for done.

### What worked in the fourth pass

The UI, driven in a real headless Chrome against a real subject for the first time, was correct
throughout: the engine panel distinguished available from not-wired, the funnel matched the CLI's
exactly, the row carried `holds ⟳ stale`, and the item detail showed all three drift reasons plus a
`Proved in:` line naming every engine version and the missing environment label. The bulk
`Re-verify all stale (1)` action was present. Nothing on that surface overclaimed.
