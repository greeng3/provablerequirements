# Verus as a cat-1 deductive engine — evaluation

Status: evaluation / not adopted. Tracking issue:
[#578](https://github.com/greeng3/provablerequirements/issues/578).

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
