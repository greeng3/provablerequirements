# Proving requirements mechanically — a cookbook

How to turn a requirement into a **mechanical verdict** (`proven` or `model-checked (bounded)`),
distilled from provreq proving 16 of its own requirements (issue #514). This is the _proof craft_
— the extraction patterns, proof shapes, and engine gotchas. For the CLI pipeline that wraps it
(`init` → `triage` → `draft` → `ground` → `admit` → `writeback` → `verify`), see
[end-to-end-guide.md](end-to-end-guide.md); this document is what makes a requirement _provable_ in
the first place.

Written for anyone formalizing a Rust subject with provreq, including an AI adopting an existing
tree such as [qrusty](qrusty-as-a-subject.md).

## First: is this requirement a mechanical-proof candidate at all?

Most of the leverage is deciding this correctly. Do **not** force a mechanical verdict where one
does not fit — a manufactured proof is worse than an honest asserted verdict.

- **Category-1, deductive (`proven` / `model-checked (bounded)`)** fits a requirement whose claim
  reduces to a **payload-free decision over a finite domain**: a classifier, a fold, a guard, an
  ordering. "The combiner reports the strongest basis present." "The guard reads `Asserted` only
  when every decisive piece is asserted." These have a crisp core you can prove exhaustively (Kani)
  or deductively (Creusot).
- **Category-2/3 or asserted** fits a **system-level or behavioural** requirement: an HTTP endpoint
  exists, a WebSocket message lock is exclusive, a record persists. These do not reduce to a pure
  decision; their honest verdict is `not-falsified`, **asserted** over `Verifies:`-tagged tests
  (the asserted route in the end-to-end guide), or a cat-2 monitor / cat-3 UI check.

A tree of mostly system-level requirements (qrusty's is) will earn mostly **asserted** verdicts.
That is correct, not a shortfall. Reach for the deductive machinery below only for the subset with a
genuine payload-free core.

## The core move: delegate a payload-free classifier

A real function usually mixes the decision with I/O, strings, and allocation. Extract the
**decision** as a payload-free function the real code delegates to, then prove _that_. The proof is
then about the rule the subject actually applies, not a copy.

- Payload-free means enums and small ints — **never `String`, `Vec`, or a struct carrying them**.
  Kani emits a bare `Kani: fails` on heap it cannot model; a payload-carrying sort gives no real
  counterexample.
- If the decision reads a string, reduce it at the boundary to the facts it actually uses. REQ071's
  drift decision reads only _presence_ and _equality_ of two fingerprints, so the real code maps its
  `Option<&str>` to a payload-free `Pair { BothEqual, BothDiffer, OnlyStored, OnlyCurrent, Neither }`
  and the proof is over `Pair`, not strings.
- The proof **sort** must be a named item (`struct`/`enum`), not a tuple or array type — the sort
  resolver reads named items. Its module must be **`pub`** (the generated harness references
  `crate::<module>::<Sort>`; a `pub` item in a private module is `E0603`).

## The leaf-crate escape (Creusot's `DeepModel` wall)

Creusot compiles the **whole crate** before running any harness and intercepts
`#[derive(PartialEq/Eq)]` to demand `creusot_std::model::DeepModel` on every reachable type. A real
application crate with dozens of such derives cannot clear this wall, so a whole-crate Creusot proof
is an open-ended arc (provreq's #531).

The escape (provreq's #483/#536): put each proved core in its **own excluded workspace crate**.

- A crate that is merely a `members = [...]` entry is still built by `cargo creusot` and still drags
  the app crate in. It must be its **own workspace**: a `[workspace]` table in the leaf's
  `Cargo.toml` **and** `exclude = ["<leaf>"]` in the root. Then `cargo creusot` there compiles only
  the leaf plus `creusot-std`. The app still path-depends on the leaf (a path dep needs no shared
  workspace).
- The leaf holds: payload-free proxy types that **derive no `PartialEq`** (that derive is the wall;
  the proofs use pearlite `==`/`match`, which needs no derive), the pure decision the app delegates
  to, a `#[cfg(kani)]` `Arbitrary`, and `#[cfg(creusot)]` `#[logic]` pearlite mirrors.
- Point the requirement at the leaf in the subject's `provreq.yml`:
  `verify: { crates: { <REQ>: <leaf path> } }`. No re-grounding is needed if the leaf's predicate and
  sort keep the grounded names — the resolver finds them by name.
- The excluded crate is skipped by `cargo fmt/clippy/test --all`, so add a leaf step
  (`--manifest-path <leaf>/Cargo.toml`) to CI and the local check target.

## The four proof shapes

1. **Bounded case analysis** — a loop-free decision over a finite enum. Prove a soundness predicate
   `<name>_is_sound(x) -> bool` that always holds; Kani enumerates the sort, Creusot unfolds the
   cases. Example: REQ071's `code_drift(fp: Pair, commit: Pair)`.
2. **Seq-induction** — a fold over an **arbitrary-length** slice. Write the core as an **indexed
   `while` loop** (not `.iter().filter().any()` — Creusot has no contract for those) carrying
   `#[cfg_attr(creusot, invariant(...))]` bounded-quantifier invariants, and
   `#[ensures(result == <core>_logic(.., xs@))]` linking it to a pearlite mirror that states the
   property over the whole `Seq`. SMT discharges prefix-extension automatically — no hand lemma.
   Example: REQ076's masquerade guard.
3. **Finite-lattice max** — an **argmax** fold (the "strongest present" of a small ordered set).
   Do **not** reach for recursion or a sub-sequence lemma: because the scale is a finite lattice,
   "the maximum present" equals "the highest rung that appears," which is **one existential per
   rung**. Example: REQ030 — `strongest(s)` is three existentials, and the loop invariant
   `best == strongest_prefix(xs@, i@)` steps by prefix extension.
4. **Pair reduction** — a decision over _equality and presence_ of inputs. Collapse each input pair
   to a payload-free enum at the boundary so the proved core carries no generics and no `==`, just a
   finite case analysis. Example: REQ071 (above).

For a core that both engines check, keep **one sort**: a `Vec`-backed sort whose `#[cfg(kani)]`
`Arbitrary` builds from a fixed small array (e.g. `[_; 3].to_vec()`) lets Creusot prove ∀-length
while Kani checks the bounded case — no conflict.

## The mirror mechanism (how a predicate reaches Creusot)

A grounded predicate `p` is proved by Creusot through a pearlite mirror `p_logic`, which
`creusot::with_mirrors` redirects the harness onto **whenever the source contains `fn p_logic`**.
So the deductive proof is: hand-write `#[cfg(creusot)] #[logic(open)] pub fn p_logic(...) { pearlite! { ... } }`
plus an `#[ensures(result == <core>_logic(args))]` link on the real decision, then run `verify` — no
`--draft-semantic` / LLM needed (that is only the assisted way to _generate_ mirrors).

- The executable twin Kani runs (the one built from iterator `.any()/.all()`) must be
  `#[cfg_attr(creusot, trusted)]` — Creusot verifies every function in the crate and chokes on those
  contractless iterators; its proof runs on the pearlite mirror anyway.
- A sub-function the proof branches on needs its **own** `#[ensures(... == <sub>_logic(..))]`, or its
  branch is opaque to the invariant.

## Engine gotchas (hard-won)

### Kani

- Set a loop bound: `kani.default_unwind` in `provreq.yml` (provreq uses 4). A proof predicate that
  iterates over more elements than the bound trips `unwinding assertion loop 0` — count with
  arithmetic, not an iterator, if you can.
- Without an unwind **and** a `timeout_seconds`, a blown-up claim runs for hours instead of being cut
  off. Set both.

### pearlite (Creusot logic)

- `==` is **logical** equality — it works on any type with **no `PartialEq` derive** (the derive is
  the `DeepModel` wall). Use it freely on enums, `Option`, tuples.
- It **rejects `matches!`** (use `match`), **cannot match integer literals** (write
  `if n == 0usize { .. } else ..`, suffixed), and does not take tuple-with-or patterns
  (`(a, b)` against `A | B`) — nest `match`es instead. A bare `_` wildcard arm is fine.
- Import `use creusot_std::macros::*;` and, when needed, `use creusot_std::logic::{Int, Seq};` —
  **not** `creusot_std::prelude::*`, whose glob re-exports a `Clone` derive macro that collides with
  std's (`E0659`).
- Quantifiers: `exists<j: Int> 0 <= j && j < s.len() && P`, and
  `forall<j: Int> (0 <= j && j < s.len() && P) ==> Q` (`==>` binds loosest). Index a `Seq` with
  `(s[j]).0`; take a slice's `Seq` view with `xs@`.

### Creusot toolchain

- On arm64, prune provers whose binary does not resolve (archived cvc4 has no arm64 build) or
  `why3find prove` dies at calibration and **every** Creusot verdict sinks to inconclusive. provreq
  does this in its devcontainer `Dockerfile` after `why3-conf`.
- why3/alt-ergo **flake** under CPU contention (`can not calibrate prover …`, `why3find prove
failed`). This is not a proof failure if the harness compiled and translated — **re-run a Creusot
  red before believing it**.

### Provenance

- Set `USER=<reviewer>` before `draft --admit`, or provenance records `reviewer: unknown`.

## The mechanical workflow, condensed

Hand-written cores need no LLM (`$0`):

```text
draft <ID> --set '<PRL>'      # PRL: category: 1; vocabulary { state p(x: Sort) }; require { always (p(x)) }
draft <ID> --check            # the mechanical gate
draft <ID> --ground p=<fn>    # one binding per call; also ground Sort=<Type>
draft <ID> --ground Sort=<Type>
draft <ID> --dry-run          # expect: GROUNDED
USER=<you> draft <ID> --admit --yes
draft <ID> --writeback        # stamps provenance onto the requirement item — commit it
verify <ID>                   # runs the ensemble; persists verdicts.yml — commit it
```

A verify of a cat-1 self-proof runs several minutes (Kani compile+solve, Creusot translate+discharge,
plus the tagged tests). Keep the whole set honest on every change with the CI **verdict gate**
(`provreq gate --path . --base origin/main`), which re-verifies the committed baseline and blocks a
regression.

## ⚠️ Never gitignore `drafts.yml`

Admitted drafts are **durable state**, not a cache. qrusty gitignored its `drafts.yml`, a rebuild
wiped it, and ~530 PRLs were lost (provreq's #477-482). Commit `drafts.yml`, `verdicts.yml`, and the
`provreq-formalized` / `provreq-verdict` breadcrumbs the writeback appends to each requirement item.
provreq warns on this via `draft::drafts_at_loss_risk`.

## Related

- [end-to-end-guide.md](end-to-end-guide.md) — the CLI pipeline this cookbook plugs into.
- [requirement-language.md](requirement-language.md) — PRL design and the category/verdict model.
- [dogfood-self-adoption.md](dogfood-self-adoption.md) — the chronological record these recipes were
  distilled from.
- [qrusty-as-a-subject.md](qrusty-as-a-subject.md) — an existing tree's survey and capability gap.
