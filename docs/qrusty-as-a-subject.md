# qrusty as a Subject — Survey and Capability Gap

What is actually in qrusty's requirement tree, and — measured against it — what provreq can do
for that tree today and what it would need next.

qrusty is the first candidate subject whose Doorstop requirements **predate provreq**. Every
subject so far was either scaffolded for the purpose or provreq itself, so the adoption path an
outside user walks has never been exercised. This document exists so that the roadmap discussion
has facts rather than impressions.

The survey sections are observations. [Where the work falls](#where-the-work-falls--two-buckets)
records the roadmap discussion of 2026-08-18: items marked **Decided** were settled there, and
everything else is deliberately still open. Nothing here schedules anything.

Surveyed 2026-08-18 against `/qrusty` (read-only; see [Cautions](#cautions)).

## What the tree contains

108 items across six documents under a single root, `requirements/`:

| Document      | Prefix | Items | Parent |
| ------------- | ------ | ----- | ------ |
| `system`      | SYS    | 27    | —      |
| `websocket`   | WS     | 28    | SYS    |
| `persistence` | PER    | 21    | SYS    |
| `api`         | API    | 14    | SYS    |
| `delivery`    | DLV    | 14    | SYS    |
| `scheduling`  | SCH    | 4     | SYS    |

- **Traceability is real**: 100 of 108 items link to a parent. SYS is the root document.
- **All items are `active: true`, `derived: false`**, and every one carries a review baseline.
- **IDs are `SYS-0001`-shaped** — four digits, `-` separator — not provreq's own `REQ001`.
- **`ref: ""` on all 108.** Doorstop's own requirement-to-code binding field is entirely unused,
  so there is no existing binding to import. Every grounding is new work.
- **A custom `verification:` attribute already exists**, with values `test` (100), `analysis` (4),
  `inspection` (3), and `demonstration` (1). This is a pre-existing classification axis and it is
  **not** provreq's category axis — `test` covers everything from an HTTP endpoint's existence to
  message-lock exclusivity. Whether triage should read it as a hint or ignore it is an open
  question, not a settled one.

## The shape of the requirements, and why it matters

qrusty's requirements are **system-level and behavioural**, not function contracts. A
representative one:

> **DLV-0001** — When a message is consumed, the Qrusty server shall lock the message to the
> requesting consumer for a configurable timeout and shall not deliver that message to other
> consumers while the lock is unexpired.

Others in the same vein: messages survive process restart (PER-0001); per-queue rates sampled at
a one-second interval (SYS-0005/0006); a WebSocket upgrade at `/ws` following RFC 6455 (WS-0001);
dead-letter messages stored under `_dlq/{queue}/{id}` (DLV-0009); a browser-accessible web UI
(SYS-0003, 0007, 0008).

**Provisional category mapping** — read as a first pass to be confirmed by real triage, not as a
result:

| Likely category      | Examples                                | Rough weight |
| -------------------- | --------------------------------------- | ------------ |
| 2a — model checking  | lock/expiry semantics, ordering modes   | moderate     |
| 2b — runtime monitor | sampling intervals, delivery over time  | moderate     |
| 3 — UI               | the three web-UI items                  | small        |
| 1 — code             | no clear instance found while surveying | small        |

**The consequence for planning.** provreq's most-developed machinery is category 1: a
three-engine ensemble (Kani, Creusot, Prusti), contract drafting, the mirror channel, and the
aggregation rules that let a deductive `proven` outrank a bounded `model-checked`. Very little of
qrusty lands there. The weight falls on 2a, 2b, and 3 — the categories with **one engine each**
and the least mileage. Working with qrusty therefore exercises provreq's thinner half, and the
result will be judged on that half. That is a reason to do it, but it should be a decision taken
knowingly rather than discovered halfway through.

## What already works

Verified while surveying, rather than assumed:

- **Multiple Doorstop documents are supported.** `adopt.rs` plans over a `Vec<ManifestDoc>` and
  requires only that all documents share one root. qrusty satisfies that, so `provreq init`
  should scaffold a companion tree covering all six documents with no new code. This was the main
  structural worry and it is unfounded.
- **Multi-category requirements are implemented, not merely designed.** `category: 2a + 2b`
  parses to two categories and the fragment gate checks the requirement against _every_ declared
  category (REQ024). This matters because qrusty's strongest candidates are exactly the properties
  that want both a bounded model-check and a runtime monitor.
- **Every category has a real engine**: 1 → Kani/Creusot/Prusti, 2a → TLC, 2b → MonPoly,
  3 → Selenium over W3C WebDriver.
- **The canonical worked example is already a message queue.** `no_message_lost` in
  [requirement-language.md](requirement-language.md) — conservation across in-flight / retrying /
  succeeded / dead-lettered, plus bounded liveness to a disposition — is close to a direct match
  for qrusty's delivery document, down to the dead-letter reason. The language was designed with
  this shape of subject in mind; qrusty is a real instance of it rather than a new domain.

## What would be needed

Roughly in dependency order. Sizes are deliberately absent — that is the roadmap conversation.

1. **Adopt and find out what breaks.** `provreq init` over a six-document, 108-item tree. No
   subject of this size or shape has ever been adopted, so this is a measurement, not a formality.
   Everything below depends on it being honest.
2. **Triage 108 items.** Our best _measured_ triage accuracy is 8 of 12 on provreq's own
   requirements, whose language we wrote. Expect worse on someone else's prose, and decide what
   role the existing `verification:` attribute plays.
3. **Formalize a representative item per category.** DLV-0001 is the strongest candidate and the
   closest to the worked example. A web-UI item covers category 3. Whether qrusty contains any
   genuine category-1 item is itself worth answering.
4. **Ground the bindings by hand.** With `ref` unused there is nothing to import, and `--ground`
   takes one binding per invocation.
5. **Only then**, "adjust the subject to leverage provreq" — which cannot be specified until
   steps 1–4 have said what the subject actually needs.

### Gaps this survey did not resolve

- Whether a 108-item tree exposes anything that only shows up at scale — triage batching, the
  UI's item list, verdict-store size. Unmeasured.
- Whether qrusty's parent/child link graph should surface in provreq at all. provreq's own tree is
  flat, so nothing has ever consumed Doorstop links.
- What "leveraging provreq" means for a subject that is mostly I/O and state. The category-1
  answer (write contracts, prove them) does not transfer.

## Where the work falls — two buckets

Prospective work splits cleanly, and the split is more useful than a single ordered list because
the two halves have different owners and different blockers.

### Bucket A — covering qrusty's requirement shapes

Make _these kinds of requirement_ answerable. Much of this is **qrusty-side**, not provreq-side.

- **A1 — the subject must emit an event trace** (qrusty-side). Category 2b reads a declared trace
  file: jsonl, numeric timestamps, converted to MonPoly's log syntax. qrusty emits no such stream.
  It already depends on `tracing`, `tracing-subscriber` with `env-filter`, and `serde_json`, so
  the cost is low.
  **Decided: the emission is conditional at RUNTIME, never at build time.** No
  `#[cfg(feature = "provreq")]` gate — if events compile in only for verification, the monitored
  binary is not the shipped binary, and a 2b verdict then describes a build nobody runs. provreq's
  drift axes would not catch that: they would see a different source fingerprint, not "you
  monitored something else". Same binary, configurable sink, subscriber off in production.
- **A2 — MFOTL coverage beyond the three measured rules** (provreq-side). DLV-0001 — lock for a
  configurable timeout, do not deliver to others while unexpired — is a metric-time property.
  MonPoly supports metric operators natively; `src/monitor/mfotl.rs` is ours, and only three rules
  have ever been measured. **Decided: this is ours to add**, with scope set against the real
  requirement text rather than in the abstract. Note `leads_to … within` already routes here
  correctly — TLC refuses it explicitly as "a metric (real-time) bound", which is #222 working as
  designed.
- **A3 — someone must write TLA+ specs** (qrusty-side). Category 2a needs the operator to supply
  the model: `spec_paths`, constants, and a `--ground` per binding to spec operators. qrusty has no
  TLA+ at all, so ordering modes and lock/expiry each need a hand-written spec. This is the largest
  hidden cost in the exercise and it is not provreq work. To be done interactively, later.
- **A4 — a running deployment and a WebDriver grid** (both sides). Category 3 wants `ui.base_url`,
  `ui.steps`, and a reachable endpoint. qrusty has a web UI and its own container, so this is
  mostly configuration — the cheapest of the four.
- **A5 — requirements qrusty does not yet state** (qrusty-side). Restart durability, ordering, and
  deduplication are believed to hold **by construction**, which is precisely the argument for
  writing them down: an invariant that holds because of the shape of the code is the one a
  refactor breaks silently, with no test naming it. **Decided: add explicit items for all three.**
  They are new Doorstop items in qrusty's own tree — a **GitLab** repo, so `glab` and an MR.
  Separately, whether qrusty contains any genuine category-1 item is still unanswered; its ordering
  comparators and dedup keys are the plausible candidates.

### Bucket B — adoption at scale

Make a 108-item, six-document tree workable for **any** subject. Nearly all provreq-side, and
mostly measurement rather than new engines.

- **B1 — the LLM passes.** `src/llm.rs` already speaks both OpenAI-compatible
  `/chat/completions` and Anthropic `/v1/messages` with `api_key_env`, so provreq can call Claude
  models today given an API key. A Claude **subscription is not API credentials**, so it cannot
  simply be pointed at. Triage of 108 items is a real cost: at the measured local rate (~2 minutes
  per five-item batch) it is roughly 45 minutes, and one request has already timed out mid-backlog.
  **Open design direction:** a portable provreq **agent** that carries its own model access and
  rides along with the tool, driven from the web UI — Rust preferred, Python acceptable. That is
  not a new idea so much as the LLM half of A5's deferred Option A (below). Needs a usability
  discussion before any code.
- **B2 — triage quality at 108 items.** Best _measured_ accuracy is 8 of 12 on provreq's own
  requirements, whose language we wrote; expect worse on someone else's prose. Batching and resume
  exist (REQ054). Open: what role the subject's existing `verification:` attribute should play.
- **B3 — grounding ergonomics, ordered by correctness.** With `ref` unused, every binding is
  hand-made, and `--ground` takes one per invocation. The ordering principle is D5 — binding
  fidelity feeds verdict strength — so convenience must never buy a weaker binding:
    1. **LLM-proposed groundings, dry-run-validated before anything is written** (D13). The proposal
       is checked against the subject, so a wrong guess is rejected rather than recorded.
    2. **Auto-proposal from the existing `syn`-based resolution**, which already maps a grep term to
       a real fn at `file:line` — a resolved binding, not a textual guess.
    3. **Import Doorstop's `ref`** where a subject populates it. qrusty does not; the next one might.
    4. **Batch `--ground`** from a file or repeated flags. Pure convenience, and last for that
       reason: it makes the existing binding faster to enter, and nothing about it is more correct.
- **B4 — ID-shape assumptions.** `SYS-0001` (four digits, `-` separator) against provreq's
  `REQ001`. **Decided: infer the shape, and convert where conversion is genuinely needed.**
- **B5 — the requirement model itself, beyond importing from Doorstop.** 100 of 108 items link to
  a parent, and provreq has never consumed Doorstop links because its own tree is flat.
  **This is bigger than an import feature.** A peer repository, **reqforge**, holds a previous
  attempt at improving on Doorstop. **Decided: absorb what is valuable in reqforge into provreq,
  then retire reqforge.** That is a design conversation about provreq's own requirement model, and
  it needs its own issue, branch, and a mount alongside qrusty.
- **B6 — per-document reporting and the UI at 108 items.** Coverage is reported today over one
  flat document of about 70 items.

### The deployment question underneath both

qrusty has its own dev container, and **we do not build or run it here**. So how does provreq
verify a subject it cannot build?

[applying-to-existing-repos.md](applying-to-existing-repos.md) already decides this, and names
qrusty while doing so. **A5 — Option B**: provreq splits into a subject-independent brain (PRL,
gate, verdict store, UI) and a subject-local executor that runs the engines against the **built**
subject, and the executor is installed into the subject's own dev environment. "The
ProvableRequirements container never learns to build the subject." **Option A** — a thin agent per
subject driven over a protocol by a central brain — is deliberately deferred.

Two consequences worth stating plainly:

- Nothing in Bucket A can be _verified_ until provreq is installable and runnable inside qrusty's
  dev container. That is a packaging and deployment problem, not an engine problem, and it gates
  the entire bucket.
- The portable-agent idea in B1 is Option A wearing different clothes. If it is built for model
  access, it should be designed knowing it is also the executor seam.

Both land back on **#1 (end-to-end operator workflow)**, which is where this belongs.

## Cautions

- **Do not run bare `doorstop` against qrusty.** It rewrites items and stamps review baselines as
  a side effect, which would dirty a repo we are only meant to be reading.
- **`provreq verify` writes `verdicts.yml`** into the subject's companion tree. Expected once
  qrusty is deliberately adopted; surprising before that.
- qrusty has its own dev container and build. Do not build it from this one.

## Related

- [applying-to-existing-repos.md](applying-to-existing-repos.md) — the A1–A6 design decisions for
  adoption; this document is a concrete instance to test them against.
- [requirement-language.md](requirement-language.md) — PRL, the categories, and the
  `no_message_lost` worked example.
- [dogfood-self-adoption.md](dogfood-self-adoption.md) — provreq as its own subject, the only
  adoption we have done end to end.
