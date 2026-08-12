# Design C — native-provisioning go/no-go

**Status: Proposed** (ratified on merge). Decision record for the deployment half of
issue #1 / [operator-workflow-notes.md](operator-workflow-notes.md), which left Design C as
"the current lean … not yet decided — pressure-test the native-provisioning cost first." This
is that pressure-test, grounded in the repo's own ground truth, and the go/no-go it produces.

## The question

Design C ships a native per-platform executable that **provisions verification engines into the
operator's dev env** (consent-gated, version-checked — R-eng-2), rather than reaching into a
container (A's socket/agent) or deriving an image (B's `FROM subject-image`). The open worry has
always been the honest cost: on a bare host with no Dockerfile, C "signs up to be a cross-platform
package manager for specialist verification tools." Is that tractable, or a non-starter?

The stance in the notes is that provisioning is **best-effort with graceful degradation** — a tool
that won't install simply removes its own capabilities (R-eng-3 coverage gating), it never fails the
tool. This ADR tests whether that stance actually holds up against the real engine roster.

## What is already built (so the cost is only about provisioning)

- **Release/packaging half — done.** [`release.yml`](../.github/workflows/release.yml) is
  tag-triggered, cross-builds all six targets (Linux/macOS/Windows × x86_64/arm64), and publishes
  per-target tarballs/zips **plus `.sha256` checksums plus a `dist-manifest.json`** (the schema in
  [build-and-release.md](build-and-release.md)). REQ001–006 are merged: native binary, matrix,
  published artifacts, CI-on-tag, `serve`, embedded UI.
- **Engine detection — done.** [`src/engine.rs`](../src/engine.rs) probes each engine's presence and
  version and reports readiness (`EngineStatus`), **without ever installing** (R-eng-2's detect
  half). Coverage is honestly gated on what is present.

So the only unbuilt, undeliberated piece is the **install** half of R-eng-2: turning a detected
`Missing` into an `Available` with the operator's consent, and degrading honestly when it can't.

## Evidence: the engine roster as a provisioning-cost table

The repo already contains a **working native-provisioning recipe** for Linux (x86_64 **and** arm64):
the [devcontainer Dockerfile](../.devcontainer/Dockerfile). It is, in effect, the provisioner we
would be productizing — and its comments record exactly how hard each engine was. That is the
strongest evidence available, and it is unambiguous: the cost is **tiered, not uniform.**

| Engine                   | Cat        | Native install (per the Dockerfile)                                                                                                                                | Cost tier  | Cross-platform reach                 |
| ------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------ |
| **TLC**                  | 2a model   | headless JRE + a ~2 MB `tla2tools.jar`                                                                                                                             | **light**  | anywhere with Java                   |
| **Kani**                 | 1 code     | `cargo install --locked kani-verifier && cargo kani setup` (fetches CBMC + SAT/SMT)                                                                                | **medium** | Linux/macOS; **no Windows** upstream |
| **Prusti**               | 1 code     | **source-built** at a pinned tag — JVM + system `z3` + `prefer-dynamic` + staged `prusti-contracts` + a `uuid` version cap; **no release binary for arm64**        | **heavy**  | Linux/macOS; JVM-bound               |
| **Creusot**              | 1 code     | opam switch — `opam init`, why3 depexts (`autoconf`/`zlib`/`gmp`/`m4`/`rsync`), SMT provers (Z3/CVC5/Alt-Ergo); arm64 needed a **custom patch** to `creusot-setup` | **heavy**  | opam → effectively Unix-only         |
| **MonPoly**              | 2b runtime | wired (#233); built from source, OCaml/opam                                                                                                                        | **heavy**  | Unix-only                            |
| **Selenium (WebDriver)** | 3 UI       | wired (#245); **nothing installed** — a grid reached over HTTP at `WEBDRIVER_URL`                                                                                  | **none**   | any host that can reach a grid       |

## Findings — where graceful degradation is _structurally forced_

The pressure-test does not merely say "some engines are annoying to install." It surfaces three
findings that make degradation **mandatory by construction**, not a fallback:

1. **The 6-binary matrix ≠ 6× full engine coverage.** Kani has no Windows support upstream; Creusot
   and MonPoly are opam/OCaml and effectively Unix-only. So on the two Windows targets, category-1
   coverage is _at best_ Prusti (JVM), and 2b never. A Windows operator gets a working tool with a
   **narrower** engine set — exactly the R-eng-3 "unavailable — engine absent/incompatible" path.
   Degradation is the design, not an accident.

2. **The heavy tier is genuinely a package-manager's job.** Provisioning Creusot means driving opam +
   why3 depexts + SMT solvers; Prusti means a source build with a JVM and a native `z3` and a
   contracts-staging dance. The devcontainer needed a **hand-authored arm64 patch** for Creusot and a
   pinned-tag source build for Prusti. Reproducing this natively, per platform, on demand, is real
   sustained work — the "cross-platform package manager for specialist tools" cost is not
   hypothetical.

3. **The light tier is genuinely cheap.** TLC is a JRE plus a 2 MB jar; Kani is two `cargo` commands.
   These are honestly scriptable, consent-gated, per-platform installs with a clean re-detect — the
   provisioner's happy path exists and is small.

## The escape hatch the design already has

Design C folds B in as a strategy branch: **if the subject ships a devcontainer, inherit its
toolchain instead of provisioning natively.** The heavy tier (Creusot, Prusti, MonPoly) is _already
baked into this repo's devcontainer image_ (`build-devcontainer-image.yml` pays Kani setup + the
Prusti source build + the Creusot opam switch once, per arch). So the engines that are expensive to
provision natively are exactly the ones the dev-container branch supplies for free. The two branches
are complementary: **native provisioning carries the light tier; the dev-container branch carries the
heavy tier.** Neither is asked to do the other's hard job.

## Decision

**GO on Design C** as the deployment model — but with the cost model made explicit, which the notes
left implicit:

- Native provisioning is **tiered, not uniform.** The provisioner commits to the **light tier**
  (TLC, Kani) as first-class native installs, and treats the **heavy tier** (Creusot, Prusti,
  MonPoly) as **dev-container-branch-first**: provision natively only where a clean, scripted,
  per-platform recipe exists; otherwise inherit from the subject's devcontainer or degrade.
- Graceful degradation is **load-bearing and confirmed**: it is what makes the platform matrix honest
  (Kani/Windows, opam/Unix) rather than a coverage lie. R-eng-3's "unavailable" verdict is the
  mechanism, and it already exists.

### Recommended first implementation slice (the pressure-test spike, in code)

**Consent-gated native install of TLC on the current platform**, end to end: detect `Missing` →
prompt for consent → run the scripted install (fetch the pinned jar + ensure a JRE) → re-detect →
report `Available` or a specific, honest failure. TLC first because it is the lightest, most
cross-platform recipe and exercises the whole detect → consent → install → re-detect → degrade loop
without the heavy tier's platform hazards. Kani is the natural second (Linux/macOS only, explicitly).

### Explicitly deferred (do **not** build natively now)

- Native provisioning of **Creusot / Prusti / MonPoly** — heavy tier; rely on the dev-container
  branch (already built in the image) until a real subject forces the native recipe.
- **Windows/arm64 native engine installs beyond the light tier** — surface honest "unavailable" and
  let the operator use a devcontainer.
- The **dev-container-detected build-env branch** itself (docker-socket strategy) — a separate slice;
  this ADR only decides that native provisioning is worth building for the light tier.

## Consequences

- The next codeable slice is small and reversible: a `provreq` install path for one light-tier engine,
  reusing `engine::detect` for the before/after readiness check. It doubles as the empirical
  pressure-test — if even TLC's install loop is unpleasant, that is a strong signal to lean harder on
  the dev-container branch before committing further.
- The provisioner never becomes a universal package manager: each engine's install outcome gates only
  its own capabilities, and the heavy tier has a sanctioned non-native home.
- This ADR supersedes the "undeliberated" status of Design C's deployment half in the notes; the
  operator-journey spine (steps 1–6) remains the separate, already-shipped half.
