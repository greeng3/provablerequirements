# Applying PRL to an Existing Repository — Design

How PRL attaches to a real project whose requirements already live in
[Doorstop](https://doorstop.readthedocs.io/) — **qrusty** is the reference case. This
records where PRL and its derived artifacts live, how the tool is deployed and driven,
and how a human uses it. Complements [requirement-language.md](requirement-language.md)
(the language itself) and the [README](../README.md) workflow (the trust boundary).

Status: **in design.** Decisions below; open questions at the end.

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

## A3 — Artifacts co-located in a companion tree, mirroring the Doorstop layout

A **companion directory beside the subject's Doorstop directory**, mirroring its structure
(nested Doorstop documents → nested companion subdirectories; flat → flat). Artifacts are
keyed by Doorstop ID so the 1:1 anchor stays obvious under navigation.

Split what is committed from what is regenerated:

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

**Staleness:** a verdict is suspect when either the requirement text _or_ the subject
commit moves. Doorstop already has _suspect links_; whether to reuse that machinery or run
our own provenance-hash check is open (see below).

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

The resolution is not "read-write" — it is **write-through-review: the tool never holds
commit rights to the subject.** It _proposes a diff_; a human applies it through the
subject's normal git/MR review. That is the same trust gate as D12 — only the artifact is
a source patch instead of a confirmation click. It is exactly how a human adds a Verus
contract by hand, just LLM-drafted and mechanically gate-checked before it lands. The tool
stays a never-unreviewed writer, and proofs version atomically with the code they guard.

Annotation therefore travels by **channel chosen per artifact**, not one overlay for
everything:

| What | Lands in | How | Read by |
| --- | --- | --- | --- |
| Proof carriers — contracts, invariants, ghost/spec fns | subject source (`.rs`) | tool proposes patch → human applies | the verifier directly (Verus/Prusti) |
| Traceability marker (`// prl: QRUS042`) beside the carrier | subject source | same patch | humans + `scripts/traceability.py` |
| Back-link: PRL id + latest verdict | the Doorstop item's YAML (native `links` / custom attr) | tool proposes → human applies | Doorstop, published docs |
| Runtime/monitor bindings, telemetry field maps, dry-run bindings pre-commitment, transient verdicts | companion tree | tool writes freely | monitors, UI |

The companion overlay does not disappear — it stays correct for artifacts that genuinely
should not touch source: a binding still being _dry-run_ before you commit to it, transient
verdicts, telemetry maps. It simply stops being the _only_ channel. The browser gains a
**"draft annotation → review as patch"** action; applying that patch is the human
touchpoint, the same gate as every other. The pre-existing in-code `Implements:` /
`Verifies:` tags are just the traceability-marker channel used by hand.

This sits cleanly on A5-B: proof carriers live in the checked-out subject the executor
already builds, so nothing new is mounted.

## Cautions carried into implementation

1. **Verification needs the subject's build/toolchain.** Deductive checks (Verus/Prusti)
   run on the actual compiled code, so the executor lives where the subject builds (A5-B).
   Never teach the ProvableRequirements container to build the subject.
2. **Source-mounting covers static categories only.** Categories 1 and 2a are satisfied by
   source + build. Runtime (2b) monitoring needs the subject's traces/telemetry and UI
   probes (3) need a running system + driver. A mounted repo gives source, not runtime —
   the trace/telemetry data-input path must be designed, not assumed away.
3. **The tool writes into the subject only as reviewed patches.** Deductive proof carriers
   live in the subject's source (A6); the tool proposes them and a human applies the diff.
   It never gets commit rights to the subject and never lands an annotation unreviewed —
   the source patch passes the same human gate as every other trust touchpoint.

## Build-order guardrail

The walking skeleton stays **CLI-first**: a command that reads a PRL file from the
companion tree + the checked-out subject → lowers → runs one engine → writes a verdict
object. The web UI _wraps_ that spine once it returns real verdicts; it is layer two, not
the starting point.

## Open questions

- **Unit of attachment** — strictly 1:1 Doorstop-item ↔ PRL requirement, or many-to-many
  (one prose item → several formal claims; one claim spanning several items)?
- **Staleness mechanism** — reuse Doorstop's suspect-links, or run an independent
  provenance-hash check (D9/D14) alongside them?
- **Annotation model** — A6 sets the channel split (in-source proof carriers via
  reviewed patch; companion overlay for dry-run/runtime/transient). Open within it: the
  exact patch-review flow (does the tool open the subject MR, or hand the diff back for the
  human to commit?), and how companion-side overlays re-anchor when the underlying code
  moves.
- **Runtime/UI data-input path** — recorded traces mounted in vs. a live-system connection,
  for categories 2b and 3.
- **Companion tree specifics** — its exact name and on-disk item format (mirror Doorstop's
  YAML `itemformat`, or a PRL-native format?).
