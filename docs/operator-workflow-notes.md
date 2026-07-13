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
