# Operator Workflow — Working Notes

> **Status: the deliberation recorded here is finished; both threads are decided and shipped.**
> Working notes for issue #1 (migrated from GitLab issue #5). The **reasoning** is the value of this
> document and is preserved verbatim, including for directions that lost — what follows is a record
> of how each question was settled, not a live proposal. Builds on the merged adoption model in
> [applying-to-existing-repos.md](applying-to-existing-repos.md).
>
> **Three deployment designs were on the table. All are kept here deliberately:**
>
> - **Design A — native install + dev-env agent/socket** — the earliest direction.
>   **⚠️ SUPERSEDED / kept for reference.** Sections below marked _Design A (old)_.
> - **Design B — dev-container scope cut + docker-socket seam** — the middle direction.
>   **🔴 REJECTED (ADR #104, 2026-07-25).** The docker-socket branch is a NO-GO; see
>   [devcontainer-branch-decision.md](devcontainer-branch-decision.md). The seam it proposed
>   resolves instead to **detect-and-advise** (REQ048). Sections below marked _Design B_.
> - **Design C — seam-free native provisioner, platform-scoped** — **🟢 DECIDED (ADR #98,
>   2026-07-24): GO, tiered.** Design A resurrected, but with the seam removed and B folded in as one
>   build-env strategy. Shipped as the light install tier (TLC, Kani); the heavy tier is
>   dev-container-first by decision. See [design-c-decision.md](design-c-decision.md).
>
> The operator-journey spine (below) is shared by all three and independent of which won. Design C
> is the first framing that made the tool **operationally possible** rather than a non-starter.

## Operator-journey spine

> **📌 ALL SIX STEPS SHIPPED (Step 1 on 2026-07-13 / issue #8, through Step 6 on 2026-07-22 /
> issue #83).** What began as a proposed skeleton was worked through with the operator step by
> step; each step's record below names the issue and PR that landed it. The genuine open design
> questions lived at Steps 2–3 (see below); Step 1 was fully specified by the settled adoption
> model (A1–A3) and needed none.

1. **First contact** — point at a subject repo → discover its Doorstop layout → propose the
   companion tree + name → operator confirms. **✅ Implemented as `provreq init [PATH]`**
   (`src/doorstop.rs` discovery, `src/adopt.rs` A3 name-derivation + scaffold). Discovers
   `.doorstop.yml` roots (prefix + item IDs), derives the companion name by swapping the
   `requirements`/`reqs`/`req` token for `ProvableRequirements`, and on confirmation
   (`--yes` / `--name` for scripting) writes the peer companion root + a `provreq.yml`
   manifest. Single-root + no per-item files yet (those arrive at Step 3).
2. **Triage backlog** — classify each item (formalizable-now / falsifiable-only / stays-prose).
   **🟢 Designed (2026-07-14, issue #10); machinery SHIPPED (2026-07-14, issue #12)** —
   `RequirementsSource` seam + Doorstop adapter (`src/source.rs`, `src/doorstop.rs`
   `DoorstopSource`), `provreq triage` advisory state (`src/triage.rs`), and the `provreq
status` coverage funnel (`src/status.rs`). REQ009–011. **LLM bulk pre-sort classifier SHIPPED
   (2026-07-14, issue #14 / PR #15)** — `src/llm.rs`, multi-provider and Ollama-first, with the
   `Classifier` seam bulk + fallible + async; its output stays advisory and the honest prose-floor
   seed remains the fallback. Triage from the UI landed later with Step 4's surface (REQ037, issue
   #79 / PR #80). See the "Steps 2–3 design" section below.
3. **Formalize one item** — translate → read-back confirm (D12) → validate grounding dry-run (D13).
   **🟢 Designed (2026-07-14, issue #10); draft lifecycle SHIPPED (#16); D11 translate SHIPPED
   (2026-07-15, issue #18)** — `provreq draft` persists a resumable draft (`src/draft.rs`,
   `drafts.yml`) keyed by source id with revision-token drift detection and a distinct `drafting`
   count in `provreq status` (REQ013–014); `provreq draft <ID> --translate` forward-translates the
   item's prose into an ungated candidate PRL via the LLM seam (`src/formalize.rs`, REQ015).
   **Mechanical gate part 1 SHIPPED (2026-07-15, issue #20)** — `src/prl/` parses a candidate PRL
   block into a typed AST and type/name-checks it (predicate name + arity, no-duplicate-decl,
   category well-formedness, non-empty `require`); `provreq draft <ID> --check` runs the gate over a
   draft's candidate and reports acceptance or structured, line-anchored errors (REQ016).
   **Mechanical gate part 2 SHIPPED (2026-07-15, issue #22)** — vacuity/triviality sanity (accept the
   candidate but warn: self-`leads_to`/`precedes`, immediate `P or not P`/`P and not P`, `occurs at
most 0`, unused vocabulary), the generate-then-repair loop (`src/formalize.rs` feeds gate errors
   back to the LLM for bounded re-translation; warnings ride to the human, not the loop), and a
   persisted per-draft gate outcome (`Draft.gate`: ungated / passed-with-warnings / failed), kept
   truthful across `--set`/`--translate`/`--check` (REQ017).
   **D12 read-back renderer SHIPPED (2026-07-16, issue #24)** — `src/prl/readback.rs` renders a
   gate-passed candidate's AST to deterministic CNL (pure fn, NOT an LLM — the independence is the
   point); `provreq draft <ID> --readback` surfaces the formal meaning for the operator to confirm
   intent (read-only, requires a gate pass, shows vacuity warnings) (REQ018).
   **D12 human confirm gate SHIPPED (2026-07-16, issue #26)** — `provreq draft <ID> --admit
[--reviewer NAME] [--yes]` moves a gate-passed draft to `admitted-but-ungrounded` (`Draft.admission`
   in `src/draft.rs`); risk-tiered (vacuity-flagged → mandatory-review, shown + confirmed; clean →
   optional, direct); records tier/reviewer/time; editing the candidate revokes admission; the
   `status.rs` `formalized` funnel count (honest 0 since Step 2) now counts admitted drafts, with
   `drafting` = in-progress-not-yet-admitted (REQ019).
   **A6/D14 back-write SHIPPED (2026-07-16, issue #28)** — `RequirementsSource::annotate` seam method
   (`src/source.rs`) + Doorstop impl (`src/doorstop.rs`) stamps a `provreq:` block (status, confirmed
   PRL, review/reviewer/time, source revision) onto the subject item, preserving existing fields;
   `provreq draft <ID> --writeback` writes it (requires an admitted, non-drifted draft; a drifted
   admission → needs-reconfirmation, surfaced in draft display/list). REQ020.
   **D13 grounding SHIPPED (2026-07-22, issue #77 / PR #78)** — live grounding validation in the
   item detail surface: each binding resolves or **parks**, and a no-match never fakes a verdict
   (REQ036). See the "Steps 2–3 design" section below.
4. **Verify** — run one engine → inspect the verdict tree. **🟢 SHIPPED (2026-07-22, issue #81 /
   PR #82)** — per-item verify on demand runs the wired engine ensemble and returns the aggregate
   verdict plus each engine's own evidence (REQ038), over the ensemble aggregation from issue #60 /
   PR #61 (REQ030). Category 1 is a three-engine ensemble — Kani (bounded), Creusot and Prusti (both
   deductive) — and category 2a is TLC.
5. **Annotate** — stage the working-tree proof-carrier edit; operator reviews + commits on their own
   forge. **🟢 SHIPPED** — A6/D14 back-write above (issue #28 / PR #29, REQ020) plus the A6
   contract-draft channel (issue #71 / PR #72, REQ033) and the semantic contract drafting that
   followed (REQ040/REQ041). **Which channel applies is the dialect's call, not a flag**: the marker
   draft is Prusti-only (#158) and semantic contracts are Prusti-only (#164), while Creusot reaches
   an ordinary program function through a drafted `#[logic]` **mirror** (REQ068). See
   [the mirror channel](#creusot-reaches-a-program-function-through-a-mirror-issue-160--req068) below.
6. **Living loop** — re-run on drift, act on stale verdicts. **🟢 SHIPPED (2026-07-22, issue #83 /
   PR #84, REQ039)** and since **closed on five drift axes**: requirement prose, subject commit,
   formalization (REQ045), tool version, and the environment a verdict was proved in (REQ049/REQ050).
   With a drifted-verdict funnel stat (REQ043) and a re-verify-all-stale bulk action (REQ044).

The four emerging questions (triage one-at-a-time vs bulk pre-sort; half-finished-formalization
state; coverage display; grounding no-match) are **answered** in the Steps 2–3 design below.

## Operator journey — Steps 2–3 design (triage + formalize) [SETTLED 2026-07-14]

> **🟢 SETTLED (2026-07-14, issue #10), worked through with the operator.** Covers spine Steps 2
> (triage) and 3 (formalize) and answers all four emerging questions. Design-level requirements
> below (`R-src-*`, `R-life-*`, `R-triage-*`, `R-cov-*`, `R-draft-*`, `R-ground-*`) are promoted to
> Doorstop REQ items when each is implemented, exactly as REQ005–008 were.

### The requirements source is an abstraction; Doorstop is adapter #1

Doorstop is **one** requirements tool, not the model. The operator also builds
[**reqforge**](https://gitlab.com/greeng3/reqforge) — a broader-scope, faster requirements
manager (requirements + design docs + use cases + diagrams + roadmaps, one file per artifact in
git) intended to **eventually supplant Doorstop**. reqforge already ships a Doorstop _importer_
(`legacy.doorstopUid` on imported artifacts), so subjects migrate Doorstop → reqforge and provreq
follows by swapping adapters — not by a rewrite. So the requirements source sits behind a seam, the
same interface-with-one-impl move the codebase already makes for the companion store (A3), the
engine executor (A5), and the per-language adapter (R-eng-4).

- **R-src-1** — provreq reaches requirements only through a `RequirementsSource` seam. The
  `src/doorstop.rs` discovery merged in issue #8 is **adapter #1**, not the universe; triage,
  formalize, grounding, and verdict code key off an abstract `Item`, never off `.doorstop.yml`.
- **R-src-2** — the abstract `Item` carries an `id`, prose text, a revision token, and optional
  metadata (title, links, a verification hint). Requirement **content is prose in every source** —
  reqforge's artifact shapes are `Content | Blob | Url` and a `content` body is markdown prose,
  exactly like Doorstop's `text:`. So D11's "the item's prose _is_ the untrusted NL input" (A1)
  holds universally; there is **no "already half-formalized, skip the LLM" branch** to design. The
  tool's breadth is in artifact _types_ and UX, not in making requirement text machine-structured.
- **R-src-3** — `id` is an **opaque stable string** the source owns (Doorstop `REQ001`; reqforge a
  UUIDv7). `derives_from: [id, …]` (A1) already holds either. The adapter also supplies a
  **revision token** — the source's native change signal (reqforge `modifiedAt`) when it has one,
  else a content-hash of the prose (Doorstop). All staleness checks use this token, deferring to the
  source's own change-tracking whenever present.
- **R-src-4** — the companion **logical model** (keyed by source `id`, `derives_from`, provenance,
  verdict) is source-agnostic; A3's Doorstop-file-tree mirror is one _rendering_ of it. A3 already
  separated logical-model from storage-medium, so a source that is not a file tree keeps the model
  and drops the mirror. Discipline: **draw the seam now, keep Doorstop the only implementation**
  until reqforge needs the second (the A3 "draw the interface, defer the DB" precedent; the second
  consumer is real, not speculative).
- **R-src-5** — the adapter may expose an optional **verification hint** that seeds triage: reqforge
  carries `expects_code_trace` per artifact, its own prior for "this should be verified against
  code." `None` for Doorstop. Advisory only (see R-triage-1).
- **R-src-6** — back-links (PRL id + latest verdict onto the item, A6) are written **through the
  adapter**: reqforge's native typed `links`, Doorstop's `links`/custom attribute. One seam method,
  per-adapter rendering.

### Graduated trust: five honest lifecycle states

The D11/D12/D13 human gate exists to catch **formalization** errors, which can produce a false
verdict. Not every LLM touch carries that stakes. Companion artifacts therefore move through five
explicitly-labelled states, and the governing rule is that **no state is ever presented as stronger
than it is**:

```text
advisory (triage) → draft (in-progress formalization)
  → admitted-but-ungrounded → admitted + grounded → verdict
```

- **R-life-1** — every companion artifact carries an explicit lifecycle state from the set above; the
  full D11/D12/D13 read-back-and-confirm gate applies at **formalization**, not at triage. A triage
  miss is recoverable and visible downstream; a formalization miss is what the gate is for.

### Step 2 — Triage (bulk pre-sort, advisory; coverage funnel)

- **R-triage-1** — triage classifies each item into **formalizable-now / falsifiable-only /
  stays-prose** (A2, the README's provable/falsifiable/vague split). The LLM **bulk pre-sorts the
  whole backlog**; the human reviews the sorted list and confirms/overrides. Classification is
  **advisory and ungated** (not a D12 artifact) and **freely re-triageable** — a wrong bucket routes
  work, it never fakes a proof. One-at-a-time is a supported fallback, not the primary flow.
- **R-triage-2** — triage state is stored as **mutable companion state** (A6 "the tool writes freely"
  channel), keyed by source id. It is **seeded** from the source's verification hint (R-src-5) when
  available, still human-confirmable.
- **R-cov-1** — coverage is reported as a **funnel keyed by item id**: `discovered → triaged →
formalized → verified`. The honest states are kept distinct — _un-triaged_ ≠ _stays-prose_ ≠
  _formalizable-but-not-yet-formalized_ ≠ _engine-unavailable_ (the last is R-eng-3's coverage
  gating). CLI-first (a `provreq status`-style command, mirroring `traceability_report.md`);
  it **extends** the existing A4 / `scripts/traceability.py` model. The UI wraps it later.

### Step 3 — Formalize (draft persistence; admitted-and-parked grounding)

The pipeline is unchanged: D11 LLM forward-translate → mechanical gate → D12 deterministic
read-back and human confirm → D13 grounding dry-run → admit. Two questions were open.

- **R-draft-1** _(shipped as REQ013, issue #16)_ — a half-finished formalization persists as a
  **draft** — a _third_ category beside A3's committed source-of-truth and regenerated-derived,
  because it holds human keystrokes and LLM proposals that are neither admitted nor regenerable. It
  carries the source `id`, the **revision token** (R-src-3), the candidate PRL, and — as later slices
  land — the pipeline-stage marker, mechanical-gate outcome, read-back text, and any D13 dry-run
  bindings. The shipped `src/draft.rs` slice persists the `id` + revision token + hand-authored
  candidate; the stage/gate/read-back/binding fields are added by the D11–D13 slices that produce
  them.
- **R-draft-2** _(shipped as REQ014, issue #16)_ — resuming a draft **re-checks the source revision
  token**; if the item moved under the draft, it is flagged **stale** for human re-confirmation
  before continuing (same content-drift instinct as A4's code axis and A6's re-anchor key). Editing
  the candidate re-baselines the draft against the current revision.
- **R-ground-1** — a D13 grounding **no-match never yields a verdict** — not even "unknown," because
  the engine never ran. Provenance records **"not grounded,"** never "engine returned unknown" (the
  honest-provenance rule, applied at the grounding boundary).
- **R-ground-2** — a formalized requirement whose grounding finds no match is **admitted-and-parked**
  (`admitted-but-ungrounded`): the formalization is _done_, only the anchor is missing. Two causes,
  handled differently — (a) **wrong binding** (the LLM referenced a field/fn that does not exist),
  re-propose or hand-author the binding; (b) **not yet observable** (the requirement is ahead of the
  code), park it until the code catches up. The requirement is neither discarded nor faked into a
  verdict.

### Build sequencing (when these land as code)

> **📌 This sequence was executed as planned and is complete.** Kept as the record of the intended
> order; the "Shipped so far" list below it grew into the full slice history in the sections that
> follow. The reqforge adapter is the one item still waiting, on reqforge's own format.

CLI-first, per the A5-B / build-order guardrail. Natural next slices, each its own issue+branch:
draw the `RequirementsSource` seam and refactor `src/doorstop.rs` behind it (`R-src-1..4`) → a triage
command with companion triage state (`R-triage-*`) → a `status` coverage funnel (`R-cov-1`) → the
formalize pipeline with draft persistence (`R-draft-*`, `R-ground-*`). The reqforge adapter (the
`R-src-*` second impl) waits until reqforge's own requirement format stabilises.

**Shipped so far:**

- **Issue #12** — the seam (`R-src-1..4`), the triage command + companion state (`R-triage-1..2`),
  and the `status` funnel (`R-cov-1`).
- **Issue #14** — the LLM bulk pre-sort classifier (`R-triage-1` primary flow, REQ012). The
  `Classifier` seam is now bulk + fallible + async; `LlmClassifier` sits behind it, with the
  prose-floor classifier as the honest fallback. Multi-provider and operator-configurable via an
  `llm:` block in `provreq.yml`:

    ```yaml
    llm:
        provider: openai-compatible # covers Ollama + OpenAI; or `anthropic`
        base_url: http://localhost:11434/v1 # Ollama; OpenAI = https://api.openai.com/v1
        model: llama3
        api_key_env: OPENAI_API_KEY # omit for keyless endpoints like Ollama
        timeout_seconds: 600 # per request (REQ042)
        batch_size: 5 # requirements per request (REQ054)
    ```

    The API key is read only from the named env var, never the file. No `llm:` block → triage uses
    the prose-floor default and says so. Items the model omits/mislabels fall back to stays-prose.

    `batch_size` is the unit everything else is bounded in: a request covers that many
    requirements, `timeout_seconds` bounds that request, and a failure costs at most that batch.
    Each batch is persisted as it lands, so a stopped run is resumed by re-running `triage` —
    which then asks only about what is left. Turn it down for a slow local endpoint, up for a fast
    hosted one.

- **Issue #30** — D13 grounding, first slice (REQ021). `src/grounding.rs` binds each PRL vocabulary
  symbol to a concrete observable (`Binding { symbol, category, observable, fidelity }`, D4/D5) and
  dry-runs the **category-1 (code-state)** bindings against the subject's real source
  (`dry_run_code` walks the tree, skips the companion tree + `.git`, substring-matches, capped).
  Bindings persist on the `Draft` (`--ground SYMBOL=OBSERVABLE`), cleared on candidate edit; matches
  are recomputed live, never stored. A requirement grounds only when every symbol is bound in
  category 1 and each binding matches ≥1 span (`--dry-run`); any unbound symbol, no-match code
  binding, or non-code binding leaves it **parked** (`admitted-but-ungrounded`), honestly reported —
  no-match never fakes a verdict (R-ground-1), non-code categories are deferred until their engines
  are wired. **Since done for 2a**: model binding against a real TLA+ spec (issue #46 / PR #47,
  REQ028) and TLC wired behind it (issue #48 / PR #49, REQ029). 2b/3 stay unwired by decision, so
  their bindings still park. D6 cross-category refinement mappings and regex/AST-precise queries
  remain later slices.

- **Issue #34** — engine coverage report (REQ022, R-eng-2/3). `src/engine.rs` maps each PRL category
  to one engine (R-eng-1 split: cat 1 code = toolchain-welded per-language build toolchain, R-eng-4;
  2a = TLA+/TLC, 2b = MonPoly, 3 = Selenium via WebDriver), `detect`s presence + best-effort
  version — on `PATH`, or for the grid at its endpoint (#245) — **without ever installing** (R-eng-2 — reports welded / available / missing /
  incompatible), and computes per-requirement `readiness` (pure). `provreq engines` lists engine
  status then, for every admitted requirement, whether its declared category's engine is ready —
  ready only when **every** declared category's engine is (multi-category names each blocker); an
  unparseable or category-less candidate is unroutable, never silently ready. Version minimums ship
  presence-only (machinery typed+tested; thresholds → provreq.yml config when a real engine lands).
  No engine execution / verdicts — that is Step 4.

- **Issue #36** — Step 4, the verdict object (REQ023, D7/D9). `src/verdict.rs` carries a three-valued
  `Status{holds,fails,unknown}` + `Provenance{requirement_revision, subject_commit, tool_version}`,
  and a pure `from_grounding` that turns a live grounding result into the honest verdict. **No engine
  runs in this slice, so every verdict is `unknown`** with a mandatory reason (D10): `missing-grounding`
  when the requirement is parked (R-ground-1 — carries the parked reasons as detail) or `no-engine`
  when it is grounded but nothing executed the property. The split was deliberate: a sound `holds`
  needs a prover to check the temporal property, and grounding only confirms the binding _resolves_ —
  a precondition, never a substitute. `provreq verify <ID>` re-gates the admitted candidate, re-runs
  the live cat-1 grounding dry-run (via the shared `code_matches` helper, so `verify` and
  `ground --dry-run` can never disagree), pins provenance (`subject_commit` best-effort, `None` when
  the subject is not a git repo — never fabricated), and renders the verdict; a stale-prose admission
  is flagged alongside it. Strength/basis scale + per-engine evidence tree land with real engines.

- **Fragment check: issue #38 / PR #39 (2026-07-16).** Found while smoke-testing #36 — and it
  reframed the engine slice. **Category 1 is the temporal-free fragment** ("1 → the temporal-free
  fragment (pre/post/invariants) → Viper/deductive"), but the gate never compared `category` against
  the patterns used, so `category: 1` + `leads_to` gated **clean** and earned an
  `unknown / no-engine` verdict — false hope, since no cat-1 engine can _ever_ express liveness. The
  #30 grounding fixture had the same bug (`CODE_REQ` was cat-1 + `leads_to`), so the misconception
  was baked into the codebase, not just one smoke test. `src/prl/fragment.rs` enforces the two rules
  the design states outright: (1) cat 1 is temporal-free — `always`/`never` are invariants and pass,
  everything else is rejected; (2) `can_reach` is branching (CTL `EF`) → 2a only. Every declared
  category must express every pattern. `GateError::OutOfFragment` names the token, the category, why,
  and a remedy; fragment + name/arity errors report together so the D11 repair loop sees both at once.
  **No `inapplicable` verdict reason** — a rejected candidate is never admitted, so it never reaches
  a verdict; D10 reserves `inapplicable` for the residue (soundness-direction mismatch at
  aggregation), which needs real engines. Also fixed the **cat-1 readiness overclaim**:
  `ToolchainWelded → is_ready() == true` (reasoning: "the operator runs provreq in the subject's own
  build env") conflated _having cargo_ with _having a verifier_, reporting every cat-1 requirement
  engine-ready when none existed. Now `EngineStatus::NotWired` (ours to fix by wiring; distinct from
  `Missing`, the operator's to fix by installing) and never ready; the cat-1 engine is renamed to what
  it is, a **deductive verifier**. REQ024 (1.23).

- **Cat-1 binding = a state predicate at a source location: issue #40 / PR #41 (2026-07-17).**
  The engine was never blocked on _which verifier_ — it was blocked on the **binding**. A cat-1
  binding was a **grep term** (`logged_in` ↦ the text `"fn login"`; `dry_run_code` was a plain
  substring search, grounded iff that text occurred anywhere), but the Adapters list requires cat-1
  to compile to "a state predicate at a source location". A substring cannot say which function the
  predicate is evaluated in, what computes it, or whether the symbol is a boolean over program state
  — **no engine can consume it.** Now `src/rust_adapter.rs` (R-eng-4 per-language adapter, Rust-only)
  **resolves** the binding against the subject's real syntax tree via `syn`: exactly one function of
  that name, parameter count matching the arity the **requirement** declares, written to return
  `bool`. `Resolution::{Resolved, NotFound, Ambiguous, WrongArity, NotBoolean}` stay distinct because
  each asks a different person to act; an ambiguous name is **never** silently resolved (choosing
  would bind to whichever file was walked first). All non-resolved outcomes park (R-ground-1). The
  dry-run now reports _"`login` resolves to src/auth.rs:1 `pub fn login(user: &str) -> bool`"_ instead
  of 20 grep lines — D13's "is that what you meant?" finally has something specific to confirm.
  **Resolution is syntactic** (`syn` parses, does not type-check): a `bool` alias or `Result<bool>` is
  judged as written, and the limit is printed with every resolved binding so a green line never
  implies more checking than happened. Deleted the substring machinery outright (`dry_run_code`,
  `DRY_RUN_MATCH_CAP`); `CodeMatch` + the tree walk moved to the adapter, and `grounding.rs` kept the
  category-independent schema + verdict — it owns the model, not the language. `syn`/`proc-macro2`
  promoted from transitive to direct deps (`span-locations` is what makes file:line reportable).
  REQ025 (1.24). The arity check earned itself immediately: the smoke subject declared
  `state logged_in` (arity 0) against `fn login(user: &str)` (arity 1) — a real mismatch the old grep
  binding matched right past.

- **Enum-shaped and method-shaped observables: issue #129 (REQ055).** The `-> bool` requirement was
  found by dogfooding (#125) to invert adoption: well-modelled Rust keeps decisions in enums and
  properties on types, so the more carefully a subject models its states, the less of it provreq
  could reach. An observable may now be:

    ```text
    login                       a free fn written `-> bool`   -> crate::login(&u)
    decide_install::Proceed     one variant of the enum a fn returns
                                                              -> matches!(crate::decide_install(…),
                                                                    crate::InstallDecision::Proceed { .. })
    Engine::is_ready            an inherent method on a type  -> u.is_ready()
    ```

    `A::B` is read from what `A` actually is in the subject — a function (variant test) or a type
    (method). A name that is both is an `Ambiguous` park, never a guess, and a path deeper than
    `A::B` resolves to nothing. `NotAVariant` / `NoSuchMethod` carry the real variants and methods,
    because the useful answer to a misspelling is the list it was meant to be spelled from.

    **How a predicate is called follows from its signature, not from the binding syntax.** A `self`
    receiver makes it a method however it was named — `collect_fns` had always descended into
    `impl` blocks, so a method already resolved green and then lowered to `crate::ready(&u)`, a
    free call to a method, which cannot compile. That reached the operator as an `unknown` with a
    compiler error rather than as the binding mistake it was. `PredicateForm` on
    `Resolution::Resolved` now carries the call shape and `lowering::lower_call` emits it.

    Validated on this repo: REQ047's `install_proceeds` bound to `decide_install::Proceed` and the
    requirement reports **GROUNDED**. It still verifies `unknown` — its PRL applies the predicate
    to four _free_ variables, and lowering only instantiates the quantified one. That is a
    separate gap, not this one.

- **Sorts bind too: issue #42 / PR #43 (2026-07-17).** The last prerequisite before Kani. #40 made a
  cat-1 **predicate** resolve to a real function, but **sorts** — the types a quantified variable
  ranges over — had no bindings at all (`grounding.rs` said so outright: `// ponytail: predicates
only; sort/type existence when cat-1 needs it`). Cat-1 now needs it: a harness cannot say
  `let u: User = kani::any()` when nothing maps `User` to a real type. `bindable_sorts` (quantifier
  sorts + declared `sort`s) and `rust_adapter::resolve_type` → a `struct`/`enum`/`type` alias at a
  file:line. **A distinct `TypeResolution{Resolved,NotFound,Ambiguous}`**, not a reuse of
  `Resolution` — arity and boolean-return cannot occur for a type, and an enum carrying variants a
  caller can never see misstates the state space. Predicates and sorts **never cross-resolve** (a
  `struct login` is not the predicate `login`), through one shared `for_each_rust_file` walk so the
  two can never disagree about which files count. An unbound or unresolved sort **parks** — a
  quantified claim whose domain names nothing real is not grounded, however well its predicates
  resolve. That bit immediately: the smoke subject was GROUNDED before this slice and now parks on
  `User: unbound`, correctly. **Existence only** — whether a type is instantiable (Kani's `Arbitrary`)
  is the engine's question, since the binding is core-owned and shared; answering it here would bake
  one engine's shape into the core, which is exactly what "Kani is lowering #1, not the definition"
  forbids. REQ026 (1.25). The cross-check that was deferred here is **shipped** — see
  [Cross-checking a parameter's type against its sort](#cross-checking-a-parameters-type-against-its-sort-issue-118--req057)
  (**#118** / REQ057). Still deferred from that same list, now split out as **#138**: **generics and
  path-qualified types in sort resolution** (`resolve_type` matches a bare ident), which is why the
  cross-check skips a generic parameter rather than judging it.

- **Kani wired as cat-1 engine #1: issue #44 / PR #45 (2026-07-17).** `verify` now produces a real
  `holds`/`fails` for a grounded cat-1 requirement instead of only `unknown / no-engine`. **Kani is
  engine #1 — first, not only:** D2b wants a per-language **ensemble**, and the verdict object reserves a
  per-tool evidence map for tools with differing soundness directions. Kani goes first because it takes
  **additive proof harnesses** (a generated file under the subject's `tests/`, importing its public API),
  so it never forces the "does provreq write annotations into the subject's own code?" decision —
  Prusti/Creusot would force it immediately, and Verus needs the subject _written in_ its Rust subset (a
  rewrite, against the adopt-existing-repos premise). **That call was since made**: provreq drafts
  contracts into the subject but never writes them unreviewed — the A6 contract-draft channel stages
  deductive markers (REQ033) and semantic `#[requires]`/`#[ensures]` drafts (REQ040) for the operator
  to review, with a bounded repair loop against the real prover (REQ041). **The whole harness shape was
  run against real Kani 0.67.0 BEFORE any lowering was written** — holds → `VERIFICATION:- SUCCESSFUL`,
  violated → `FAILED` + `Failed Checks`, counterexample via `-Z concrete-playback`, and a sort without
  `kani::Arbitrary` → `E0277`, exit 101 — so the design rested on observed behavior, not guesses.
    - **New module `src/kani.rs`** owns lowering + run + output→outcome, so the core never learns what Kani
      is (D2's "one meaning, lowering to each engine" runs in this direction too). `lower` is **pure** and
      Kani-free-testable; only `run` touches the tool. Lowering target is small because the gate (#38)
      already guarantees cat-1 is temporal-free: `always`/`never` over boolean combinations, quantified.
      `never P` = `always not P`. Anything it cannot faithfully express (a scope, a `with` guard, an
      argument that is not the quantified variable) is a `NotLowerable` → honest `unknown`, **never** an
      approximation.
    - **The call follows the subject's real signature.** `rust_adapter::Resolution::Resolved` now carries
      `params: Vec<ParamMode>` (by-ref vs by-value, judged syntactically like everything else the adapter
      does), so the harness emits `login(&u)` or `login(u)` to match — a mismatch surfaces as a harness that
      won't compile → `unknown`, never a wrong verdict. Cross-checking a param's _type_ against the sort
      is **shipped** (**#118** / REQ057, which absorbed this from the since-closed #42) — it happens at
      grounding now, so only a mismatch no written-name comparison can see still lands as a
      compile-error `unknown`.
    - **Verdict split (D7/D8/D9):** polarity (`status`) from basis (`Basis::ModelCheckedBounded` — the ONLY
      rung, because Kani is bounded; `proven` is _unrepresentable_, so an engine cannot overclaim by
      accident) from witness (the concrete counterexample as a runnable replay test — D9's re-checkable
      witness). The read-back spells out "verified over the states the engine explored, NOT proven for all
      executions" so a bounded pass can't be misread even at a glance. New `UnknownReason::Inconclusive` for
      "engine ran, couldn't decide" — and its detail prefers the compiler's own `error…` line over the tail
      of the log, because the actionable cause (`User: kani::Arbitrary is not satisfied`) is at the _top_ of
      a rustc diagnostic.
    - **`engines` honesty, both ends (the pre-existing overclaim this slice had to fix):** cat 1 gained a
      real probe (`cargo-kani`), and **2a/2b/3 LOST their probes** — a probe now exists iff provreq can
      _run_ the engine, so `probe: Some` means "wired", not "we know the binary's name". Otherwise an
      operator with `playwright` on PATH would see cat 3 report ready while `verify` still answered
      `no-engine` — the exact REQ024 overclaim wearing a different hat. `ready` finally means "provreq can
      run it".
    - **No litter in someone else's repo:** the harness file _and_ any `tests/` dir provreq created are
      removed on every path incl. failure; an existing file is never clobbered (a collision is `unknown`,
      not an overwrite).
    - **Install/CI (settled 2026-07-17, done here):** Kani in `.devcontainer/Dockerfile` (`cargo install
--locked kani-verifier` + `cargo kani setup` for CBMC/solvers) + `postCreateCommand` version summary.
      CI's main `test` job stays **Kani-free by design** (R-eng-2: engine-absent is the common user state
      and the path most worth proving continuously); real-engine tests are `#[ignore]`d and run by a
      **separate parallel `kani` job** (`cargo test -- --ignored`). Verified here: 164 Kani-free unit tests +
      4 real-engine tests (holds/fails-with-witness/inconclusive/no-trace) + a live CLI smoke against a real
      cargo subject, all green. REQ027 (1.26). Both once-deferred items are since **shipped**: the
      default-unwind/timeout config landed as a `kani:` block in `provreq.yml` (**#117**, REQ027 amended —
      `kani.default_unwind` and `kani.timeout_seconds`, each optional, each falling back to Kani's own
      default, and the effective pair reported on every bounded `holds` _including_ when nobody chose it);
      the param-type-vs-sort cross-check as (**#118** / REQ057). The D2b ensemble is
      since **complete** — Creusot
      (REQ031) and Prusti (REQ032) joined as engines #2 and #3, with per-tool evidence aggregated by
      REQ030 and the shared claim-lowering core extracted in #69 / PR #70.

- **Cat-2a grounding — model binding vs a TLA+ spec: issue #46 / PR #47 (2026-07-17).** Chosen fork =
  the **second category** (2a model checking). Sliced the way cat-1 was — **grounding first, engine
  (TLC) next** — so a category-2a requirement can become GROUNDED while its engine stays `NotWired`,
  exactly the state cat-1 sat in between #30 and #44.
    - **The observable world for 2a is a MODEL, not the subject's code.** Per the design (Grounding layer →
      Adapters: "2a (model) a direct model variable/action reference"), the operator writes a TLA+ spec and
      a 2a binding names a definition in it. New `src/tla_adapter.rs` (peer of `rust_adapter`, R-eng-4 per
      observable-world) walks the subject's `.tla` files (same companion/`.git` skip discipline) and
      resolves a symbol to a `VARIABLE(S)`/`CONSTANT(S)` declaration or a top-level operator definition
      (`Name ==`, `Name(args) ==`, `Name[x \in S] ==`).
    - **ONE resolver, not two — and that's more faithful, not just smaller.** Cat-1 splits predicate→fn
      from sort→type because Rust makes them distinct and a `struct login` must never satisfy the predicate
      `login`. TLA+ draws no such line: an action, a state operator, a data set, a variable, a constant are
      all just _named definitions_, so `ModelResolution{Resolved,NotFound,Ambiguous}` asks one question —
      does the spec define this name? Ambiguity (defined in two specs) never silently disambiguates.
    - **Existence only** (like REQ026 sorts): arity/shape is the engine's question, deferred (**#119** — arity
      later **reversed** by a live run; see "A model binding is checked at the arity it is used with"). **Structural
      read, not SANY** (no TLA+ parser crate exists as `syn` does for Rust): comments stripped, `VARIABLES`/
      operator forms parsed properly (not substring-matched), and the read-back states the limit
      (LET/INSTANCE/multi-line decls not seen) so a green line never implies more than was checked —
      mirroring `rust_adapter`'s "syntactic check only" honesty.
    - `grounding.rs` gained a `BindCategory::Model` arm in `verdict()` (now takes a third resolution map);
      `main.rs resolutions()` produces model resolutions for 2a bindings and threads them through
      `verify`/`--dry-run`. `engine_verdict` returned `no-engine` for 2a until TLC was wired in the
      next slice (issue #48 / PR #49).
      REQ028 (1.27). 178 tests + a live CLI smoke (a 2a `leads_to` requirement grounds against a real
      `Msg.tla`, and an unresolvable binding parks), all green. Deferred: wire TLC → real verdict (**done** — issue #48 / PR #49); operator
      arity/shape checks (**#119** — arity **done**, shape still deferred); a configured spec path when specs live outside the subject tree
      (**#120**).

- **TLC wired — cat-2a engine: issue #48 / PR #49 (2026-07-17).** The REQ027 analog for the model world.
  A grounded category-2a requirement now earns a real `holds`/`fails`, closing the `NotWired` state #46 left
  it in. Everything below was verified against real TLC 2.19 (a JRE + `tla2tools.jar`, ~2 MB — far lighter
  than Kani/CBMC), not designed in the abstract.
    - New `src/tlc.rs`: `lower` (PURE, TLC-free-testable) + `run` + `Outcome::into_verdict`, mirroring
      `kani.rs`. `lower` turns a gated 2a temporal pattern into a TLA+ temporal property and emits an
      **additive** module that `EXTENDS` the subject's own spec plus a `.cfg` naming the subject's `Spec` and
      the property. Subject spec untouched; generated files removed after the run (TLC's `states/` scratch
      redirected to a throwaway `-metadir` outside the subject); an existing file is never clobbered.
    - Linear-temporal core: `always`→`[]`, `never`→`[]~`, `eventually`→`<>`, `leads_to`→`~>`, over a
      `\A x \in Sort` quantifier. A scope, a `with` guard, a metric `within`, a non-variable arg, or a
      pattern outside that core (`precedes`/`occurs at most`/`can_reach`) → honest `NotLowerable` →
      `unknown`, never approximated (D2). The subject must define a behaviour `Spec` (located via
      `tla_adapter`); a missing/ambiguous `Spec`, an unassigned `CONSTANT`, or a parse error → honest
      `inconclusive` — the TLC analog of Kani's uncompilable harness. Constant models deferred
      (**#121** — **done**; see "A parameterised spec is checked under the operator's model").
    - `engine.rs` honesty: `EngineProbe` gained `args: Vec<String>` + a `version_marker`, because TLC runs as
      `java -cp <jar> tlc2.TLC` (no PATH binary) and `java` present ≠ TLC present — only the `TLC2 Version`
      banner in the output counts, so a jar-absent host reads `Missing`, not falsely `Available`. Cat-2a gets
      a real probe and stops being `NotWired`. (The probe later gained the other half of that honesty:
      REQ051 reads the **exit status**, so a binary that is on `PATH` but cannot start — exit 126/127 —
      reports `Unusable`, distinct from both `Available` and `Missing`.)
    - `main.rs engine_verdict` now dispatches on category: 1→Kani (`kani_verdict`), 2a→TLC (`tlc_verdict`);
      2b/3 stay `no-engine`. `verdict.rs` reused as-is — TLC is bounded too, so a pass is
      `Basis::ModelCheckedBounded` (`model-checked (bounded)`, **never** `proven`) and a `fails` carries TLC's
      counter-example behaviour as the D9 witness.
    - Install/CI (Kani precedent): JRE + `tla2tools.jar` baked into `.devcontainer/Dockerfile` + a postCreate
      version line + `TLA2TOOLS_JAR`; installed in the running container too. Main `test` job stays
      engine-free; a separate parallel `tlc` CI job installs TLC and runs `cargo test --lib tlc:: -- --ignored`
      (the `kani` job was scoped to `kani::` so it stops running the cat-2a real-engine tests it can't).
      REQ029 (1.28). 198 engine-free tests + 3 real-TLC (`#[ignore]`d) + live CLI smoke, all green.

**What happened next (the cat-1 fork, since RESOLVED):** the `proven`-capable deductive verifier
question was answered by taking **both** candidates rather than choosing — Creusot as cat-1 engine #2
(REQ031, issue #62 / PR #63) and Prusti as #3 (REQ032, issue #64 / PR #65), each baked into the
devcontainer image. The D2b per-tool evidence map arrived first as REQ030 (issue #60 / PR #61), so
`aggregate` reports the stronger rung when a deductive engine and a bounded one both hold. The
annotate-the-subject decision it forced was answered by the A6 contract-draft channel (REQ033) and
then semantic contract drafting with a bounded repair loop (REQ040/REQ041). The three engines'
duplicated claim-lowering was extracted to one core in #69 / PR #70.

**Still `NotWired`, by decision:** the **cat-2b/3** engines (MonPoly runtime monitor / a UI driver).
The 2b/3 liveness-monitorability question (can a finite trace refute `eventually P`? — only the metric
`leads_to … within T` looks decidable there) **stays open**; settle it with the user before narrowing
2b/3 from permissive. Until then a probe for them would promise a readiness nothing can honor, which
is the REQ024 overclaim wearing a different hat.

## Packaging — Design A (old, superseded)

> **⚠️ SUPERSEDED by Design B's scope cut.** Kept for reference. R-pkg-1/2/3's native-install
> path and the agent/socket topology are the parts Design B deletes; R-pkg-4 (`serve` +
> embedded UI) survives into Design B.

Direction: an **installable package the operator runs in their own dev env**, not a container
with their repo mounted (that loses the ability to build in their env). The tool's process
must be **co-resident with the subject's build toolchain**; distribution channel is a
separate axis from where it runs.

- **R-pkg-1** — installs as a single self-contained binary into the operator's existing dev env.
- **R-pkg-2** — the executor invokes the subject's already-present **build** toolchain
  (discovered on `PATH`), never bundled/rebuilt. _(Correction: this is the build toolchain
  only — the verification engines are a separate, usually-absent concern; see below.)_
- **R-pkg-3** — install/distribution is independent of the subject's language (prebuilt
  per-platform binaries + script; `cargo install` a convenience, not the only door).
- **R-pkg-4** — single binary, multiple entry modes: headless subcommands (the CLI-first
  spine, scriptable) **and** a `serve` mode running a **local** web server hosting the
  **embedded** web UI (the A6 gate surface), co-resident in the dev env. Local-served, not
  hosted — **decided, see "provreq is a single-operator tool" below (#122)**. `serve` foreground
  default; `--background` / `--port` flags; no daemon manager.

### Where the subject path goes (issue #130 / REQ056)

The positional slot is each command's **own primary subject**, so it holds different things:

| positional | commands | subject path |
| --- | --- | --- |
| the path itself | `init`, `triage`, `status`, `engines` | positional `[PATH]` |
| an id / engine name | `verify`, `draft`, `install` | `--path` |

The rule is consistent, but not self-evident from any single command, and the habit `status .`
builds carries straight into `verify REQ047 .`. Clap's own answer named the problem without the
fix — `Usage: provreq verify [OPTIONS] <ID>` hides `--path` inside `[OPTIONS]`. `parse_cli` in
`src/main.rs` now recognises a stray path-shaped positional and appends the accepted form in the
operator's own words (`--path .`). The recognition is deliberately narrow — `.`/`..`, a separator,
or a directory that exists, never something starting with `-` — because a confident `--path` hint
attached to an unrelated error would be worse than the bare message it replaced.

### Cross-checking a parameter's type against its sort (issue #118 / REQ057)

`each u: User . always logged_in(u)` bound to `fn login(s: &Session) -> bool` used to ground
**green**: sorts resolve and predicates resolve, but nothing compared the two. The harness then
named a `User` where the subject wanted a `Session`, so the operator learned the binding was wrong
from a compiler error inside an `unknown` — from the wrong surface, since grounding is the surface
built to answer exactly that question.

The check lives in the adapter, because that is where every read-back already flows from:

- `grounding::expected_param_types(req, bindings, symbol)` says what each parameter's argument
  ranges over — the type the **sort's own binding** names, position by position. Pure, and on this
  side of the seam on purpose: the sort a parameter should take is the requirement's claim, while
  the adapter only reads what the subject wrote.
- That vector **is** `rust_adapter::resolve`'s `params` argument, replacing the old bare `arity` —
  its length is the arity, so the two can no longer desync.
- A disagreement is `Resolution::WrongParamType`, an 8th variant, so `describe()` teaches it and
  every surface (CLI dry-run, `verdict`, the UI's per-binding grounding report) shows it without
  new plumbing.
- Checks run coarsest-first — arity, return type, then parameter types — so the operator is never
  sent to fix a parameter on a function that is not a predicate at all.

**It skips more than it judges, on purpose.** `None` at a position means nothing is claimed: an
argument that is not a variable the claim ranges over, an unbound sort,
two properties quantifying the same position over different sorts, a generic parameter (`T` names
whatever the caller instantiates — resolving it is type inference, which `syn` does not do), a
tuple/slice/`impl Trait`. Generic _arguments_ are ignored rather than rejected, so `Wrapper<u32>`
still reads as `Wrapper`. A `self` receiver's type is the type its `impl` block is for, so a method
bound to the wrong sort is caught however it was named. **A false park costs the operator a working
binding, which is worse than the compiler error this removes** — that asymmetry is what decides
every skip above.

Comparison is by written name, the same syntactic limit the rest of the adapter works under, and
the read-back says so: an alias for the expected type would read as a mismatch here. Generics and
path-qualified types on the **sort's own** side stay unresolved — split out as **#138**.

Also lifted: the atom walk is now `Property::for_each_atom` in `prl/ast.rs` (was private in
`prl/check.rs`), so the gate's name/arity check and grounding cannot disagree about which
applications exist.

### Primitive sorts (issue #140 / REQ058)

A sort grounded only by finding a `struct`/`enum`/`type` alias the subject declares (REQ026), so
**`bool` could not be a sort** — and two of `decide_install`'s four parameters are `bool`. Every
direction in #136 needs to quantify over those, so this is its prerequisite and shipped first.

Two halves, and the second is the one that bites:

1. `TypeResolution::Primitive(String)` — grounds, and carries **no `CodeMatch`**: nothing in the
   subject declares `bool`, so a source location would be one the adapter invented. The read-back
   says so instead. `rust_adapter::is_primitive` is the list; a **declared type of the same name
   wins** (it has a location the operator can confirm, and the read-back names it — the language is
   only the fallback).
2. **Lowering writes it bare.** `lowering::qualify` — a declared sort is reached through the
   harness's prefix, a primitive is not, because `crate::bool` does not compile and would reach the
   operator as an `unknown` carrying a compiler error. The primitive list lives in `rust_adapter`
   (the per-language adapter owns what Rust's primitives are) and lowering asks it, rather than
   both keeping a copy.

`str` is refused (unsized — a quantifier over it lowers to a harness that cannot build, the exact
failure grounding early prevents), and so is `String` (a declared std type, not a primitive;
admitting it opens "which std types may a subject quantify over" with no reason to answer yet).
REQ057 needed nothing: it compares written idents, so a parameter written `bool` against a sort
bound to `bool` already matched.

**First real `holds` in this project.** A scratch cargo subject with `fn flag_settled(b: bool) ->
bool { b || !b }`, `each b: Flag . always settled(b)`, `Flag=bool` — grounds, and Kani proves it
(`holds — model-checked (bounded)`). Creusot and Prusti report honest inconclusives for their known
subject-side preconditions (no `creusot-std` dependency; Prusti's pinned nightly cannot read a v4
lockfile), which is the ensemble behaving as designed.

### Free variables are universally closed (issue #136 / REQ059)

A property carried **one** binder (`Property.quantifier: Option<Quantifier>`, parser: `each <var>:
<sort>.`), and lowering would instantiate only that variable — so a predicate of arity > 1 could
never be lowered, whatever the operator wrote. REQ047 grounds and still died at
`install_proceeds is applied to d, which is not the quantified variable`. Relating a function's
inputs to its result is as ordinary as program properties get, so this was a ceiling on which true
things could be proved.

**Decided on #136 (comment there records the weighing): option 2, sorts from the vocabulary.** The
N-variable plumbing was unavoidable in every option; the only real choice was where a sort comes
from, and the vocabulary already parsed `state p(d: EngineStatus)` into `Param { name, ty }` with
`ty` read by nothing.

- **`Requirement::binders(prop)`** (`prl/ast.rs`) is the one derivation everything uses: the `each`
  binder first when written, then each free variable at first application, sorted by the
  vocabulary's declared parameter type. An explicit `each` **wins** — the operator wrote it
  deliberately. `sort: Option<String>`, `None` when the requirement does not say (undeclared, or
  two applications disagreeing); nothing is guessed.
- **`LoweredClaim.quantified` is a `Vec`**, and the three wrappers take N: `let v: T =
  kani::any();` per binder, `forall<a: A, b: B>`, `forall(|a: A, b: B|)`.
- **The read-back states the closure** — "for each d of type Decision and each f of type Flag, …
  — every variable the claim mentions is quantified". D12 is only faithful if the operator sees the
  quantification the harness is actually built with, not just the binder they typed.
- **A declared parameter type is a bindable sort** (`grounding::bindable_sorts`), so it must resolve
  to a real type like any other — otherwise the closure would range over a domain nothing confirmed.
- **REQ057's cross-check now covers every position**, not just the one an `each` supplied, because
  `expected_param_types` reads the same binders.

**Scoped deliberately: invariants in the code fragment only.** The readback test caught the
overreach — in `accepted(m) leads_to (dead_lettered(m, r) with r != "")`, `r` is the reason there
_happens to be_, not every reason there could be; and a 2a/2b claim is lowered by a path that closes
over nothing, so advertising a closure would misdescribe the tool. `closes_over_free_variables` =
`always`/`never` **and** routed to code (undeclared category defaults to code, the same rule
`grounding::default_category` uses). Literals are not variables (`p(true)` binds nothing —
otherwise the harness would emit `let true: bool = kani::any()`).

Validated on the REQ047 shape end to end, both directions: `fn decide(supported, present, consent)
-> Decision` with `always (not proceeds(s, p, c) or supported(s))` grounds, and Kani **proves** it;
removing the platform gate from the subject turns it into **`fails`**, with the counterexample
assertion showing all three closed-over variables instantiated. (REQ047 itself declares no parameter
types yet — updating its candidate is an operator act for the next dogfood pass, not something this
slice does to the companion tree.)

### Every cat-1 engine explains its own limit (issues #146–#152 / REQ062–REQ067)

Pointing the wired ensemble at real subjects turned up six ways an engine could fail while telling
the operator nothing they could act on. Each was answered by the same rule: **an engine that cannot
answer says what it is that stopped it, in terms of the thing that stopped it.** A tool error dumped
verbatim reads as the subject's fault, and the operator's next move depends entirely on whose fault
it was.

- **REQ062 — a variant test every checker can read** (#151). The enum-variant lowering emitted a
  `match`, which Kani reads as ordinary Rust and Creusot's Pearlite does not read at all. The
  generated fragment has to be expressible in the language the checker actually interprets, which is
  not always the language of the surrounding file.
- **REQ063 — a toolchain ceiling named as a ceiling** (#152). Prusti is welded to a 2023 nightly
  whose cargo cannot read a v4 lock file or a dependency declaring `edition = "2024"`. That is not a
  missing annotation and no contract fixes it, so the verdict says so and stops inviting the operator
  to annotate their way out.
- **REQ064 — a crash is neither a `fails` nor a defect in the subject.** Creusot panicked
  (`internal error: entered unreachable code`) and nothing at all had been learned about the claim.
  Presented as a prover defect, with the scratch harness cleaned up after it.
- **REQ065 — an uninstantiable sort is a subject-side precondition** (#148). Kani needs
  `kani::Arbitrary` to range over a sort; without it the raw `E0277` looks like the subject failing to
  compile. It is a capacity the subject must supply for the method to apply at all, and it is said
  that way. This was the one case that earned an exception to "quote the first error line".
- **REQ066 — a boolean variable can stand as a condition on its own** (#146). Requiring every
  condition to be a named predicate of the subject left a whole class of requirement unsayable —
  exactly the ones relating what goes _into_ a function to what comes out.
- **REQ067 — an untranslatable construct is the checker's limit** (refs #153). Creusot ICEs on any
  crate containing an `async fn`, because an async body reports as a closure but is a coroutine.
  Reported upstream (creusot-rs/creusot#2212, still open) and **patched locally** so the engine is
  usable here meanwhile; the verdict offers `#[trusted]`, never a `#[logic]` hint, since no contract
  makes an untranslatable construct translatable.

### Creusot reaches a program function through a mirror (issue #160 / REQ068)

**This is the arc that produced provreq's first real `proven`.** Everything before it topped out at
Kani's bounded `holds`.

The wall: Pearlite — the language inside `proof_assert!` — may only call `#[logic]` items, and
`#[logic]` cannot go on the subject's function, because it moves that item out of the program
namespace and breaks every call site (#158, six of them in this repo). A category-1 predicate
normally resolves to exactly that: an ordinary function the subject calls. So the harness could not
name what the requirement was about.

**The bridge is a mirror.** provreq leaves the function alone and stages a `#[logic(open)]` twin
stating its meaning, plus a linking `#[ensures(result == mirror(args))]` on the untouched program
function. The prover discharges the mirror against the real body, so a wrong mirror fails **at its
own link**, naming the function it misstates, instead of going on to prove something false.
`creusot::with_mirrors` does the redirection by rewriting _resolutions_, so the shared lowering and
the harness shape never learn about any of it. Creusot-only: Kani executes these functions and must
keep the program ones.

The meaning is the model's proposal, the operator's to review, the prover's to check — so provreq
writes only the mirror's **name** into a proof, never its content, and the attribute mechanics are
provreq's job and never the model's: the link is built from the signature with `syn`, visibility is
copied from the mirrored fn, transparency is forced, and the item is `syn`-validated. **A mirror
provreq cannot link or parse is dropped**, because an unlinked mirror is an unchecked meaning.

**Contracts are Prusti-only, and this is soundness rather than tidiness (#164).** A mirror's link is
discharged _assuming the function's preconditions_, so a drafted `#[requires]` narrows the domain the
mirror was ever checked on while the harness's `forall` ranges over all of it. Measured against real
Creusot: a mirror genuinely correct under `!allowed`, whose link discharges honestly, plus an
ordinary `#[requires(!allowed)]`, yields **`Holds` for a claim that is false of the program** —
nothing contrived, nothing in the prover misbehaving. The probe is kept as a test that **asserts
`Holds`**; if it ever stops holding, re-derive the rule rather than assuming it.

Ten defects came out of this arc and **every one was found by running the real prover and a real
model** — the stub tests passed throughout. Two worth carrying forward:

- **`#[logic]` vs `#[logic(open)]`: visibility is not transparency.** Creusot defaults a logic fn's
  _body_ to `Transparent(Restricted(parent_module))`; `pub` makes it callable, not unfoldable. A bare
  `#[logic]` mirror compiles, runs, and **cannot discharge** — presenting as a claim that will not
  prove rather than as a missing attribute.
- **A false `proof-carrying`.** The repair loop reported `Proved` if _any_ engine held, so Kani's
  standing bounded `holds` passed off as Creusot's proof while Creusot had not even compiled. Never
  report a `proven` from a drafting message — re-run a plain `verify` and read the verdict line.

**provreq still cannot be its own Creusot subject** (#153): the async ICE above, plus whole-crate
`format!`, across 28 of 32 files. That is why the in-repo `mirror_subject` fixture exists, and CI
asserts both directions on it.

### A model binding is checked at the arity it is used with (issue #119 / REQ028)

**A deferral that a single live run reversed.** Cat-2a grounding asked one question — does the spec
define this name? — and left arity to the engine, by analogy with REQ026 sorts. The issue itself set
the condition for revisiting: _only if_ a real subject showed a wrong-arity binding reaching TLC as
a confusing `inconclusive`. Nobody had tried it. The first cat-2a end-to-end pass did.

The subject was built so the mistake would be an ordinary one rather than a contrived one: a spec
with an operator `Accepted(m)` **and** a variable `accepted`. Binding the 1-ary predicate to the
0-ary variable is a plausible slip, and it ground green:

```text
accepted → `accepted` resolves to msgs.tla:4  VARIABLES accepted, done
    (existence only — …)
REQ001: GROUNDED — every symbol binds to a confirmed observable.
```

then produced `unknown (inconclusive)` — an engine ran and could not decide. **This is the wrong
verdict class**, and its reason pointed at `line 6, col 57 of module provreq_req001`: a generated
module `tlc::run` deletes before it interprets the output. The operator is sent to a file that no
longer exists, in a module they never wrote, for a mistake in their own binding. (The reason was
worse still until #206, which found that provreq was reporting SANY's `*** Errors: 1` banner rather
than the cause under it — a separate defect the same run exposed.)

**Two premises had to be engaged with, and both gave way.** `ModelResolution`'s own doc said arity
was a Rust-type question that "does not arise for a bare TLA+ name" — but a TLA+ operator has an
arity by definition, and TLC says so in as many words: `The operator accepted requires 0 arguments.`
And the objection that checking it costs a second walk was not true either: `SpecMatch.text` already
holds the declaration line, so the arity was in hand the whole time.

Now the binding resolves against the arity the **requirement** applies the symbol to —
`grounding::predicate_arity`, the same number cat-1 checks a function's parameters against, computed
by the core and passed in, so the adapter still only reads what the operator wrote. A sort declares
no parameters and is applied to none, so it must resolve to a definition taking none. Live, the same
subject now says:

```text
accepted: `accepted` is defined at msgs.tla:4  VARIABLES accepted, done — but it takes no
arguments, and the requirement applies `accepted` to 1 argument. TLC would reject the generated
spec instead of checking it, so this is refused here, where the binding is
REQ001: unknown (missing-grounding) — … no engine can run until every symbol binds
```

The verdict class changed with it, which is the honest half: no engine ran, so none should be
reported as having run.

**What is deliberately not checked.** Return shape stays the engine's question. A line stating no
arity claims nothing — a binding wrongly parked costs the operator more than the check saves them —
and the read-back tracks that exactly, saying `existence and arity` only where arity was read.
Ambiguity is still reported first: until it is known which definition is meant, there is no arity to
be right or wrong about. A function definition (`Double[x \in Nat] == …`) takes **no** arguments as
an operator, because TLA+ applies it by subscript; that was confirmed against real TLC rather than
reasoned about, since provreq can only ever emit the `Op(args)` form.

**REQ028 asserted the deferral in its own text** and had to be amended — the fifth time in this
project that a written artifact held a defect in place rather than catching it, and the second time
the artifact was a requirement item rather than a test.

### The model may live outside the subject (issue #120 / REQ028)

Cat-2a found `.tla` files by walking the subject, which quietly made a layout decision for the
operator: a team whose specs live in a sibling repo could not ground a category-2a requirement at
all. Every name resolved to nothing and every binding parked. Parking was honest; the layout being a
hard limit was not.

`provreq.yml` now names extra roots, resolved against the **subject** (the operator is describing
where the model sits relative to the thing being verified):

```yaml
tla:
  spec_paths:
    - ../models
```

**Two consequences make this more than a search path, and both are about what a verdict means.**

**Provenance.** The five drift axes are the requirement revision, the formalization, the subject
commit, the proving environment, and the tool version. **None of them can see a file outside the
subject change.** A verdict proved against a sibling repo's spec would have gone on reading `fresh`
forever while the model moved underneath it — the living loop blind to the very artifact the verdict
is about. So the out-of-subject specs are fingerprinted and that becomes a sixth axis. Driven live,
with the subject's commit deliberately untouched:

```text
verified  0
stale     1
REQ001    last verdict: holds
    the TLA+ specs outside the subject moved since this verdict — the subject's commit does
    not cover them, so re-verify against the current model
```

An in-tree subject carries **no** fingerprint and gains no axis: the subject commit already covers
those specs, and a second axis would flag the same drift twice. Same rule as every other axis — a
verdict carrying no fingerprint is left alone rather than flagged on a basis we cannot establish.

**Where generated files go.** provreq used to write its module beside the spec. A configured root may
be a repository provreq has no business writing into, so it now generates into its own scratch dir
and points SANY at the spec with `-DTLA-Library` (verified against real TLC before being relied on).
The subject's tree is only ever read. That retired two things: the "refusing to overwrite a file
provreq did not write" guard, and the `_TTrace_` sweeper — with nothing written beside the spec,
there is no file to clobber and no trace to sweep. The test that asserted the old refusal was
replaced by one asserting the stronger property directly: a run adds nothing to the spec's directory.

One trap worth naming: a root configured _inside_ the subject is a plausible thing to write, and
without deduplication every definition in it would resolve twice — a spurious `Ambiguous` telling the
operator to disambiguate a file from itself. Files are deduplicated by canonical path.

Live end to end: the spec moved to a sibling directory, the subject left holding only its
requirements, and `REQ001` still reaches `holds — model-checked (bounded)`.

### A parameterised spec is checked under the operator's model (issue #121 / REQ029)

TLC needs a behaviour **and** a value for every `CONSTANT` the spec declares. provreq found the
behaviour itself and supplied no values, so a parameterised spec — the common case in real TLA+ —
returned an honest `inconclusive` and stayed there forever. Unlike #206's banner, TLC's message was
never the problem: `Error: The constant parameter MaxLen is not assigned a value by the
configuration file.` names the constant and says what is missing. This was **missing capability, not
a reporting defect**, which is why no evidence pass was needed to justify building it.

The assignments are declared once, in the companion `provreq.yml`, and written into the generated
`.cfg`:

```yaml
tla:
  constants:
    MaxLen: 3
    Kinds: "{1, 2}"
```

A value is the right-hand side of a `CONSTANT X = …` line — TLA+, passed through, because every set,
record, tuple and model value the operator needs is already expressible there and translating YAML
into TLA+ would be provreq guessing at the model. Numbers and booleans are rendered (`TRUE`/`FALSE`)
since those are the two scalars where the two spellings coincide. A value provreq **cannot** write —
a list, a map — is refused by name rather than dropped: dropping it would leave TLC reporting that
constant unassigned while the operator is looking straight at it in their manifest.

**The model is reported on the verdict, and that is the point.** `MaxLen = 3` and `MaxLen = 10` are
different claims about the same spec, so the assignments ride along with a `holds` exactly as Kani's
unwinding depth does (`kani::Bounds::describe`). The real-engine test drives both directions on one
spec and one claim: it **holds** under `MaxLen = 1` and is **refuted** under `MaxLen = 5`. Live:

```text
REQ001: holds — model-checked (bounded): verified over the states the engine explored, NOT proven
    - TLC (TLA+): holds (model-checked (bounded))
    - checked under the model — Kinds = {1, 2}, MaxLen = 3 (`tla.constants` in provreq.yml)
```

**No seventh drift axis, deliberately.** `provreq.yml` lives in the companion tree inside the
subject, so `subject_commit` already covers a changed assignment once committed — unlike #120's
external specs, which nothing covered. Kani's engine config sets the precedent: report it, don't
fingerprint it.

**An unassigned constant is still left to TLC.** Pre-empting it at grounding (the #119 move) was
considered and rejected: #119 existed because TLC's message pointed into a generated module that had
already been deleted, and here it names the operator's own constant and the file that must assign
it. A check that says nothing new is a check that can only be wrong.

**The live pass found a second defect, invisible to every test.** `--path` defaults to `.`, so the
ordinary `cd <subject> && provreq verify REQ001` handed `locate_spec` a relative subject root — and
TLC runs with its working directory set to provreq's scratch metadir, where `-DTLA-Library=.` names
the scratch directory rather than the operator's. SANY reported the subject's own spec as a module it
could not find: `Cannot find source file for module queue`. It arrived with #120 (before that,
provreq generated _beside_ the spec, so relative resolution happened to land right) and no test could
see it, because every test hands `locate_spec` an absolute tempdir. Library entries are now resolved
where they are built.

### The reported model must be the model that ran (issue #211 / REQ029)

Two defects from the 8th end-to-end pass, both in what #121 had shipped hours earlier, both about
the same thing: whether a verdict's account of the model is true.

**A — an assignment TLC discarded was reported as part of the model.** TLC **silently ignores** a
`CONSTANT X = …` for a name its spec does not declare — no warning, no error, the run completes
(pinned now by a real-engine test, because the refusal is only justified while that stays true).
provreq passed it through and put it on the verdict:

```text
- checked under the model — Ceiling = 99, Drones = {d1, d2}, MaxAlt = 2, …
```

`Ceiling` was declared nowhere. Rename a constant in the spec, leave the old assignment behind, and
the verdict describes a model that never existed — the one failure the reporting exists to prevent.
Now refused by name, and the refusal says what the model _does_ declare, so the fix is one edit
away. `tla_adapter::declared_constants` reads it off the same walk `locate_spec` already does.

⚠️ **A MODEL VALUE IS EXEMPT, and the first live run of this check is what proved it.** `d1 = d1` —
a name assigned to itself — is how a `.cfg` introduces an opaque element, so no spec declares it or
can. The check as first written refused `d1` and `d2` on the very configuration that had reached
`holds` an hour earlier, **with 39 tests green**, including seven against the real engine. Static
green is not evidence on this channel; the live subject is.

**B — the model rode only on `holds`.** #121 followed `kani::Bounds`, which attaches its bounds to
the pass alone. That precedent does not transfer, and #121's own real-engine test is the refutation:
one spec, one claim, `holds` under `MaxAlt = 1` and **refuted** under `MaxAlt = 5`. A Kani
counterexample is a concrete input the operator can run; TLC's is a behaviour that only exists
relative to a model, so a `fails` that says "replay this" while withholding the model is not
handing over a D9 witness at all.

**Each outcome now states its own relation to the model**, because a line that overstates is the
same small lie in miniature:

```text
holds         checked under the model — Drones = {d1, d2}, MaxAlt = 2, …
fails         refuted under the model — Drones = {d1, d2}, MaxAlt = 2, …
inconclusive  the model provreq supplied — Drones = {d1, d2}, MaxAlt = notanumber, …
```

That last line is the case worth seeing: TLC says `Assumption line 4 … of module Base is false` and
names the assumption but not the value that broke it — and provreq is the one that supplied it.

### An `Error:` line can be a banner too (issue #212 / REQ029)

The other half of the 8th pass, and the direct successor to #206. A constant assigned a value the
claim then quantifies over (`Drones: 3` for a sort's model set) reported:

```text
- Error: TLC cannot handle the temporal formula line 6, col 6 to line 6, col 59 of module provreq_req001:
```

A sentence ending mid-colon, the cause dropped, and a pointer into a module the operator never
wrote. Real TLC had said:

```text
Error: TLC cannot handle the temporal formula line 4, col 6 to line 4, col 61 of module p:
TLC encountered a non-enumerable quantifier bound
3.
```

**Why #206 missed it.** #206 fixed exactly this shape for SANY's `***` banners and deliberately left
`Error:` lines alone, on the reasoning — written into `tlc::diagnostic` — that _"TLC's own runtime
errors do state their cause there"_. True of the two cases it was written against (the unassigned
constant, the false `ASSUME`), false in general. **A line ending in `:` is announcing that what
follows is the point of it**, whichever family it belongs to, and that — not the `***` prefix — is
what marks a banner now.

**The location was the half that mattered.** `provreq_req001` is generated into the scratch metadir
and taken away with it before the output is read, so the operator was sent to a file they never
wrote, cannot open, and which no longer exists — for a mistake sitting in their own `provreq.yml`.
That is #119's complaint arriving by another route. provreq holds the generated text, so the
dangling coordinates become the line itself:

```text
- Error: TLC cannot handle the temporal formula in the temporal property provreq generated for this
  requirement (`(\A d \in Drones : ([]((~(Airborne(d)) \/ Cleared(d)))))`) — TLC encountered a
  non-enumerable quantifier bound — 3.
```

Where the line cannot be quoted, the location is **dropped** rather than kept: a pointer that cannot
be followed is worse than none, because it still sends the reader somewhere. The needle is the
module name alone, not `of module …` — TLC reaches for whichever preposition suits the sentence
(`of module X`, `imported in module X`), and assuming one leaves the others dangling.

⚠️ **The live run caught a bug the new unit test was too weak to see.** TLC prints a range —
`line 6, col 6 to line 6, col 59` — and searching back once lands on the _second_ `line`, leaving
`line 6, col 6 to` stranded in front of the replacement. The test asserted the module name was gone
and the cause survived, both of which were true; only the live output showed the wreckage. The
assertion now checks the whole span goes.

### provreq is a single-operator tool (issue #122 — A5-A stays deferred, decided 2026-08-05)

`serve` is one subject, one operator, one machine. **This is the product's shape, not a stage it is
passing through**, and the deferral of hosted multi-repo serve (A5-A) is now a decision rather than
a parenthetical.

The limit was already load-bearing in three places, all confirmed against the code before deciding
rather than taken from the issue's own account of itself:

- `server::serve` binds `[127, 0, 0, 1]` — the visible half, and the least of it.
- The router holds a single `Subject`, so **one process serves one repository by construction**.
  Multi-repo is not a missing flag; it is a different program.
- `verify_requirement` calls the synchronous `verify::verify` on the executor, so a request that
  runs Kani or TLC holds its thread for however long the engine takes. There is no auth, no
  tenancy, and no daemon manager.

**Why it stays deferred.** The mechanics of _one_ operator using this on _one_ repository are still
being worked out (#1 is open, and the last several end-to-end passes each found something the suite
could not). Designing tenancy, identity, and a job queue on top of that would be building for a
second operator before the first has worn the tool in — and the queue nobody needed is the classic
way to get one.

There is also a claim buried in the deployment question, which is the part worth remembering if
this is ever reopened: **a hosted provreq proves verdicts in an environment nobody reading them is
standing in.** The proving-environment drift axis (REQ049) exists because where a verdict was
proved is part of what it says. Hosting does not merely relocate the prover; it separates the
verdict from the machine the reader can inspect. That is a question about what a verdict _means_,
and it should be answered deliberately rather than inherited from a deployment choice.

**If it is reopened, it is not one issue.** At least: a job queue so verification stops blocking the
request; identity and tenancy; per-subject isolation of the scratch dirs and generated harnesses
every engine writes; and REQ049 re-answered for a prover the reader does not control.

The boundary is now stated where an operator meets it — `serve`'s own startup line and its
`--help` — rather than only here.

### The empirical rung, and the trace it is over (issues #229, #230 — category 2b)

`Basis` carried two rungs and a note saying the third would arrive with the engine that earns it.
MonPoly is that engine: it reads a log of what actually ran, so its silence is neither `proven` (∀
executions) nor `model-checked` (∀ over a model). **`not-falsified` is now representable**, ranked
below `model-checked (bounded)` so an empirical corroboration never demotes a model check, and
rendered with the limit stated — _a statement about what ran, NOT about what can run_.

The rung is the weakest evidence provreq deals in, so it is the one that must not be claimable bare.
`Evidence::not_falsified` takes what the monitor read as a **required argument**; there is no other
constructor for it. A naked `not-falsified` would overclaim by omission exactly the way a naked
`holds` did before the model line (#121). The browser gives it its own tone and says the limit in
words, because tone alone is a convention the operator has to have learned.

**The trace is the operator's to supply.** provreq reads a log the subject already produces and
never runs the subject — running it is category 3's problem. So the trace is a declaration in
`provreq.yml`, the same move `tla.spec_paths` and `tla.constants` already make, resolved against the
**subject** for the same reason:

```yaml
monitor:
    trace: logs/events.jsonl
    format: jsonl # or `monpoly`, for a subject already writing MonPoly's own log
    time_field: ts # jsonl only — a monpoly log carries its timestamp in the `@` prefix
    event_field: event # jsonl only — a monpoly log names the predicate after the timestamp
    events:
        accepted: { name: msg_accepted, args: [id] }
        succeeded: { name: msg_done, args: [id] }
```

Both formats are accepted, and the unit is named per format rather than flattened: a MonPoly line
carrying three predicates is one **time point**, not three events, and counting it as three would
overstate what was read.

**Reading the block is forgiving; a half-written block is not.** A missing file, a missing block, or
a manifest that will not parse all mean "no monitor configured" — a subject that never wanted one is
not broken by the field existing. But once `monitor:` is there, an empty `trace:`, an unreadable
`format:`, a missing `time_field:`, a `time_field:` under `monpoly`, or an event with no `name:` is
an error naming the key and saying what to write. Silence is a choice not to configure; a
half-written declaration is a mistake the operator is looking straight at.

**Two refusals, and they are the point of the slice.** A **missing** trace is a loud failure naming
the path and saying outright that provreq will not read a missing log as `no violations`. An
**empty** trace is refused too: zero records cannot falsify anything, so monitoring one would report
`not-falsified` having observed nothing at all. A record that will not parse is an error naming its
line rather than a record quietly dropped — a dropped record shrinks the extent, and the extent is
the whole claim.

**The trace is a seventh drift axis**, for the same reason as out-of-subject specs and on a much
faster clock: the subject's commit does not cover a log the subject _produced_, and that log grows.
Without it a `not-falsified` verdict would read `fresh` against a trace that has since recorded the
very violation it says it did not see. Same rule as every other axis — a verdict carrying no trace
fingerprint (every category-1 and 2a verdict in existence) is left alone rather than flagged on a
basis we cannot establish.

### A 2b binding resolves against the declaration, not the code (issue #231)

Category 1 resolves a binding against the subject's Rust, 2a against its TLA+. **Category 2b
resolves against neither.** A 2b claim speaks of events that appear in a log, so whether `accepted`
is a real observable is answered by whether the trace _declares_ it — not by whether some Rust
function happens to be named `accepted`. Resolving against the code would bind the claim to the
wrong artifact entirely and would silently succeed on a subject whose logging says something else.

The binding names the **alias** the operator declared the event under (`--ground accepted=accepted`),
exactly as a 2a binding names a TLA+ definition. The alias is provreq's handle; the `name:` inside is
the trace's own spelling, which the adapter needs when it writes MonPoly's signature and the operator
should not have to repeat. Arity is checked against `args:` and is not negotiable — REQ026/#119
already established that a binding checked at the wrong arity is a binding that proves nothing.

**The decision this slice had to make: does a declared event that never occurs park, or ground?**
It **grounds, with a warning**. The split is _declaration decides grounding; occurrence decides
evidence_. Grounding asks whether the symbol binds to a real observable, and the declaration answers
that. Parking on "not in the trace right now" would make grounding depend on a file that changes by
the minute — #230 made the trace a drift axis precisely because it moves — and a binding that flipped
between grounded and parked as a log grew would be reporting the weather, not the requirement.

But a policy over an event that never fires is **vacuously satisfied**, and a monitor would report
`not-falsified` having seen nothing of it. So the dry-run says that outright, before a verdict is
produced from it:

```text
accepted → `accepted` resolves to the declared event `msg_accepted` taking (id)
      ⚠ declared, but it does not occur even once in the trace as it stands — a policy over an
      event that never fires is vacuously satisfied, and a monitor would report `not-falsified`
      having seen nothing of it
```

"Never occurred" and "could not read the trace" never render the same. The second says _unknown —
not zero_, because a dry-run must not fail on a log that does not exist yet; the loud version of that
question is `Extent::read`, at verification time.

Counting occurrences is what added `event_field:` to the manifest — a jsonl record needs a key naming
its event, the same way it needs one naming its timestamp, and guessing either would fail quietly
rather than loudly. `grounding::verdict` and `detail::grounding_report` now take the whole
`Resolutions` rather than its maps positionally, which is what that type's own doc comment already
argued for: the maps are produced together and travel together, and splitting them at a call site
only invites passing three of the four.

### A metric deadline lowers to negated MFOTL (issue #232)

`leads_to … within T` is the pattern only category 2b can decide — `src/tlc.rs` has refused it since
Issue #121, naming an engine that did not exist. It now lowers, and **three rules shape the output, all
measured against the real MonPoly before a line was written**:

1. **No leading `ALWAYS`.** MonPoly evaluates at every time point implicitly, so emitting one is not
   redundant but fatal: _"The formula contains an unbounded future temporal operator. It is hence
   not monitorable."_
2. **Emit the NEGATION.** MonPoly reports what _matches_. Handed the policy it printed `true` at
   every satisfying time point — and the one **violating** point was absent from that output
   entirely. Handed the violation pattern it prints nothing on a clean trace and the offending tuple
   on a dirty one. Read the policy form as "output means violation" and every satisfied policy
   reports as refuted.
3. **The quantified variable stays FREE.** `¬∀x. P(x)` is `∃x. ¬P(x)`, so the violating value _is_
   the witness — but `EXISTS id. …` printed a bare `true` and threw that witness away. Free form
   prints `(2)`.

```text
(accepted(m)) AND NOT EVENTUALLY[0,30] (done(m))
accepted(id:string)
done(id:string)
```

The signature names the **trace-side** names (`done`), not the aliases the operator binds
(`delivered`), and declares only the events the formula actually uses. Arguments are typed `string`
deliberately: the declaration records argument names rather than types, this lowering never emits a
comparison or an arithmetic term, and a numeric-looking value still matched and still produced its
witness under `string`.

**The deadline is converted or refused, never rounded.** `30s`/`5m`/`2h` become whole seconds;
`500ms` is refused, because `EVENTUALLY[0,0.5]` is a **parse error** in MonPoly's interval grammar
and rounding it up to a second would silently widen the very bound the requirement is about.

**The emitted formula is shown on dry-run**, with a sentence saying what it is. A generated temporal
formula the operator never sees is a claim they cannot check, and one that is the negation of what
they wrote makes that worse rather than better.

**The both-direction test is the point of the slice, not decoration.** The inversion above is the
likeliest bug in the whole 2b arc, and it cannot be caught by asserting on a string: the same
`AND NOT` that makes silence mean "clean" would, if dropped, make silence mean "broken". So two
`#[ignore]`d tests drive the real engine — deadline met produces no output, deadline missed produces
the offending id at exactly one time point — with a `monpoly` CI job to keep them running. MonPoly
builds from bitbucket with ocaml + dune and nothing else.

**Found live, and fixed here: a quantified 2b claim could never ground.** The dry-run showed the
sort `Message` being looked up as an event and parking — and every claim this lowering emits is
quantified. A monitor binds a quantified variable from the trace's own argument values, so unlike a
model checker (which needs a finite domain to enumerate) there is **no domain to declare and nothing
that could be wrong**; a 2b sort now resolves by construction and says so.

### MonPoly runs, and its silence is read honestly (issue #233 — the 2b arc closes)

The last slice, and the one that produces a verdict. Category 2b now goes prose → PRL → binding →
formula → **a real answer from a real engine**.

**⚠️ The exit code carries no information — it is 0 for everything**, measured in slice 0 and the
trap this slice exists to survive:

| situation | exit |
| --- | --- |
| clean run, no violations | `0` |
| violations found | `0` |
| **formula refused as not monitorable** | **`0`** |

An adapter trusting that exit code reports a _refusal_ as a passing verdict — a fabricated `holds`
of exactly the kind REQ024 and REQ051 exist to prevent. The sibling trap runs the other way and is
already recorded for engine probes: a blanket non-zero rule is equally wrong, because Creusot and
TLC exit 1 while perfectly healthy. **So the output is parsed, and the refusal is detected FIRST** —
a refused formula also produces no violation lines, so checked in the other order it is
indistinguishable from a clean run.

Three answers, three verdicts, all three driven against the real engine:

```text
holds         REQ001: holds — not-falsified: … a statement about what ran, NOT about what can run
                  - not falsified over — logs/events.jsonl — 2 events, 100 … 110
fails         REQ001: fails
                  - the deadline was missed 1 time(s) — checked over logs/events.jsonl — 2 events…
                  witness: @100. (time point 0): (late7)
inconclusive  REQ001: unknown (inconclusive)
                  - the monitor trace `logs/events.jsonl` does not exist (looked in …) — provreq
                    will not read a missing log as `no violations`
```

That third line is #230's refusal finally reaching a verdict rather than a return type. The witness
is MonPoly's own violation line — timestamp, time-point index, and the offending tuple — which is
the most legible counterexample any engine here produces after TLC's.

**Where the binary comes from: `PATH`, with a `MONPOLY_BIN` override.** MonPoly is a single portable
executable, so it probes like Kani rather than needing the toolchain welding Creusot and Prusti do;
the override mirrors `TLA2TOOLS_JAR` and lets a vendored build be pointed at without touching the
operator's `PATH`. Its `-version` prints `MonPoly (development build)` with no version number, so a
probe reports `available (unknown)` rather than inventing one. Baking it into the devcontainer image
is left as its own change — it is packaging, and nothing here waits on it.

**A jsonl trace is converted to MonPoly's log syntax; a `monpoly` one is passed through untouched.**
provreq does not rewrite a log the subject already produces in the engine's own syntax. Conversion
requires **numeric** timestamps: MonPoly's log grammar takes a number of time units, provreq carries
no date parser, and an ISO-8601 string is refused with the value it found rather than guessed at — a
wrong epoch would silently move every deadline in the claim. A record naming no declared event still
occupies its time point, because dropping the line would move every later event closer to its
trigger.

Nothing is written to the subject: signature, formula and converted log all go to a scratch
directory that is removed when the run ends, so there is no generated file to clobber an operator's
and none to sweep afterwards.

### The operator declares the deployment a UI check drives (issue #239 — category 3 opens)

Category 3 is the last unwired world (#223), and it differs in kind from every other: a UI driver
observes **one run of a live system**, so it can establish that a requirement _fails_ — with the
most legible counterexample in the tool — and can never establish that one holds. The verdict
vocabulary for that already exists and needed nothing new: `Basis::NotFalsified` with its required
`over` (#229). The inversion is worth stating plainly, because it is unlike every category before
it: **a category-3 `fails` is the strongest evidence provreq deals in, and its `holds` is the
weakest** — both out of the same single run.

**Selenium vs Playwright was settled by measurement, not preference.** Nothing browser-shaped is on
`PATH` in this container — no `chromedriver`, no `geckodriver`, no browser — and a driver is
reachable anyway:

```console
$ curl -s http://172.17.0.2:4444/status
ready: True | Selenium Grid ready.
 node http://172.17.0.2:4444 slots: [('chrome', '124.0')]
```

W3C WebDriver is an HTTP protocol, not a program. So the engine is drivable here with **nothing
installed**, where Playwright would need node plus per-browser downloads in whatever environment
provreq runs in. The consequence for `engine.rs` is that `EngineProbe { bin, args, version_marker,
min_version }` cannot express this engine's probe: the right check is `GET <endpoint>/status` →
`value.ready`, which is _stronger_ evidence than a `PATH` hit — a file existing versus the grid
stating it can seat a session, and naming the browser it would seat. That probe lands with the run;
this slice only makes the endpoint declarable.

**The 2b rule carries over one category further out.** #230 settled that the operator supplies the
trace and provreq never runs the subject. Here the operator declares a **deployment that is already
up**: provreq never starts it, health-waits on it, or tears it down. That is what keeps this slice
small — no lifecycle code at all, and the same seam category 2b already proved out.

```yaml
ui:
  endpoint: http://localhost:4444      # the WebDriver grid (optional)
  base_url: http://localhost:8080      # the deployment under check
  steps:
    open_cart:  { goto: /cart }
    checkout:   { click: "button[data-test=checkout]" }
    sees_total: { text_present: "Order total" }
```

**`endpoint` is deliberately the odd field out, because it is a different party's fact.** `base_url`
and `steps` describe the _subject_ — required, committed, and what the drift axis fingerprints.
`endpoint` describes _the machine provreq happens to be running on_: which grid is reachable from
here. So it is optional in the manifest and overridden by `WEBDRIVER_URL`, the same shape as
`MONPOLY_BIN` (#233). An address that differs per operator does not belong welded into a committed
file — #225 is open against exactly that mistake for the LLM endpoint, and there was no reason to
make it twice. Consequently **no endpoint configured** and **no engine wired** must never render the
same: the first is the operator's to fix in their environment, the second is provreq's, and
conflating them sends them looking in the wrong place.

Forgiving to read, strict once present, as everywhere else: a missing file, missing block or
unparseable manifest is "no UI check configured"; a block that _is_ there and is half-written is an
error naming the key. A URL without a scheme is refused here, where the operator can still see which
field caused it, rather than surfacing far downstream as an unexplained connection failure. A step
is **exactly one** action — zero declares nothing, and two would leave the running order unstated,
where silently picking one makes the manifest mean something unreadable from the manifest itself.
The step vocabulary is three (`goto`, `click`, `text_present`) because three is what makes a check —
go somewhere, do something, observe something — and every entry has to be both lowered to and driven
over the wire, so an unused one is a liability in two places.

**The 8th drift axis is the first with no file behind it.** `spec_fingerprint` (#120) and
`trace_fingerprint` (#230) both hash bytes on disk; a running deployment has none. So this hashes the
**declaration** — `base_url` plus the steps — and the gap is accepted knowingly rather than
overlooked: it sees the operator's check change, and it is **blind to the deployment moving
underneath it at the same URL**. The documented extension is hashing an operator-named version
endpoint's response, deferred because it only works if the subject exposes one and a flaky endpoint
would manufacture phantom drift. The grid endpoint is deliberately _excluded_ from the fingerprint:
it is where the browser ran, not what was checked, and since it can come from `WEBDRIVER_URL`,
folding it in would make a verdict go stale the moment a colleague ran against their own grid.
"Where this was proved" is already the environment axis (REQ049).

Three fingerprints of the same type also stopped fitting as positional arguments: `DriftAnchor::
current` now takes a named `Fingerprints { spec, trace, ui }`, because swapping two `Option<String>`
axes compiles cleanly and mislabels every drift the operator is later shown. `verify::
current_fingerprints` is the single place all three are read, so the browse surfaces cannot anchor
against a different set of axes than `verify` stamped.

### A UI binding resolves against the declared steps (issue #241)

`BindCategory::Ui` had a label and no adapter, so a category-3 binding fell through
`grounding::verdict`'s catch-all to "dry-run deferred — engine not wired yet" and a cat-3
requirement could never ground, whatever the operator declared. It resolves against the **declared
steps** now, by the alias they were written under — #231's rule one category further out, and for
the same reason: a cat-3 claim speaks of what a browser does to a running page, so whether
`checkout` is a real observable is answered by whether a step called `checkout` was declared, not by
whether some Rust function shares the name.

**The read-back names the action, not just the alias:**

```text
checkout → `checkout` resolves to the declared step `checkout` —
           click the first element matching `button[data-test=checkout]`
```

A read-back that hides the selector cannot catch a check that clicks the wrong thing, which is the
entire point of the D13 dry-run.

**A sort does NOT resolve at category 3, and this is the decision the slice turns on.** Category 2b
answers the same question with `TraceBound` (#232): a monitor binds the quantified variable from the
_trace's own argument values_, so there is no domain to declare and nothing that could be wrong.
_That reason does not exist here._ A driver runs a fixed script against one deployment; there is no
set of values for a variable to range over and nothing to draw a domain from. So the mirror-image
answer is the honest one — `UiResolution::NoDomain`, which parks. Copying `TraceBound` across would
have grounded a claim nothing can drive, and the operator would only have found out at lowering,
after the read-back had already told them it was fine. The refusal names category 2b as the one that
_can_ bind such a variable, so an operator told "no" is not left with nowhere to go.

Three smaller calls, each the analogue of one #231 made:

- **A step takes no arguments** — it is a fixed action on a fixed selector, so a requirement applying
  the symbol to arguments is refused at the binding, where the operator can see it, rather than
  surfacing as something incomprehensible from the driver.
- **No occurrence analogue, and none was invented.** 2b's vacuity warning counts events in a trace
  that already exists. There is nothing here to count: nothing has run, and nothing may ever run. A
  read-back implying otherwise would be worse than silence.
- **Grounding never consults the grid.** Whether `WEBDRIVER_URL` or `ui.endpoint` is set is a fact
  about the operator's machine (#239). A binding that flipped between grounded and parked as a
  container came and went would be reporting the weather rather than the requirement.

**The membership-vs-rule trap fired a fourth time, and this one had predicted itself.**
`verdict_defers_the_categories_with_no_observable_world` asserted that a cat-3 binding parks with
the reason `deferred`, and its own comment read: _"Category 3 is the last one left… what must not
change is the rule itself."_ Wiring category 3 removed the last unwired category, so there was
nothing left to re-point it at. It pins the rule directly now, across all four categories: **a
binding the caller did not resolve parks — the caller having skipped a symbol is never evidence
that it grounds.** That is one site of the slice-5 sweep, done early because it had to be.

**Two read-back chains were one too many, and the live run is what found it.** 565 green tests said
the slice was finished; the real CLI disagreed with itself — `verify` answered `GROUNDED` while
`draft --dry-run` printed "dry-run deferred — engine not wired yet" for the same bindings, because
the CLI and `detail::grounding_report` each hand-rolled their own if-else chain over the resolution
maps and only one had gained a `ui` arm. That is the #218 defect in another guise: one binding
reading differently by path. Fixed at the root rather than by adding a fifth arm twice —
`Resolutions::describe(binding)` is now the single answer and both surfaces call it, which deleted
both chains. A symbol no resolver answered for reads "no resolver answered for it, so it does not
ground", never "engine not wired": the engine _is_ wired, and blaming it would send the operator
after the wrong thing.

**`--ground` says "Bound", not "Grounded".** It attaches a binding; whether that binding _grounds_
is a question only the resolvers can answer. For a typo that parks the moment anything looks at it,
the old wording announced:

```text
Grounded checkout → `chekout` (category 3, probed fidelity).
```

— the opposite of what had happened, in the one line the operator reads before moving on.

### A category-3 claim lowers to a step script (issue #243)

The fact that decides this slice: **a UI driver makes the trace it then judges.** A model checker
explores a state graph it did not build; a monitor reads a log the subject wrote (#230 — provreq
never runs the subject). A driver performs the actions _and_ observes the result. So a claim whose
outcome is decided by the script rather than by the deployment is not a check at all — it passes
because provreq made it pass.

Slice 1's step vocabulary already splits along the line that matters: `goto` and `click` are
**actions** the driver decides; `text_present` is an **observation** the deployment decides. Hence
the rule `ui/script.rs` enforces — **what a claim asserts must be an observation**:

```prl
require { checkout leads_to sees_total }   # action → observation: real
require { open_cart leads_to checkout }    # action → action: refused
```

The second is refused with the reason and a way out, because a vacuously-passing UI check is worse
than none: it yields a `not-falsified` verdict indistinguishable from the real thing.

**`after` is the ordering mechanism, taken from the language rather than invented.** The declaration
keys steps by alias with no order, and a claim names only the steps it mentions — so nothing would
ever run the setup. The AST already carries Dwyer scopes, and `after open_cart` means exactly
"having done that first". A monitor defers every scope (#232) because it reads a trace from its
start; a driver is _already_ performing actions, so a setup action is the same kind of thing it does
anyway. Conjunction in the antecedent is refused instead: `and` would have to be read as ordered,
and it is not.

**`eventually` is refused, deliberately.** Over a single run with a fixed step count, `always p` and
`eventually p` lower to the identical script — do the setup, look once. Lowering both the same way
would let the verdict imply a distinction it never checked, so only `always` is accepted (the
reading one look supports) and the refusal tells the operator to write `always` if that is what they
meant. `precedes` is refused because the script _is_ the order — it would be answered by what the
operator wrote rather than by the deployment, and would hold every time. `occurs at most` is refused
because a script performs a fixed number of actions, so the count is authored rather than observed.

**#232's "emit the negation" rule does NOT carry over, and the code says so outright**, because
anyone arriving from the 2b arc will look for it. MonPoly had to be handed the violation pattern —
the policy form prints `true` at every satisfying point and omits the violating one, a measured fact
about how that engine reports. A driver asserts directly and reports its own pass/fail, so a
`UiClaim` carries the claim as written.

The read-back shows the script, and leads with what the evidence is:

```text
  What a driver will run (one execution of one deployment — it can show the
  requirement BROKEN, and can never show it holds):
    1. navigate to `/cart`
    2. click the first element matching `button[data-test=checkout]`
    3. then check the page contains `Order total`
```

⚠️ **`--set` clears a draft's bindings**, so a live check that edits the candidate has to re-`--ground`
before `--dry-run` says anything but "no grounding bindings yet". Cost an entirely confusing minute
during #243's live run.

### A UI check drives a real browser (issue #245)

The step script from #243 is now driven through a W3C WebDriver session, and category 3 is wired —
the last one. `provreq engines` no longer has a `NOT WIRED` row.

**A step that fails to _run_ is not a refutation.** This is the rule the slice exists for, and it is
the mirror of #233's: there, every MonPoly answer arrived on exit code 0, so a refusal could pass
for a clean run. Here the danger runs the other way and is more tempting, because the wrong answer
_looks like the tool working_.

| what happened                       | who decided it     | verdict                  |
| ----------------------------------- | ------------------ | ------------------------ |
| the grid would not seat a session   | the environment    | `inconclusive`           |
| `goto` could not load the page      | the environment    | `inconclusive`           |
| `click` matched no element          | **the script**     | `inconclusive`           |
| the asserted text was absent        | **the deployment** | `fails`, with a witness  |

A selector that has gone stale means the check never became a check. Reporting it as a failed
requirement would manufacture exactly the counterexample category 3 exists to produce — a red result
nobody can act on, because nothing about the requirement was ever tested. Only the observation step
can refute, and #243 already refuses to lower a claim that asserts anything else.

**The probe is `GET <endpoint>/status`, and that is a gain rather than a workaround.** A `PATH` probe
answers "a file with this name exists"; the grid answers "**I can seat a session right now**", and
names the browser it would seat. `EngineProbe { bin, args, … }` cannot express that, so `Engine.probe`
became `Option<Probe>` with `Command` and `Grid` arms — `Option` keeping its meaning exactly
(`None` = not wired, ours to fix). `detect` gained a companion path, because that is where
`ui.endpoint` lives, and both `provreq engines` and `GET /api/engines` pass it so the two surfaces
cannot disagree. No endpoint, or nothing answering → `Missing` (the operator's environment).
Answering with `ready: false` → `Unusable`, carrying the grid's own words — REQ051's distinction,
one category further out.

```console
$ provreq engines .
  category 3   Selenium (WebDriver)             available (grid seating chrome 124.0)
```

The registry entry stopped saying `Selenium/Playwright driver`. That slash described an undecided
question; driving one settled it, and the name now says what runs.

⚠️ **An empty `alwaysMatch` does not mean "anything you have" — it HANGS.** provreq has no opinion
about which browser runs a check, so the first cut asked for none: `{"capabilities":{"alwaysMatch":{}}}`.
Selenium Grid does not refuse that. It **queues the request until the session timeout**, so a
perfectly healthy grid answers nothing for a full minute and the driver reports "the grid could not
be reached" — a wrong reason for a working system, which is worse than a slow one. Reproduced with
plain `curl` in 90 seconds of silence. The fix is to ask the grid what it has and then ask for that:
the stereotype in `/status` is the operator's own statement about what they run. Guessing `chrome`
would fail identically against a Firefox-only grid.

⚠️ **The W3C element key is `element-6066-11e4-a52e-4f735466cecf`** — and the first cut had it typed
from memory as `…-4f15f4f8fca4`, which no unit test could see. The live grid found it in one run.
The code now reads the response's _single_ value rather than looking up a UUID it can typo.

**The settle window, and why it is one loop.** A deployment is not obliged to have finished rendering
when the driver looks, so the observation is **polled for presence** until it appears or the window
closes (5s / 250ms). One loop, correct in both directions: an asserted text gets the window to show
up, and a text asserted _absent_ gets the whole window to betray itself. Looking exactly once would
turn a slow page into a fabricated `fails` — or a fabricated `holds` — depending only on which way
the claim happened to be written. Measured against a page that renders its text after 2s: it holds,
in 3.0s wall clock.

**The driver runs on its own thread, with its own runtime, and that is load-bearing.** `verify` is
synchronous and is called straight out of an async axum handler (`POST /api/requirements/:id/verify`),
where blocking on a future from the calling thread panics. A fresh OS thread has no runtime context
to conflict with, so one code path serves the CLI and the web surface — which is the whole point of
`verify` being one flow. There is a real-grid test that drives from inside a multi-thread runtime for
exactly this reason: every real caller is in that position, and it was the only one no test covered.

**`Basis::NotFalsified`'s gloss was one engine's sentence on a shared rung, and the live run caught
it.** #229 wrote "no violation appeared in the trace a monitor actually read" when MonPoly was the
only engine on that rung. A UI verdict then printed it — a `holds` produced by clicking a button
announcing itself as a trace a monitor had read. The rung's line now says only what the rung is
("no counterexample appeared in what was actually observed"); _how_ it was observed travels with the
evidence, in the `over` extent each engine supplies. The test checks both engines against it.

**There is a fourth kind of "no" from `provreq install`.** A grid is not installed anywhere: it is a
service you run and point provreq at. "No native install — use a devcontainer" would send the
operator after something that does not go into a build environment, so `install selenium` now says
to run a grid and set `WEBDRIVER_URL`.

**The membership-vs-rule trap fired a fifth time**, in `engine.rs`: a test asserted that category 3's
engine has no probe — true when it was written, false the moment this slice landed. It pins the rule
in both directions now: every registered engine is probed, and an engine with no probe is `NotWired`
and never ready (checked on a constructed engine, since the registry no longer holds one).

What a category-3 verdict looks like, both ways, against a live deployment:

```text
REQ001: holds — not-falsified: no counterexample appeared in what was actually observed …
    - Selenium (WebDriver): holds (not-falsified)
    - not falsified over — one execution of the deployment at http://…:17870 in chrome 124.0.6367.78,
      watching 5s for the result — 1. navigate to `/index.html`; 2. click the first element matching
      `button[data-test=checkout]`; 3. then check the page contains `Order total`

REQ001: fails
  witness (D9 — replay it against the subject to re-check this):
    the page at http://…:17870/index.html did not show `Free shipping`
```

**Not in scope, deliberately: the screenshot.** The most legible counterexample in the tool (#223)
needs somewhere to live, a lifecycle, and an answer for what happens to it on the next run. The
witness carries the replayable script and the URL it stopped at, which is actionable today.

## Engine provisioning — Design A (old, superseded topology)

> **⚠️ The engine SPLIT (artifact-fed vs toolchain-welded) and R-eng-1..4 survive into Design B.**
> What's SUPERSEDED is the **topology** — the two-world container + installable dev-env agent +
> Unix-domain-socket seam (the block starting "Topology (= A5-A instantiated on one host)").
> Design B replaces that seam with the docker socket. Read the split below; ignore the old topology.

Multi-language is the endgame; Rust/qrusty is the **first** target, not the model. The
engines split by a **language-general law** — the dividing line is _what the engine consumes_:

- **Artifact-fed** — consume a portable artifact, need **no** subject build: TLA+/TLC, Alloy
  (spec), MonPoly/MFOTL (a trace — any language emits traces), Viper/Silicon (Viper IR),
  Z3/CVC5 (SMT-LIB). Language-agnostic → **ride in our tool container**, one image, all languages.
- **Toolchain-welded** — must type-check/compile the **real** code with its **real** deps
  (compiled _or_ interpreted): Rust Verus/Prusti, Python Nagini/CrossHair, C Frama-C/CBMC/VeriFast,
  Java KeY/OpenJML, Go Gobra, Ada GNATprove. Every language with a code-level deductive
  verifier has one → **provisioned into the subject's dev env**. Can't be containerized away
  without our container reproducing the subject's build (forbidden — Caution 1). This is the
  irreducible residual: **code-level proof — proved against a commit, not a model — is
  inseparable from the subject's toolchain.** No lowering trick escapes it.

**Topology (= A5-A instantiated on one host):** our container holds our tools + UI backend +
artifact-fed engines; a thin **installable agent** persists in the subject's dev env, exposing
the dev-env toolchain + run-the-built-exe over a socket the container reaches back through.
Socket: prefer a **Unix domain socket bind-mounted into the container** — no listening TCP
port (no attractive nuisance), OS file permissions as authz; localhost + mTLS only if
container networking forces TCP; SSH/TLS is overkill for same-host IPC.

**Architecture consequence — quarantine language-specificity:**

- Language-neutral, shared: the brain, the UI, lowering to portable IR, the artifact-fed
  engine container, the agent seam.
- A **per-language executor adapter** (behind the A5 executor interface) is the _only_
  language-specific place: detect toolchain, detect + **version-check** the welded verifier,
  drive build+verify, collect a trace. Adding a language = adding an adapter, not a rewrite.
  Rust/Verus is adapter #1.

**Engine requirements:**

- **R-eng-1** — engines split artifact-fed (portable-input, containerizable, language-agnostic,
  shared) vs toolchain-welded (need the subject's compiler/interpreter + deps, co-resident with
  the dev-env build); placement follows the **class**, not the language.
- **R-eng-2** — never silently install into the operator's env; detect presence **and
  version-compatibility**, report honestly; provision toolchain-welded engines into the dev
  env (devcontainer feature / documented install), with at most an opt-in, consent-gated setup helper.
- **R-eng-3** — coverage is gated by installed + compatible engines, reported first-class
  alongside A2's formalizability triage ("category unavailable — engine absent/incompatible").
- **R-eng-4** — toolchain-welded verification is handled by a per-language executor adapter
  behind the A5 seam; the core stays language-neutral; adding a language = adding an adapter.

**Sequencing:** for the qrusty walking skeleton the one engine (Verus) is toolchain-welded, so
it lives in the dev env either way → ship A5-B (Verus via a devcontainer feature). The
container-agent split earns its place only when artifact-fed engines (TLA+, MonPoly) come
online. The container topology is the **destination** for the engine zoo, not the skeleton's start.

## Design B — dev-container scope cut + docker-socket seam (rejected)

> **🔴 REJECTED — ADR #104 (2026-07-25), [devcontainer-branch-decision.md](devcontainer-branch-decision.md).**
> The docker-socket branch is a NO-GO; the seam it proposed resolves instead to **detect-and-advise**
> (REQ048, `src/buildenv.rs`), which reports what the subject's build environment offers and explains
> an engine's absence in those terms rather than reaching into a container. Kept in full because the
> reasoning is what makes the rejection legible. Superseded Design A's native-install path and
> agent/socket topology; its engine split and R-eng-1..4 and R-pkg-4 survive into Design C.

**Scope cut (the enabling assumption):**

- **A-scope-1** — the subject repo **must** ship an in-repo dev-container Dockerfile. Native/host
  builds and non-Linux OSes are **out of scope**.
- **A-scope-2** — identify the dev container via the devcontainer spec
  (`.devcontainer/devcontainer.json` → `build.dockerfile` / `context` / `args`); fallback to an
  explicit `provreq` config key. (This repo already uses devcontainers.)

This **deletes** the native single-binary install (R-pkg-1/3 native path), the custom dev-env
agent, and the bespoke UDS/mTLS socket. Rationale: the build env is now itself a container, and the
subject hands us the **authoritative** build recipe (their Dockerfile) — we no longer
reverse-engineer it.

**Topology (primary):** a **generic** tool container (built once, language-neutral: brain,
UI/`serve`, artifact-fed engines, orchestration) mounts the subject repo **+ the docker socket**.
It reads the subject's Dockerfile and builds a per-subject **"dev+engines" image** = subject
Dockerfile + our per-language engine layer (toolchain-welded verifier, version-matched to the
toolchain the Dockerfile pins). Runs it as a **sibling** container; drives build/verify/run via
`docker exec`. **The docker socket is the seam** (replaces the old UDS/agent). The docker-socket
mount is already the established pattern here (git log: _"feat(devcontainer): add mounts … docker
socket"_).

- Lazier variant: FUSE into one image (`FROM subject-build` + our tools/engines, our binary as
  PID 1) — no socket, but a per-subject rebuild of the tool layer. Default to the
  two-container / docker-socket split.

**Build-image construction — EXTEND, don't DUPLICATE:**

- **Preferred — extend:** `FROM subject-dev-image` (target its **builder** stage if multi-stage)
  plus our thin tool/engine layer. Inherits the exact build env as an opaque authoritative base;
  no reconstruction. Multi-stage "slim final stage lacks the toolchain" is handled by targeting
  the builder stage — still extension, not merge.
- **Why NOT a per-run LLM-merged compound Dockerfile:**
    1. **Fidelity = soundness.** A copy can drift from the real env (base digest, toolchain patch,
       flag, env var, layer order → different resolved deps). You then verify a slightly-different
       program and the gap is **invisible** — a green check against a duplicate. Kills "proved
       against the real code" (A4).
    2. **Puts the LLM in the trusted build base.** The whole trust boundary keeps the LLM untrusted
       (D11 gated by read-back + human confirm). LLM-authoring the env the proof runs in makes every
       verdict's provenance depend on "the LLM merged correctly" — unauditable, in the most
       safety-critical spot.
    3. **Non-determinism breaks provenance/staleness (A4):** same commit → possibly different image
       → unreproducible verdict.
    4. **Fork that rots:** a duplicate must be re-merged on every upstream Dockerfile change; `FROM`
       inherits for free.
    5. **LLM never sees the invocation:** `--build-arg`s, secret mounts, registry auth, build
       context — a text merge can't reconstruct them; the devcontainer spec hands extension the
       args/context.
- **If a base truly isn't layerable:** the LLM may **draft** a compound Dockerfile, but treat it
  like any other LLM output — human-reviewed, committed, **pinned** artifact in the companion tree
  (A6 write-through-review gate), never a per-run generation.
- **Honest-provenance rule (always):** a verdict records **what** it was proved against; if the
  env is ever a reconstruction (not the inherited original), provenance says so and verdict
  **strength is downgraded** — a proof-against-a-duplicate must never masquerade as
  proof-against-the-real-thing.

**Caution 1 rewritten:** was "never teach our container to build the subject." Now: we **may**
build the subject — but **only** via the subject's own in-repo dev-container recipe, **never** by
reimplementing its build.

**What survives from Design A:** the engine split (artifact-fed vs toolchain-welded) and
R-eng-2/3/4 survive, but no longer drive **topology** (there's one build env now) — the split lives
on only inside the per-language adapter (which engine to layer, which version to match; version-match
moves to image-build time). R-pkg-4 (`serve` + embedded UI) survives, hosted in the tool container.

**Honest cost (why this may still not be operationally possible):** repos with no in-repo Dockerfile
are unsupported; the derived build inherits the subject Dockerfile's needs (private registries,
secrets, base images); the **docker socket is a privileged (root-equivalent) seam** — that's the
trust cost that replaces Design A's "attractive-nuisance listening port".

## Design C — seam-free native provisioner, platform-scoped (decided)

> **🟢 DECIDED — ADR #98 (2026-07-24): GO, tiered.** See [design-c-decision.md](design-c-decision.md).
> The light tier ships (`provreq install tlc` / `kani`, consent-gated, `src/provision.rs`, REQ046/REQ047);
> the heavy tier (Prusti, Creusot) is dev-container-first by decision, with subject-grounded advice when
> an engine is absent. Design A resurrected + the platform-scoping insight, with
> two moves that A and B both missed: **the seam is removed**, and **B is folded in** as one build-env
> strategy rather than a competing design. Inherits R-pkg-4 (`serve` + embedded UI) and the engine
> split as a per-adapter concern (no longer a topology concern).

**The shape:** a **native per-platform executable** the operator runs in their own dev env. It is the
**front door** — installer, supervisor, and UI host in one. Prebuilt for the platform targets we
choose to support: Windows / macOS / Linux × x86_64 / arm64 (6 binaries), each shipped only for the
`(OS, arch, language)` classes we commit to. It **provisions** the tools the operator needs into their
dev env (consent-gated, version-checked — R-eng-2), **manages running** them plus the non-intrusive
engines, and hosts the **embedded web server + browser-driven UI** (R-pkg-4, unchanged).

**The key move — no seam.** One native supervisor process, running _in_ the dev env, manages
**everything as local child processes**: both toolchain-welded verifiers _and_ artifact-fed engines
(TLC, MonPoly, Z3, CVC5 are language-agnostic binaries — nothing forces them into a separate
container). Consequences:

- Design A's Unix-socket + dev-env agent → **gone.**
- Design B's docker socket (root-equivalent, the trust cost) → **gone.**
- No agent, no socket, no sibling containers, no container-reaches-back. Just processes on the host.

**The engine split survives but stops being topology.** Artifact-fed vs toolchain-welded no longer
decides _where things run_ (it's all one host now) — it only decides _what the provisioner installs
and how it invokes it_. The split lives on inside the per-language executor adapter (R-eng-4),
exactly as in B, but with no image-build step.

**Soundness is cleaner than B.** B derives an image (`FROM subject-image`) and must keep worrying
about drift and honest-provenance downgrades. Design C runs the verifier in the **literal** dev env
against the **literal** commit — there is no derived artifact to drift from. Provenance is clean by
construction: proved on this machine, this toolchain, this commit (A4 preserved, arguably better).

**The narrowing assumption (this is what makes it tractable, not a non-starter):** supported platforms
are an **explicit input, not auto-discovery**. We do not adapt to the whole diversity of dev
environments; we ship a provisioner for a **finite committed matrix** of `(OS, arch, language)`
classes. Rust-on-Linux-x86_64 first. Each supported requirement class = its own installable version.
This is the "installer, not monolith" reframe made concrete — it turns the "wide wide world" problem
from infinite to enumerable.

**B folded in, not competing.** The executable **detects** the subject's build-env strategy:

- Subject ships a dev-container (devcontainer spec, A-scope-2)? → **use it as the build env**
  (Design B's `FROM subject-image` inheritance — best fidelity when the toolchain is exotic/pinned;
  requires the docker socket _only in this branch_).
- No dev-container? → **provision the toolchain natively** on the host (the Design C path).

So B stops being a rival topology and becomes the "there's an authoritative Dockerfile, inherit it"
branch inside C's front door. This resolves the A-vs-B deadlock: **neither wins outright — the
provisioner picks the build-env strategy based on what the subject offers.**

**Honest cost (what C gives up vs B) — and the stance that resolves it:** when the subject has _no_
Dockerfile, C must **reproduce/provision** the toolchain on a bare host across the supported platform
matrix — i.e. C signs up to be a **cross-platform package manager for specialist verification tools**.
B got the build env _by inheritance_ for free (`FROM their-image` just works, even for exotic pins);
C's native branch has to construct it, which is genuinely harder for a repo pinning an unusual
toolchain version. The platform-scoping assumption bounds this — and the **stance makes it a
non-blocker: provisioning is best-effort with graceful degradation.** C installs what it can; **a tool
that won't install in a given environment simply removes its own capabilities for that user** — the
categories that need it report "unavailable — engine absent/incompatible" (this _is_ R-eng-3's
coverage gating), and everything else still works. The provisioner is **never obligated to succeed
everywhere**; a failed install **narrows the feature set for that user, it does not fail the tool.**
There is no all-or-nothing "must be a universal package manager or it's worthless" — each tool's
install outcome gates only its own capabilities, honestly surfaced.

**Requirements deltas (vs A/B):**

- **R-eng-2 becomes the core, not an afterthought:** detect-presence + version-compat + consent-gated
  install _is_ the executable's primary job, across the supported platform matrix.
- **R-pkg-1/3 (native single-binary install) revived** from Design A — but per-platform prebuilt and
  platform-scoped, not "install into whatever env we find."
- **A5 build-env seam becomes strategy-selected:** local-process (native branch) vs docker-socket
  (dev-container-detected branch), chosen per subject, behind one adapter interface.

> Design C supersedes A's socket/agent seam and reframes B as a strategy-select branch; if C holds,
> A collapses into "the seamless case B never had" and B into "the Dockerfile-present case."
>
> **Decided (pressure-test done): GO — see [design-c-decision.md](design-c-decision.md).** Native
> provisioning is tiered: the provisioner carries the light tier (TLC, Kani) as first-class native
> installs; the heavy tier (Creusot, Prusti, MonPoly) is dev-container-branch-first. Graceful
> degradation is load-bearing (Kani has no Windows; opam engines are Unix-only). **Both light-tier
> installs are shipped: `provreq install tlc` (REQ046) and `provreq install kani` (REQ047,
> Linux/macOS only) — consent-gated, confirmed by re-detection, honest on every degradation.**
>
> **The dev-container branch is decided too — see
> [devcontainer-branch-decision.md](devcontainer-branch-decision.md): NO-GO on the docker socket.**
> The deductive engines pin the toolchain rather than adapting to it, and the heavy tier's real
> precondition is subject-side contract adoption, so inheritance buys too little to earn a
> root-equivalent seam. A5's strategy-selected build-env seam resolves to **detect and advise**, not
> detect and exec: detect the subject's dev-container, explain heavy-tier absence concretely, and
> ship the engine layer as an opt-in image/feature the subject adopts in its own repo. Design C's
> deployment half now has no undeliberated piece left. **The detect-and-advise half is shipped
> (REQ048, `src/buildenv.rs`): `provreq engines` resolves the subject's dev-container — `image:`
> first-class, JSONC and all — and explains each missing engine either as a `provreq install`
> command (light tier) or in terms of that subject's own environment (heavy tier). No docker
> interaction of any kind, including a reachability probe.**
