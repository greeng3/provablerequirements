# Absorbing ReqForge — Decision and Consequences

provreq becomes a **complete requirements system**. ReqForge's management model becomes the
substrate; provreq's engines, verdicts, and evidence become the layer that makes it more than
management. ReqForge is then retired.

Decided 2026-08-19. This document records the decision, the measurements behind it, and what
follows from it. It is not a schedule — each phase gets its own issue (#290).

## The three decisions

1. **Merge, rather than port pieces or invert.** ReqForge's model and the code built on it come
   into provreq together.
2. **Doorstop stops being provreq's storage and becomes a format we read.** It was a useful
   stand-in until a real requirements service existed. ReqForge's importer is what confines it to
   the boundary.
3. **On-disk format moves from YAML to JSON.**

## Why merge, and not the option that sounds more prudent

Three shapes were on the table: port selected pieces into provreq's existing model; invert, keeping
ReqForge's model and re-fitting provreq onto it; or merge the two codebases with a seam between
management and verification.

| Approach         | Reuses                             | Rewrites                              | Risk to proven code |
| ---------------- | ---------------------------------- | ------------------------------------- | ------------------- |
| Port selectively | least                              | the model each piece depends on       | low                 |
| Invert           | ReqForge's model only              | ReqForge's UI, reports, MCP discarded | medium              |
| **Merge**        | **most — 47,667 lines, 786 tests** | the join seam                         | low                 |

**Measured, not assumed:**

- ReqForge is **47,667 lines of Rust with 786 backend tests** (499 `#[test]`, 287
  `#[tokio::test]`) — larger than provreq's 35,978 lines. It already implements what provreq
  lacks: a doorstop importer (2,197 lines, with plan/execute/refs/ids/report), typed links, UUID
  identity, review logs, seven report classes, graph and matrix views, an MCP server, and an LLM
  provider/fallback design.
- **provreq's verification is not coupled to its requirement model.** The engines consume the
  parsed PRL `Requirement`; their references to the storage layer are path and discovery plumbing
  — `adopt::MANIFEST_FILE` in `kani.rs` and `tlc.rs`, `doorstop::discover` in `verify.rs`. The
  substrate can therefore change without touching the engine adapters, the refusal
  classifications, the mirror channel, or the verdict model. That is where provreq's hard-won
  correctness lives, and the merge does not disturb it.
- **Porting selected pieces is the worst option despite sounding the most cautious.** The importer
  and link machinery depend on ReqForge's model, so lifting them means re-implementing that model
  inside provreq — rewriting the expensive part while inheriting none of the 786 tests that
  currently cover it.

## Why the format was YAML, and why that ends

provreq's YAML was never an engine requirement. The engines consume TLA+ `.cfg`, MonPoly's log
syntax, Rust source, and WebDriver JSON; none of them wants YAML. Where `tlc.rs`, `kani.rs`,
`monitor/declaration.rs`, and `ui/declaration.rs` parse YAML, they are reading **provreq's own
manifest** — which is YAML only because we chose YAML.

Two reasons put it there, neither technical:

1. **Doorstop's item format is YAML**, inherited by being a doorstop companion.
2. **We matched it** for our own files (`provreq.yml`, `verdicts.yml`, `triage.yml`, draft state)
   so the companion tree read idiomatically beside the doorstop tree it mirrors (decision A3).

With ReqForge's model as the native store, YAML demotes to an **import-only format**: the thing
read when ingesting a foreign doorstop tree. Everything provreq writes becomes JSON, and
`serde_json` is already a dependency.

## Consequences that shape the work

- **provreq must migrate its own tree.** `requirements-doorstop/` holds ~70 items, and
  `make pre-merge` runs `doorstop -e` as a gate; that gate is replaced. This is an advantage, not a
  cost: our own tree is the best first test of the importer, because we would notice anything it
  got wrong.
- **qrusty stays on doorstop.** Foreign repositories keep their own format, so the importer is a
  permanent boundary rather than transitional scaffolding. See
  [qrusty-as-a-subject.md](qrusty-as-a-subject.md).
- **`verdicts.yml` is evidence, not configuration.** Its recorded engine versions are the
  historical record of what actually ran. It needs a real migration or a retained reader — the one
  place where this format change has a correctness consequence rather than a stylistic one.
  ReqForge's per-file `schemaVersion` with lazy write-back and refuse-newer is built for exactly
  this, which is a further argument for taking its model rather than inventing one.
- **Two frontends and two HTTP servers**, both React and both Rust, are real duplication and the
  likeliest place for this work to get expensive.
- **What provreq is changes.** From a doorstop companion that proves things, to a requirements
  system whose distinguishing feature is that its requirements can be proved. That is the point of
  the exercise, not a side effect.

## What ReqForge brings

In rough order of value to provreq:

1. **The typed link catalog** — `derives-from`, `satisfies`, `verifies`, `supersedes`, `cites`,
   `conflicts-with`, `related-to`, each with forward/inverse names, directedness, and acyclicity;
   stored one-sided with the reverse view derived from a UUID index; extensible via configuration.
   Note that `verifies` is already the relationship a provreq verdict establishes.
2. **UUID identity separated from human-readable naming**, so renames and moves never break links.
3. **The doorstop importer**, which is precisely qrusty's adoption path, already written.
4. **Review as a log rather than a flag** — who, when, outcome, explanation, with
   rejected-with-TODOs blocking re-approval. This is the human half of decision A6's gate;
   provreq's verdicts are the machine half.
5. **The MCP server** — read-only, a thin adapter over the REST API, exposing artifacts, graph
   walks, reports, and review state to coding agents.
6. **The LLM provider design** — a priority array with a fallback chain, three health states, and
   a one-time privacy warning before content first reaches a cloud provider.
7. **Schema versioning with lazy write-back**, which provreq has no equivalent of.

Shared heritage is worth noting: ReqForge's code-trace tag format (`Satisfies:`, `Verifies:`,
with `Implements:` as an alias) generalises provreq's own `Implements:`/`Verifies:` comment
convention, and its design cites `scripts/traceability.py` as a reference. The two grew from the
same root.

## Phases

Each gets its own issue.

1. **Spike the seam.** Verify one requirement sourced from ReqForge's model, without touching an
   engine. Proves or kills the thesis cheaply — and if it drags the engines in, the reading above
   is wrong and we want to know first.
2. **Bring in the model and storage** — artifacts, collections, typed links, UUIDs, JSON on disk.
3. **Migrate provreq's own items** through the importer; retire the `doorstop -e` gate.
4. **Converge verdicts, review log, and reports** — provreq verdicts become evidence on `verifies`
   links, which is what decision A4 always wanted.
5. **UI and MCP** — one frontend, and an MCP surface exposing verdicts alongside artifacts.
6. **Retire ReqForge.**

## Working rules for ReqForge itself

- **Read-only, and we do not build in its tree.** It has its own dev container; `make pre-merge`
  there is the operator's to run, and its result is the evidence.
- **No CI configuration exists** in ReqForge (`.github/workflows` and `.gitlab-ci.yml` are both
  absent), so its 786 tests are unproven until that gate runs.
- It is a **GitLab** repository — `glab` and MRs, not `gh` and PRs.

## Related

- [qrusty-as-a-subject.md](qrusty-as-a-subject.md) — the survey that surfaced this, where B5
  identified the requirement model rather than an importer as the real gap.
- [applying-to-existing-repos.md](applying-to-existing-repos.md) — decisions A1–A6, several of
  which this merge finally makes reachable.
- [requirement-language.md](requirement-language.md) — PRL and the verdict model, the half of
  provreq the merge deliberately does not touch.
