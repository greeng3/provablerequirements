# Design C's dev-container branch — docker-socket go/no-go

**Status: Proposed** (ratified on merge). Decision record for the one piece
[design-c-decision.md](design-c-decision.md) explicitly deferred: "the **dev-container-detected
build-env branch** itself (docker-socket strategy) — a separate slice; this ADR only decides that
native provisioning is worth building for the light tier."

That light tier is now shipped — `provreq install tlc` (REQ046) and `provreq install kani` (REQ047).
So this branch is the last unbuilt half of Design C, and per the previous ADR it is the only path by
which the **heavy tier** (Creusot, Prusti, MonPoly) ever reaches an operator.

## The question

Design C's front door selects a build-env strategy per subject
([operator-workflow-notes.md](operator-workflow-notes.md), Design C): subject ships a dev-container
→ **inherit its toolchain** through the docker socket; no dev-container → **provision natively**.
The native half is done. Should we build the docker half — and in what form?

The trust cost is the crux. Design C's headline move was *removing* the seam: Design A's
UDS/agent and Design B's docker socket both "→ **gone**", the socket named explicitly as
"a **privileged (root-equivalent) seam** — that's the trust cost". This branch re-introduces the
exact thing the design deleted, in one code path. That is only worth it if the branch buys
something no cheaper option does.

## Evidence from this repo's own ground truth

### E1 — the heavy tier's real precondition is **subject-side adoption**, not env access

Prusti does not verify an arbitrary Rust crate. It verifies a crate that **already depends on
`prusti-contracts`**, and provreq is explicit that this is the subject's to supply, not ours to
inject ([src/prusti.rs:12-16](../src/prusti.rs#L12-L16)):

> what Prusti needs instead is the subject's `prusti-contracts` dependency … This module
> **consumes** that dependency … it does not add a dependency … A subject that does not already
> depend on `prusti-contracts` yields an honest `inconclusive`, never a guess.

That path is implemented and tested — a missing dependency is detected from cargo's own feature
error and reported as `inconclusive` ([src/prusti.rs:270-277](../src/prusti.rs#L270-L277), test
`a_missing_prusti_contracts_dependency_is_inconclusive`). Creusot is the same shape: its live e2e
subject needed a `creusot-std` dependency before anything could be discharged.

**Consequence:** handing an unprepared subject a container full of engines changes nothing. It still
answers `inconclusive`, for a reason no build env can fix.

### E2 — "toolchain-welded" runs the *other* way: the **engine** pins the toolchain

The phrase suggests the engine must bend to the subject's toolchain, which is what makes
`FROM subject-image` inheritance sound valuable. The code says the opposite. `cargo prusti` runs
under a **pinned 2023-08 nightly** that the *subject* must pin, and provreq must actively stop its
own newer toolchain from leaking in ([src/prusti.rs:211-216](../src/prusti.rs#L211-L216)):

> Prusti is toolchain-welded to its own nightly, which the subject pins, so the caller's toolchain
> must not leak in.

The Dockerfile records the same coupling from the build side: `PRUSTI_TAG=v-2023-08-22-1715`,
nightly-2023-08-15 "left installed on purpose … `cargo prusti` recompiles the subject with the
prusti driver and needs it at verify time", and a `uuid` cap at 1.10.0 that applies to "any subject
`cargo prusti` resolves" — because current `uuid` pulls an edition-2024 manifest this cargo cannot
parse.

**Consequence:** a subject that can be verified by Prusti has *already* pinned the toolchain Prusti
dictates. Inheriting the subject's env therefore buys far less fidelity than the general argument
for inheritance assumes — the degrees of freedom inheritance protects were never free.

### E3 — the engine layer is a hand-patched, hours-long build that is already a published image

The heavy tier is not scriptable-on-demand work. Creusot's installer **did not build on arm64 at
all** (`error[E0425]: cannot find value URLS in this scope`) and needed a vendored patch to
`creusot-setup`, carried in-tree at [.devcontainer/patches](../.devcontainer/patches). Prusti is a
pinned-tag source build needing `-C prefer-dynamic` (or ~196 rlib-format link errors), a distro
`z3` because the bundled one is x86_64-only, a staged `prusti-contracts` copied into the prusti
home, and a dependency cap.

This repo's answer was not to script that per environment. It pays the build **once per arch in
CI** and publishes a multi-arch tag
([build-devcontainer-image.yml](../.github/workflows/build-devcontainer-image.yml)), which
[devcontainer.json](../.devcontainer/devcontainer.json) then consumes by digest-backed tag:

> Pull the prebuilt multi-arch image instead of building Dockerfile locally: the heavy stages …
> are paid ONCE in CI.

**Consequence:** any option that rebuilds the engine layer on the operator's machine, per subject,
is signing up for a cost this repo already refused to pay even once per developer.

### E4 — the common dev-container case has **no Dockerfile to inherit from**

Design B's A-scope-2 assumes the subject's devcontainer resolves to `build.dockerfile`/`context`.
This repo — the closest thing to a real subject on hand — uses `"image": "ghcr.io/…:latest"` with
no local build at all. A detector must handle the `image` case as first-class, and for that case
"inherit the Dockerfile" is not a thing that exists; there is only a published image.

Note also what devcontainer.json already does: it bind-mounts `/var/run/docker.sock`. The trust cost
is not hypothetical or distant — it is one line in a config, which is exactly why it is easy to
adopt without weighing.

## The option space

| # | Option | Supplies the heavy tier? | Fidelity | Socket needed | Cost |
| --- | --- | --- | --- | --- | --- |
| 1 | **Run provreq inside the subject's dev-container** | Only if that image already has engines | Perfect (literal env) | **No** | Zero — works today |
| 2 | **Derive an image**: `FROM subject-image` + build the engine layer | Yes | High | Yes | E3's hours-long hand-patched build, per subject, per arch, on the operator's machine |
| 3 | **`COPY --from` a prebuilt engine image** onto the subject base | Yes, if it survives the copy | High if it works | Yes | Fragile: an opam switch, a prusti home of `prefer-dynamic` binaries, a JVM and a native `z3` copied onto an arbitrary base — glibc/arch/loader-path roulette |
| 4 | **Mount the subject into our published engine image** | Yes | Reconstruction — subject's system deps and build config absent; provenance must downgrade | Yes | Low; the image already exists |
| 5 | **Detect + advise; ship the engine layer as an opt-in devcontainer image/feature** | Yes, by the subject opting in once | Perfect (it becomes the subject's real env) | **No** | Low; the image already exists |

Options 1 and 5 are the ones Design C's own R-eng-2 anticipated: "provision toolchain-welded engines
into the dev env (**devcontainer feature** / documented install), with at most an opt-in,
consent-gated setup helper."

## Findings

1. **The branch's true value is narrow.** By E1, it helps exactly one population: a subject that has
   *already* adopted `prusti-contracts`/`creusot-contracts` and pinned the engine's toolchain, but
   whose dev-container lacks the engine binaries. A subject that has done the adoption work has
   already touched its manifest and toolchain file; adding an image tag or a feature (option 5) is
   the same kind of edit, made once, in the open, under review.

2. **Inheritance buys less than advertised.** By E2, the toolchain inheritance was the strongest
   argument for the socket, and the deductive engines invert it: they dictate the toolchain rather
   than adapting to it. What inheritance still genuinely buys is the subject's *system* dependencies
   and build configuration — real, but not the soundness argument that justified a privileged seam.

3. **The privileged seam has a zero-privilege substitute for the case that matters.** Options 1 and
   5 reach the same end state — engines resident in the literal build env — with better provenance
   than option 4 and no root-equivalent mount. Design C's own thesis was that the seam is
   removable; this branch turns out not to be the exception that forces it back.

4. **Nothing here is honest without provenance carrying the build env.** Today `Provenance` records
   requirement revision, subject commit and tool version
   ([src/verdict.rs:108](../src/verdict.rs#L108)); a verdict proved in a container and one proved on
   the host are indistinguishable in `verdicts.yml`, and the living loop's drift axes cannot see an
   env change. That gap exists **regardless** of which option is chosen — option 1 alone already
   lets the same subject be verified in two different environments.

## Decision

**NO-GO on the docker-socket seam, for now.** Do not build options 2, 3 or 4. The privileged seam is
not earned by the narrow population it would serve (Finding 1), its headline justification does not
survive contact with the engines (Finding 2), and a zero-privilege path reaches the same place
(Finding 3).

**GO on the honest half of the branch**, which is what the strategy-selection was really for:

- **Detect the subject's build-env strategy and say something useful about it** — resolve
  `.devcontainer/devcontainer.json` per A-scope-2, handling `image` as first-class (E4), and let
  `provreq engines` explain a heavy-tier absence concretely instead of generically ("this subject's
  dev-container is `X`; it does not carry Creusot — here is how to add it" vs "no dev-container
  here"). No socket, no exec, no privilege.
- **Ship the engine layer as an opt-in for subjects** — the multi-arch image already published by
  CI (E3), documented as a base or a devcontainer feature. The subject adopts it once, in its own
  repo, under its own review. This is R-eng-2's "devcontainer feature / documented install", taken
  literally.
- **Make provenance record the build env** (Finding 4), with a drift axis, so a verdict says where
  it was proved and stops being silently reused when that changes.

**A5's "strategy-selected build-env seam" therefore resolves to: detect and advise, not detect and
exec.** The strategy still varies per subject; what varies is the *advice*, not a privileged
execution path.

## Revisit triggers

This is a defer with conditions, not a permanent no. Re-open if any of these becomes true:

- A real subject appears that is contracts-adopted and toolchain-pinned, ships a dev-container we
  cannot ask it to change, and wants heavy-tier verdicts. That is Finding 1's population, in the
  flesh.
- Verification needs to run somewhere the operator cannot themselves run provreq (CI-hosted,
  multi-tenant), where "run it inside the container" stops being available.
- The heavy tier acquires a cheap, scripted, cross-platform install — which would move it to the
  light tier and make this whole branch unnecessary anyway.

## Consequences

- **The next codeable slice is small, reversible, and privilege-free:** a build-env detector plus
  the honest reporting it enables, followed by build-env provenance. Neither requires deciding the
  socket question again.
- **Design C is now fully decided.** Native provisioning: GO, tiered, shipped. Dev-container branch:
  detect-and-advise, no socket. There is no undeliberated deployment half left.
- **The heavy tier keeps a sanctioned home** — the published image — without provreq becoming a
  container orchestrator to deliver it.
- **We do not acquire a root-equivalent seam** in a tool whose entire value proposition is honest,
  auditable provenance.
