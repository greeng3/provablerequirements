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
