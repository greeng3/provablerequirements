# Prusti fork — go/no-go

**Status: Decided — no** (issue #487). Do not fork Prusti and port it to a modern Rust
nightly. Keep category 1 as **Kani + Creusot** for modern subjects. Recorded so the question is
not re-litigated without new information.

## The question

Prusti is the ensemble's third deductive engine (Viper backend). It is welded to a single pinned
Rust nightly, and upstream's newest release builds on a **2023-08 nightly** — which predates
edition 2024 (stable 1.85, February 2025) and the v4 `Cargo.lock` format. So on a modern subject
(qrusty: stable 1.98.1, edition 2024) Prusti fails while building the dependency graph, before it
reaches any requirement. Should we fork Prusti and update it to a nightly new enough to verify such
subjects?

provreq already reports this as a **toolchain ceiling**, not a build failure or a refutation
(REQ063, [`src/prusti.rs`](../src/prusti.rs)), and the category-1 ensemble is **non-blocking**
([`src/verify.rs`](../src/verify.rs)): when Prusti is unusable, Kani and Creusot still run. So the
question is purely whether the fork is worth the cost — nothing is broken today.

## Why a fork is not worth it

The port is three separate piles of work, and the expensive one has no compiler to guide it.

1. **Make it compile.** Bumping ~2023-08 to a ~late-2024/2025 nightly means absorbing roughly a
   year and a half of `rustc_private` churn — MIR, THIR, `TyCtxt`, the query system — through
   `prusti-rustc-interface` and the MIR-to-Viper encoding in `prusti-viper`. Voluminous but
   mechanical; weeks for someone already fluent in rustc internals. The Viper backend (`viper`,
   `viper-sys`, Silicon/Carbon on the JVM) is insulated from this, which is the one mercy.
2. **Make it sound again.** This is the real cost. MIR _semantics_ evolve, not just the APIs. An
   encoding that assumed the old MIR can silently mismatch the new compiler and report a proof for
   something false. A verifier's entire value is trust, so a hastily-ported one is worse than none —
   and there is no compile error to catch the mistake. Re-validating the encoding's soundness against
   the new compiler is open-ended.
3. **Keep doing it forever.** Every newer edition or feature a subject adopts needs a newer nightly.
   A solo fork makes us Prusti's de-facto maintainer, on the same treadmill that froze it.

The strongest available signal is that the original authors (ETH Zürich) have not shipped a
newer-nightly release. That points to under-resourced and hard, not overlooked.

## Why the return is small even if the port succeeded

Prusti is **redundant for the rung**. Prusti and Creusot both top out at the same basis — `proven`
(deductive, over all executions, spec-relative). The verdict aggregate takes the strongest rung any
holding engine earned, so a second `proven` engine never raises the verdict; it only corroborates.
Creusot here tracks `nightly-2026-06-22`, well-aligned with our stable 1.98.1, and already yields
the proof.

Prusti's only marginal value is therefore **secondary**: coverage (occasionally discharging an
obligation Creusot cannot) and independent corroboration (two different proof pipelines agreeing is
higher assurance). Months of specialist work, a perpetual maintenance treadmill, and an open-ended
soundness burden — to buy a sometimes-second-opinion — is deeply unfavorable.

## Current posture (no action needed)

- Category 1 for modern subjects is effectively **Kani (`model-checked (bounded)`) + Creusot
  (`proven`)**.
- Prusti reports its ceiling honestly and does not block the ensemble. Nothing is faked and nothing
  is lost.

## What would reopen this

- **Upstream ships, or has a work-in-progress branch toward, a newer-nightly Prusti.** Then the job
  collapses from "port cold" to "track, rebase, or help land upstream" — an order of magnitude
  cheaper, with upstream review on the soundness-critical parts. This record did **not** verify
  upstream's current recent-commit or PR state; checking `viperproject/prusti-dev` for a
  toolchain-bump branch is the cheap first step if the decision is ever revisited.
- Evidence that Creusot cannot cover a class of category-1 claims we care about **and** that Prusti
  demonstrably can.

## If a second or stronger deductive prover is ever wanted

Prefer contributing a nightly bump **upstream** over a solo fork; or invest in Verus (actively
maintained, tracks recent nightlies — but it wants code written in its own dialect, so it is its own
integration rather than a drop-in) or deeper Creusot. Reviving Prusti is the worst of these options.
