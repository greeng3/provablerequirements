# Operator Workflow — Working Notes (WIP)

> **Status: in-progress deliberation, not a finished design.** Scratch for issue #5.
> Captures the deployment/engine-provisioning thread worked through so far; the operator
> journey itself is still being mulled. Nothing here is final. Builds on the merged adoption
> model in [applying-to-existing-repos.md](applying-to-existing-repos.md).
>
> **Two deployment designs are on the table. Both are kept here deliberately:**
>
> - **Design A — native install + dev-env agent/socket** — the earlier direction.
>   **⚠️ SUPERSEDED / kept for reference.** Sections below marked _Design A (old)_.
> - **Design B — dev-container scope cut + docker-socket seam** — the later direction.
>   **🔶 UNDER CONSIDERATION, not decided.** Sections below marked _Design B (under consideration)_.
>
> The operator-journey spine (below) is shared by both and independent of which wins. The open
> question is still whether either design makes the tool **operationally possible** at all —
> more noodling needed before committing to one.

## Operator-journey spine (proposed skeleton, not yet agreed)

1. **First contact** — point at a subject repo → discover its Doorstop layout → propose the
   companion tree + name → operator confirms.
2. **Triage backlog** — classify each item (formalizable-now / falsifiable-only / stays-prose).
3. **Formalize one item** — translate → read-back confirm (D12) → validate grounding dry-run (D13).
4. **Verify** — run one engine → inspect the verdict tree.
5. **Annotate** — stage the working-tree proof-carrier edit; operator reviews + commits on their own forge.
6. **Living loop** — re-run on drift, act on stale verdicts.

Emerging questions to chase (→ candidate requirements): triage one-at-a-time vs bulk
LLM pre-sort + confirm; what state a half-finished formalization must persist; how backlog
coverage is shown; what happens when grounding finds no match.

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
