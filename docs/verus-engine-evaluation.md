# Verus as a cat-1 deductive engine — evaluation

Status: **ADOPTED** — wired as the fourth category-1 deductive engine (REQ093,
[#585](https://github.com/greeng3/provablerequirements/issues/585)); the
toolchain is baked into the image (#582). The original evaluation that led here
is kept below. Tracking issue:
[#578](https://github.com/greeng3/provablerequirements/issues/578). See also the
practical guide, [`using-verus-in-your-repo.md`](using-verus-in-your-repo.md).

Repo under evaluation: <https://github.com/verus-lang/verus>

## Question

Would [Verus](https://github.com/verus-lang/verus) be a useful addition to
provreq's Rust verification ensemble?

## Short answer

Yes — but with a specific shape. It fills the slot left by Prusti (dead on
modern subjects), rather than adding a new capability. Best used through the
existing leaf-crate / mirror pattern, not pointed at an adopted subject crate.

## Where it fits the ensemble

The cat-1 deductive side is currently:

- **Creusot** — unbounded deductive (via Why3); works.
- **Kani** — bounded model checking; works.
- **Prusti** — unbounded deductive; effectively dead on modern subjects.

Verus is the natural replacement for the Prusti slot:

- SMT-based (Z3), heavily automated.
- By far the most actively maintained Rust deductive verifier.
- Gives a **second independent unbounded deductive engine** behind Creusot,
  which strengthens ensemble corroboration of a `proven` verdict.

## The catch that shapes integration

Verus does **not** verify arbitrary existing Rust via external annotations.
Code must be authored in its dialect:

- `verus! { }` blocks.
- `spec` / `proof` functions, `requires` / `ensures`, loop invariants.
- Only Verus's supported Rust subset.

This differs from Creusot (attributes + pearlite on fairly normal Rust) and
sharply from Kani (plain Rust + a harness). Consequences:

- Verus is **not** a "point it at an adopted subject crate" engine.
- It pays off only through the **leaf-crate / payload-free mirror** pattern
  already used for REQ030 / REQ071 / REQ076, where the proof core is small,
  self-contained, and authored for the prover.
- In that channel it is a strong fit: those cores are exactly the small
  functional-correctness proofs Verus discharges quickly, often with less
  hand-holding than Creusot / Why3.

## Can existing subject code be retrofitted with annotations?

The natural hope is to do for Verus what [`src/prusti.rs`](../src/prusti.rs)
does for Prusti: generate an **additive** harness (a `#[cfg(...)]` module that
asserts the invariant), never edit the subject, and let the engine prove it
against the subject's own predicates. For Prusti that works because Prusti is a
rustc driver that **unfolds the subject's real `#[pure]` bodies** — the proof is
grounded in actual code, and the subject only had to already mark its predicates
`#[pure]`.

Verus breaks every clause of that:

- It is **not** a driver over arbitrary Rust. Code it reasons about must live in
  `verus! { }` and use its supported subset.
- A call into an ordinary subject `fn` is **opaque** — no `ensures`, nothing to
  unfold — so it decides nothing, exactly like a non-`#[pure]` predicate under
  Prusti.
- Making it non-opaque means rewriting the subject's predicates as `spec fn`
  inside `verus! { }` — i.e. **editing the subject**. That violates the additive
  discipline and the read-only-external-repos rule. Dead end for a modern
  adopted subject such as qrusty.

### The one hook that looks like a retrofit — and why it is a trap

Verus offers `assume_specification [path::to::fn]` (formerly
`external_fn_specification`): from the **harness** crate you can attach a spec to
a function defined elsewhere, without touching it. Mechanically additive, so it
is tempting.

But it is `assume`, not prove. The invariant would then rest on specs provreq
**asserted** about the subject's predicates, with no check that the real bodies
match — strictly weaker than Prusti unfolding the real `#[pure]` body. Mapping
that to `proven` (D8's strongest rung) would be an overclaim; the most it could
honestly earn is "proven relative to an assumed, unverified spec", which is
below the bar and arguably worse than an honest `unknown`. Not worth building.

### What is sound

- **Non-Verus subject you do not own:** no sound retrofit exists. Verus reports a
  **ceiling** — structurally the same honest `inconclusive` / `unknown` that
  [`src/prusti.rs`](../src/prusti.rs) already emits, named as "subject not
  authored in the Verus subset", never a guess. (qrusty and any adopted-but-not-
  owned subject stay here.)
- **Leaf-crate / payload-free mirror:** the real payoff for the above — but this
  is **re-authoring the core in the Verus dialect, not retrofitting the
  subject**.

### When you own the subject

The wall above is two constraints stacked: the additive discipline (do not edit
the subject) **and** the read-only-external-repos rule. For a repo you own, the
second vanishes and editing the subject is no longer a violation — it is a
legitimate, one-time authoring step. That unlocks a **sound in-place retrofit**,
not just the leaf mirror:

- You author the subject's predicates as `spec fn` inside `verus! { }`. Verus
  then reasons about the **real** code, so the generated
  `assert(forall|b: ty| claim)` harness earns genuine `proven` — grounded, not
  assumed.
- This is the **exact** Prusti parallel: Prusti's owned-subject precondition was
  "mark predicates `#[pure]`"; Verus's is "author predicates as `spec fn`". Same
  author obligation on code you control, same engine-module shape.
- The `assume_specification` trap mostly dissolves — you write the true
  `spec fn` rather than assuming one. It only resurfaces for **dependencies you
  do not own** inside an owned subject, a narrow and clearly-labelled trust
  caveat rather than the whole mechanism.

Owning the subject removes the **read-only** constraint but **not** provreq's
**additive-at-verify-time** discipline, and the retrofit splits cleanly along
that line:

- **Author-side (your commit, one-time):** predicates become `spec fn`.
- **Tool-side (unchanged):** provreq generates the ephemeral assert harness on
  each `verify` and restores the subject afterward — it never permanently
  rewrites your code.

So owned-only turns the retrofit from "no sound path, use the leaf mirror" into
"a sound in-place path exists, gated on a one-time `spec fn` authoring step the
owner is willing to do" — a **stronger** reason to build the engine, scoped to
Verus's natural domain.

### The reusable piece if an engine is built

`lowering::lower_property` already yields an engine-agnostic
`LoweredClaim { claim, quantified }`. Prusti wraps it
`prusti_assert!(forall(|b: ty| claim))`; Verus wraps the **same** claim as
`assert(forall|b: ty| claim)` inside a `verus! { }` proof fn. A `verus.rs` would
be close to a copy of `prusti.rs`'s `lower` / `assertion` with a different
wrapper and a different `classify` (Verus prints `N verified, 0 errors`). The
additive `run` / attach / restore machinery ports almost verbatim — but is only
usable against a Verus-authored subject, so in practice the engine only ever
fires on leaf mirrors.

## Cost

Another pinned toolchain. Verus ships against a specific Rust nightly plus a
bundled Z3, separate from the 1.99.1 pin and the Creusot / Kani setup. The main
tax is a devcontainer + CI install path that must stay green forever — the same
class of maintenance pain already paid for Creusot (why3 flakes, cvc5
migration) and Kani (unwind config). The proof authoring itself is cheaper than
Creusot; the plumbing is the expensive part.

## Recommended spike (low risk)

Re-prove one **already-Creusot-proven** leaf in Verus as a parallel engine on
that mirror — e.g. REQ076 (sequence induction):

1. Tests toolchain integration and dual-engine corroboration on a core that is
   known to be provable.
2. If the install stays reasonable in the GHCR image, promote Verus to a real
   cat-1 engine as the Prusti replacement.
3. If it fights the devcontainer the way Creusot did, abandon it — roughly one
   day lost, no release risk.

Do **not** expect Verus to verify qrusty or any adopted subject in place; that
stays Kani / Creusot territory.

## Related

- `docs/prusti-fork-decision.md`
- `docs/formalizing-requirements.md`
