# Operator Workflow — Working Notes (WIP)

> **Status: in-progress deliberation, not a finished design.** Scratch for issue #5.
> Captures the deployment/engine-provisioning thread worked through so far; the operator
> journey itself is still being mulled. Nothing here is final. Builds on the merged adoption
> model in [applying-to-existing-repos.md](applying-to-existing-repos.md).
>
> **Three deployment designs are on the table. All are kept here deliberately:**
>
> - **Design A — native install + dev-env agent/socket** — the earliest direction.
>   **⚠️ SUPERSEDED / kept for reference.** Sections below marked _Design A (old)_.
> - **Design B — dev-container scope cut + docker-socket seam** — the middle direction.
>   **🔶 UNDER CONSIDERATION, not decided.** Sections below marked _Design B (under consideration)_.
> - **Design C — seam-free native provisioner, platform-scoped** — the current lean.
>   **🟢 CURRENT LEAN.** Design A resurrected, but with the seam removed and B folded in as one
>   build-env strategy. Section below marked _Design C (current lean)_.
>
> The operator-journey spine (below) is shared by all three and independent of which wins. Design C
> is the first framing that makes the tool feel **operationally possible** rather than a non-starter —
> but it's a lean, not a commitment.

## Operator-journey spine (proposed skeleton, not yet agreed)

> **📌 Step 1 SHIPPED (2026-07-13, issue #8).** The rest of the spine is still the unfinished
> half — proposed skeleton, not agreed, to be worked through with the operator next. The
> genuine open design questions live at Steps 2–3 (see below); Step 1 was fully specified by
> the settled adoption model (A1–A3) and needed none.

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
status` coverage funnel (`src/status.rs`). REQ009–011. The LLM bulk pre-sort classifier
   (R-triage-1 primary flow) is the deferred next slice; the shipped default is the honest
   prose-floor seed. See the "Steps 2–3 design" section below.
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
   admission → needs-reconfirmation, surfaced in draft display/list). REQ020. Still deferred: D13
   grounding (the axis after this). See the "Steps 2–3 design" section below.
4. **Verify** — run one engine → inspect the verdict tree.
5. **Annotate** — stage the working-tree proof-carrier edit; operator reviews + commits on their own forge.
6. **Living loop** — re-run on drift, act on stale verdicts.

The four emerging questions (triage one-at-a-time vs bulk pre-sort; half-finished-formalization
state; coverage display; grounding no-match) are **answered** in the Steps 2–3 design below.
Steps 4–6 remain skeleton (tracked under the umbrella design issue #1).

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
    ```

    The API key is read only from the named env var, never the file. No `llm:` block → triage uses
    the prose-floor default and says so. Items the model omits/mislabels fall back to stays-prose.

- **Issue #30** — D13 grounding, first slice (REQ021). `src/grounding.rs` binds each PRL vocabulary
  symbol to a concrete observable (`Binding { symbol, category, observable, fidelity }`, D4/D5) and
  dry-runs the **category-1 (code-state)** bindings against the subject's real source
  (`dry_run_code` walks the tree, skips the companion tree + `.git`, substring-matches, capped).
  Bindings persist on the `Draft` (`--ground SYMBOL=OBSERVABLE`), cleared on candidate edit; matches
  are recomputed live, never stored. A requirement grounds only when every symbol is bound in
  category 1 and each binding matches ≥1 span (`--dry-run`); any unbound symbol, no-match code
  binding, or non-code binding leaves it **parked** (`admitted-but-ungrounded`), honestly reported —
  no-match never fakes a verdict (R-ground-1), non-code categories are deferred until their engines
  are wired. Real 2a/2b/3 dry-run, D6 cross-category refinement mappings, and regex/AST-precise
  queries are later slices.

- **Issue #34** — engine coverage report (REQ022, R-eng-2/3). `src/engine.rs` maps each PRL category
  to one engine (R-eng-1 split: cat 1 code = toolchain-welded per-language build toolchain, R-eng-4;
  2a = TLA+/TLC, 2b = MonPoly, 3 = Selenium/Playwright driver), `detect`s presence + best-effort
  version on `PATH` **without ever installing** (R-eng-2 — reports welded / available / missing /
  incompatible), and computes per-requirement `readiness` (pure). `provreq engines` lists engine
  status then, for every admitted requirement, whether its declared category's engine is ready —
  ready only when **every** declared category's engine is (multi-category names each blocker); an
  unparseable or category-less candidate is unroutable, never silently ready. Version minimums ship
  presence-only (machinery typed+tested; thresholds → provreq.yml config when a real engine lands).
  No engine execution / verdicts — that is Step 4.

**Next slice:** Step 4 — the **verdict** object (D7 three-valued evidence tree + D9 provenance) with a
first real engine path. The lightest is **category 1 (code)**: it is already groundable against real
source (#30) and its engine is toolchain-welded + reported ready (#34), so a `provreq verify` can
produce a real `holds/fails/unknown` for a cat-1-grounded requirement while 2a/2b/3 stay honestly
`unknown / no-engine`. Real 2a/2b/3 grounding dry-run + execution follow once those engines are wired
(the Design-C provisioning axis).

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
  hosted (hosted multi-repo = deferred A5-A). `serve` foreground default; `--background` /
  `--port` flags; no daemon manager yet.

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

## Design B — dev-container scope cut + docker-socket seam (under consideration)

> **🔶 UNDER CONSIDERATION, not decided.** Later direction. Still open whether it makes the tool
> operationally possible — the reason for more noodling. Supersedes Design A's native-install path
> and agent/socket topology; inherits Design A's engine split and R-eng-1..4 and R-pkg-4.

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

## Design C — seam-free native provisioner, platform-scoped (current lean)

> **🟢 CURRENT LEAN, not a commitment.** Design A resurrected + the platform-scoping insight, with
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
> A collapses into "the seamless case B never had" and B into "the Dockerfile-present case." Not yet
> decided — pressure-test the native-provisioning cost first.
