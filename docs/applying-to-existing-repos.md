# Applying PRL to an Existing Repository — Design

How PRL attaches to a real project whose requirements already live in
[Doorstop](https://doorstop.readthedocs.io/). **qrusty** is the motivating example, not the
only target: the design is **subject-agnostic** (any Doorstop-using repo), **forge-agnostic**
(the subject may live on GitLab, GitHub, or a private on-prem git server), and does not
assume its operator is this repo's author. This records where PRL and its derived artifacts
live, how the tool is deployed and driven, and how a human uses it. Complements
[requirement-language.md](requirement-language.md) (the language itself) and the
[README](../README.md) workflow (the trust boundary).

Status: **design settled.** Decisions below; deferred-to-implementation items at the end.

## Guiding principle

**Requirements and their proofs live with the code they constrain.** The tool is central
and subject-independent; the artifacts it produces are co-located with the subject and
version atomically with it. Just as the Doorstop requirements live inside the subject
repo, so do the PRL requirements, groundings, and verdicts derived from them.

## A1 — Doorstop items are the untrusted NL front-end input

Each Doorstop item's prose _is_ the natural-language input the D11 pipeline already
assumes. PRL does not replace Doorstop; it **formalizes a selected subset** of items and
attaches back to them **by Doorstop ID**. The item is the anchor; the formal requirement,
its grounding, and its verdict hang off that ID.

**Attachment is many-to-many, stored minimally.** One prose item routinely bundles several
claims ("no message lost _and_ delivered in order") and one formal invariant can span
items — forcing 1:1 would mangle prose or drop claims. So each PRL requirement carries a
`derives_from: [QRUS042, …]` list (usually one element); the reverse item → requirements
view is _computed_, not a stored join. Full expressivity, one field, no extra bookkeeping.

```text
Doorstop item QRUS042 (prose: "no accepted message is ever lost")
  → [LLM forward, untrusted]   → candidate PRL requirement
  → [mechanical gate] → [read-back] → [human confirm]   (D11/D12)
  → admitted PRL, linked to QRUS042
  → ground to the subject's real code / telemetry → lower → engine   (D4–D6)
  → Verdict{ holds | fails | unknown, strength, provenance{commit …} }   (D7–D10)
  → verdict attached back to QRUS042 as evidence
```

## A2 — Adoption is triaged, not wholesale

Most prose requirements are not formalizable ("should be user-friendly"). Applying PRL to
a repo means **classifying** each Doorstop item: _formalizable-now_ / _falsifiable-only_ /
_stays-prose_. Partial coverage is honest and is reported as such — the tool never
pretends to cover an item it cannot express. This is the README's
provable / falsifiable / vague distinction applied per item.

## A3 — Artifacts co-located, mirroring the subject's own Doorstop layout

A **companion tree beside the subject's Doorstop root**, mirroring its structure (nested
Doorstop documents → nested companion subdirectories; flat → flat), artifacts keyed by
Doorstop ID so the anchor stays obvious under navigation. The layout is **discovered from
the subject's own Doorstop config** (Doorstop names its document roots) — nothing about any
one repo is hardcoded, which is what makes the tool subject-agnostic. The companion root is a
**peer directory beside the requirements directory**, and the tool **proposes its name,
operator-confirmed**: identify the requirements directory (from the Doorstop config,
cross-checked against a name containing `requirements` / `reqs` / `req`) and derive the
companion name by replacing that element with `ProvableRequirements` — `reqs` →
`ProvableRequirements`, `docs/requirements/` → a peer `docs/ProvableRequirements/`,
`my_reqs` → `my_ProvableRequirements`. If no such element is present, fall back to prefixing
`ProvableRequirements-` onto the existing directory name. The operator can override the
proposal. On-disk format (file backend):
**one file per formalized item, named by Doorstop ID**, a Doorstop-shaped YAML envelope
(the `derives_from` list, review-status, verdict) wrapping the PRL text as payload — reusing
Doorstop's per-item convention so diffs are per-item and navigation matches existing habit.

**Logical model vs. storage medium are separate.** The committed file tree above is the
_default_ backend and the one that honors co-location — artifacts version atomically with
the code in the subject's own repo, on whatever forge it lives. But the model (keyed by
Doorstop ID, with the `derives_from` list, provenance, and verdict) is backend-independent
behind a small store interface, so an operator who prefers a database can supply one. The
tradeoff is explicit: a DB drops git-atomic co-location, but staleness does not depend on it
— the verdict pins its own provenance commit (A4), so correctness survives either backend.
**Decision: ship files; draw the store interface now, defer the DB.** Whether to offer a
database backend — and, for an operator who already runs one, to let them configure it — is
a deferred investigation, not a shipped feature. The interface is drawn from the start so
that later swap is not a rewrite.

Split what is committed from what is regenerated (file backend):

- **Source of truth (committed), per formalized item:** the admitted PRL requirement; the
  grounding module(s) (per environment); the `NL ↔ PRL ↔ review-status/reviewer/time`
  provenance record (D14); the latest verdict object (D7–D10).
- **Generated / derived (not committed; regenerated on demand):** the engine inputs the
  requirement lowers to (TLA+, Viper, MFOTL, …) and transient run logs. Treated like build
  output — committing them would let the companion tree rot with stale lowerings.

## A4 — The verdict closes the traceability loop

`scripts/traceability.py` already reports which requirements carry an `Implements:` /
`Verifies:` code tag — the _primitive_ form of "is this requirement met?" PRL upgrades that
from a **claim** to **evidence**: not "a human tagged this function" but "a checker proved
or falsified it against commit `a1b2c3`." The Doorstop item is the anchor; the verdict
object is the proof-or-counterexample, pinned with provenance (D9).

**Staleness is two axes, so it is not either/or.** A verdict goes suspect when the
requirement _text_ moves **or** when the subject _code_ drifts under identical text — the
second being the more dangerous case. These are covered by different machinery and we use
both. Doorstop's own _suspect links_ handle the text axis (free, and already the operator's
muscle memory). The code axis is our own check, but cheap: the verdict already pins
`provenance{commit}` plus the grounded file paths (D9), so it is stale iff
`git diff --name-only <pinned>..HEAD` intersects those paths — one git call, no content
hasher, and scoped to grounded files so an unrelated subject commit doesn't nuke every
verdict.

## A5 — Deployment: one seam; ship B first, keep A as a later topology

The tool splits into two parts:

- **A subject-independent brain** — PRL parse, LLM front-end, mechanical gate, read-back,
  verdict store, UI. Runs anywhere; knows nothing about the subject's toolchain.
- **A subject-local executor** — invokes the verification engines (Verus/Prusti, Viper,
  MonPoly, …) against the **built** subject. Must run where the subject actually builds.

Define the executor as a clean interface — _a lowered engine job + a subject workspace →
a raw engine result + witness_ — and the two deployment options become the same core with
the seam in a different place:

- **Option B (first): installed package in the subject's dev environment.** The executor
  runs in-process, where the subject already builds. One moving part; verification is a
  local subprocess call. **This is the home of the CLI walking skeleton** — it runs inside
  the subject's own dev container (e.g. qrusty's), which is exactly where building the
  subject belongs. The ProvableRequirements container never learns to build the subject.
- **Option A (later): central container + installable agent.** Split the executor out as a
  thin agent installed in each subject's dev environment, driven over a protocol by a
  central brain/UI that can serve many heterogeneous repos at once. Deferred until a
  central multi-repo UI actually justifies designing and versioning that protocol.

Both are the same interface; only the seam's location (in-process vs. wire) differs.
Starting with B and keeping the executor behind the interface leaves A a deployment change,
not a rewrite.

## A6 — The UI is the human-gate surface; annotation is write-through-review, not read-only

The UI is the concrete realization of the trust boundary's mandatory human touchpoints:

- **Pick & translate** — browse Doorstop items, choose which to formalize, trigger the LLM
  forward-translation (D11).
- **Confirm intent** — review the deterministic read-back (CNL paraphrase, independent of
  the forward LLM) and confirm/correct at the risk-tiered gate (D12).
- **Validate grounding** — review LLM-proposed bindings _dry-run against sample
  observations_ ("here are 5 spans matching this — right?") (D13).
- **Inspect results** — the verdict evidence tree: holds/fails/unknown, per-tool epistemic
  map, witnesses, staleness (D7–D10).

It also browses the subject's source, tests, and config — because grounding is authored by
pointing at real code (the function, the identity field, the span name) and assumptions
trace to real config values. But **browse-only is wrong for annotation**, for a concrete
reason: **a deductive tool's proof elements are not _about_ the code — they _are_ the
code.** Verus/Prusti read `requires` / `ensures` / `invariant` / ghost state / spec fns
straight out of the `.rs` source; the verifier never sees a companion file. So for the
whole deductive category (2a) — the center of the qrusty case — a companion overlay
_cannot host the proof at all_. Taken seriously, A3's own principle ("proofs live with the
code they constrain") argues _for_ in-source proof carriers.

The resolution is not "read-write" — it is **write-through-review, and the tool's write
surface stops at the subject's working tree.** On human approval it lands the annotation as
an _uncommitted local edit_ in the checked-out subject — nothing more. It never runs git in
the subject: the human reviews the working-tree diff and commits, pushes, and opens a merge
/ pull request on their own schedule and their own forge, if and when they consider the
branch's work done. The tool holds no commit rights, no push rights, and makes no
GitLab-vs-GitHub-vs-on-prem assumption. That is the same trust gate as D12 — only the
artifact is a working-tree patch instead of a confirmation click. It is exactly how a human
adds a Verus contract by hand, just LLM-drafted and mechanically gate-checked before it
lands. Proofs then version atomically with the code they guard, through the human's own
version control.

Annotation therefore travels by **channel chosen per artifact**, not one overlay for
everything:

| What                                                                                                | Lands in                                                | How                                 | Read by                              |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | ----------------------------------- | ------------------------------------ |
| Proof carriers — contracts, invariants, ghost/spec fns                                              | subject source (`.rs`)                                  | tool proposes patch → human applies | the verifier directly (Verus/Prusti) |
| Traceability marker (`// prl: QRUS042`) beside the carrier                                          | subject source                                          | same patch                          | humans + `scripts/traceability.py`   |
| Back-link: PRL id + latest verdict                                                                  | the Doorstop item's YAML (native `links` / custom attr) | tool proposes → human applies       | Doorstop, published docs             |
| Runtime/monitor bindings, telemetry field maps, dry-run bindings pre-commitment, transient verdicts | companion tree                                          | tool writes freely                  | monitors, UI                         |

The companion overlay does not disappear — it stays correct for artifacts that genuinely
should not touch source: a binding still being _dry-run_ before you commit to it, transient
verdicts, telemetry maps. It simply stops being the _only_ channel. The browser gains a
**"draft annotation → stage into working tree"** action; the human reviewing and committing
that working-tree change is the touchpoint, the same gate as every other. The pre-existing
in-code `Implements:` / `Verifies:` tags are just the traceability-marker channel used by
hand.

A companion-side overlay anchors _into_ code that shifts, so it **re-anchors by a resilient
key** — the enclosing signature plus a content hash of the target span, not raw line
numbers — re-resolved on load and flagged for human re-confirmation when the match
confidence drops. In-source channels need none of this: they move with the code they live
in. The exact key scheme is left to implementation.

This sits cleanly on A5-B: proof carriers live in the checked-out subject the executor
already builds, so nothing new is mounted.

## Cautions carried into implementation

1. **Verification needs the subject's build/toolchain.** Deductive checks (Verus/Prusti)
   run on the actual compiled code, so the executor lives where the subject builds (A5-B).
   Never teach the ProvableRequirements container to build the subject.
2. **Source-mounting covers static categories only.** Categories 1 and 2a are satisfied by
   source + build. Runtime (2b) monitoring needs the subject's traces/telemetry and UI
   probes (3) need a running system + driver. A mounted repo gives source, not runtime.
   **Runtime is recorded-trace-first:** a monitor trace is a log file the subject already
   emits, so it drops onto the same "checked-out artifact → executor → verdict" spine as the
   deductive path — deterministic and re-runnable, which a pinned verdict wants. A live
   connection is a whole streaming design deferred behind the same executor seam as A5's
   Option A; UI probes (3) defer harder still (running system + driver), out of scope for the
   walking skeleton.
3. **The tool writes into the subject only as uncommitted working-tree edits.** Deductive
   proof carriers live in the subject's source (A6); the tool stages them into the working
   tree and stops there. It never runs git in the subject, holds no commit or push rights,
   and lands nothing unreviewed — the human reviews the diff and owns every git action on
   their own forge.

## Build-order guardrail

The walking skeleton stays **CLI-first**: a command that reads a PRL file from the
companion tree + the checked-out subject → lowers → runs one engine → writes a verdict
object. The web UI _wraps_ that spine once it returns real verdicts; it is layer two, not
the starting point.

## Deferred to implementation

The design decisions are settled above; these are intentionally postponed to build time,
each governed by a decision already made:

- **Database backend** (A3) — ship files; investigate offering a DB backend, and letting an
  operator who runs one configure it, only when a real operator needs it. The store
  interface is drawn from the start so the swap is not a rewrite.
- **Overlay re-anchoring key** (A6) — the resilient-key approach (enclosing signature +
  span content hash, re-resolved on load, flagged on low confidence) is chosen; the exact
  key scheme is an implementation detail.
