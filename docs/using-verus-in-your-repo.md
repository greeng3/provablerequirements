# Using Verus to prove requirements in a repo you own

Audience: a developer **or** an LLM coding assistant adding deductive proofs to
a Rust crate the reader controls. The goal is a concrete, do-it-now guide: by
the end you can install Verus, write a specification for existing code, run the
verifier, and read its answer. It closes with how this feeds provreq's
category-1 `proven` verdicts.

Scope and honesty notes:

- This is for **repos you own**. Verus requires authoring code in its dialect
  (or annotating it in place), which means editing the subject — fine for your
  own crate, not for a read-only dependency. See
  [`verus-engine-evaluation.md`](verus-engine-evaluation.md) for why the
  not-owned case stays a ceiling.
- Verus moves quickly and some official guide pages were missing when this was
  written. Where a mechanism is documented it is cited; where a detail is
  version-sensitive (exact toolchain string, whether a `cargo-verus`
  subcommand exists in your build) this doc says "verify against your release"
  rather than guess. **Always trust the output of `verus --version` and your
  release's `INSTALL.md` over this doc** if they disagree.
- A specification you never run proves nothing. Every example here is meant to
  be fed to the `verus` binary; the whole point of the tool is that it checks
  you. Run it.

---

## 1. What you get, and what it costs

Verus is an SMT-based (Z3) deductive verifier for a subset of Rust. You write
pre/postconditions, loop invariants, and assertions; Verus proves they hold for
**every** execution (relative to the spec), not just the ones a test exercises.
That is the `proven` rung — strictly stronger than bounded model checking
(Kani) or testing.

The cost is real and worth stating up front:

- Code that Verus reasons about must be in its **supported subset** and either
  inside a `verus! { }` block or annotated with its macros.
- You pin a specific Rust toolchain (Verus tells you which) and a specific Z3.
- Proof authoring is a skill: triggers, invariants, and non-linear arithmetic
  need care. Small functional cores are cheap; large imperative code is not.

The sweet spot is a **small, self-contained predicate or decision core** — which
is exactly the shape of a category-1 requirement.

---

## 2. Install

### 2.1 Prerequisites

- `rustup` installed and on `PATH`. Verus does **not** bundle a Rust toolchain;
  it requires a specific rustup toolchain and will tell you which one.
- A supported platform for the prebuilt binary (as of writing: recent macOS
  arm64, macOS x86_64, Windows x86_64, Ubuntu 24.04 x86_64). Other platforms
  need a source build.

### 2.2 Install the prebuilt binary (recommended)

1. Go to <https://github.com/verus-lang/verus/releases/latest> and download the
   asset for your platform (weekly point releases are recommended over the
   rolling pre-release). The asset name encodes a date and commit, e.g.
   `verus-0.YYYY.MM.DD.<hash>-x86-linux.zip`.
2. Unzip and enter the directory:

    ```bash
    unzip verus-0.YYYY.MM.DD.<hash>-x86-linux.zip
    cd verus-x86-linux   # directory name matches the asset
    ```

3. On macOS only, clear the Gatekeeper quarantine (inspect the script first if
   you like):

    ```bash
    bash macos_allow_gatekeeper.sh
    ```

4. Run `./verus` once. If the required toolchain is missing it prints the exact
   `rustup` command to run, for example:

    ```text
    verus: required rust toolchain 1.XX.0-x86_64-unknown-linux-gnu not found
    run the following command (in a bash-compatible shell) to install the necessary toolchain:
      rustup install 1.XX.0-x86_64-unknown-linux-gnu
    ```

    Run what it prints, then re-run `./verus`. Repeat until it stops asking.
    (The exact version string comes from your release — do not hard-code the one
    above.)

The prebuilt binary ships its Z3; you do not install Z3 separately for this
path.

### 2.3 Build from source (only if unsupported platform)

Requires `rustup` and `cargo install cargo-make`, then from the repo's
`source/` directory:

```bash
cd source
./tools/get-z3.sh        # Windows: ./tools/get-z3.ps1  (needs Z3 4.16.0)
rustup toolchain install # installs the toolchain pinned by rust-toolchain.toml
cargo make build
```

The `get-z3` script sets `VERUS_Z3_PATH`. The built binary lands at
`source/target-verus/release/verus`.

### 2.4 Confirm it works

Save this as `hello_verus.rs`:

```rust
use vstd::prelude::*;

verus! {

spec fn min(x: int, y: int) -> int {
    if x <= y { x } else { y }
}

fn main() {
    assert(min(10, 20) == 10);
}

} // verus!
```

Run it:

```bash
./verus hello_verus.rs
```

Expected:

```text
note: verifying root module
verification results:: 1 verified, 0 errors
```

If you instead change the assertion to `assert(min(10, 20) == 20);` you get
`error: assertion failed` and `verification results:: 0 verified, 1 errors`.
That failure is the tool doing its job.

Note: when you invoke the `verus` binary on a file, it supplies `vstd` and the
builtin crate itself — that is why `use vstd::prelude::*;` resolves without a
Cargo dependency. This "point Verus at an entry file" model is the reliable
workflow; see §6 for whole-crate use.

---

## 3. The dialect in one page

Everything below lives inside `verus! { ... }` unless noted. All of these are
from the official guide and verify as shown (or as cited).

### 3.1 Three function modes

```rust
spec fn f1(x: int) -> int { x / 2 }   // ghost: specification only, erased at compile time
proof fn f2(x: int) -> int { x / 2 }  // ghost: carries proofs, erased at compile time
fn f3(x: u64) -> u64 { x / 2 }        // exec: real runnable code ("exec" keyword optional)
```

Call rules: `spec` may call only `spec`; `proof` may call `proof` and `spec`;
`exec` may call `exec`, `spec`, and (inside a `proof { }` block) `proof`.

### 3.2 `requires` / `ensures`

Preconditions and postconditions go in the signature; name the return value to
refer to it in `ensures`:

```rust
fn octuple(x1: i8) -> (x8: i8)
    requires -16 <= x1 < 16,
    ensures x8 == 8 * x1,
{
    let x2 = x1 + x1;
    let x4 = x2 + x2;
    x4 + x4
}
```

A caller must satisfy the precondition — `octuple(20)` fails to verify with
"precondition not satisfied". Callers may then rely on the postcondition.

### 3.3 Spec integers

Spec code may use the unbounded ghost types `int` and `nat`; exec code uses
real types like `u64`. Cast at the boundary with `as int`:

```rust
fn compute_min3(x: u64, y: u64, z: u64) -> (m: u64)
    ensures m == min3(x as int, y as int, z as int),
{ /* ... */ }
```

### 3.4 Loops: `invariant` and `decreases`

A `while` loop needs an invariant strong enough to re-establish itself and to
imply the postcondition; `decreases` proves termination:

```rust
while idx < n
    invariant
        idx <= n,
        sum == triangle(idx as nat),
    decreases n - idx,
{
    idx = idx + 1;
    sum = sum + idx;
}
```

The invariant must hold on entry and be preserved by every iteration. A common
mistake is `invariant idx < n` — false after the last iteration, where
`idx == n`.

### 3.5 Quantifiers and triggers

```rust
proof fn test_use_forall(s: Seq<int>)
    requires
        5 <= s.len(),
        forall|i: int| 0 <= i < s.len() ==> #[trigger] is_even(s[i]),
{
    assert(is_even(s[3]));
}
```

Use `==>` for implication. The SMT solver instantiates a `forall` only at
**triggers** — expressions you (or `#![auto]`) mark with `#[trigger]`. A trigger
must be a function call, field access, or similar; it cannot be a bare `==`,
`+`, `<=`, or `&&`. Bad triggers are the most common reason a "obviously true"
`forall` fails to apply — if a proof stalls, suspect the trigger first.

### 3.6 Assertions and proof blocks

```rust
assert(x > 0);                       // prove this here
assert(hard_fact) by {               // prove hard_fact using a local sub-proof
    some_lemma(a, b);
}
proof {                              // drop into proof mode inside exec code
    my_proof_fn(a);
}
```

`assume(p)` asserts `p` **without proof** — a trust hole. `assume(false)` proves
anything. Use it only as a temporary scaffold, never in code you call verified.

### 3.7 Spec visibility

A `spec fn` body is visible within its module by default. A `pub spec fn` must
declare `open` (body visible to other modules) or `closed` (body hidden):

```rust
pub open spec fn is_sorted(s: Seq<int>) -> bool { /* visible everywhere */ }
pub closed spec fn secret(x: int) -> bool { /* opaque to other modules */ }
```

`#[verifier::opaque]` hides a body even within its module until a `reveal(name)`
call — a performance/automation lever, not a visibility one.

---

## 4. Four ways to put specs on code you own

Ordered least-to-most intrusive. For a provreq category-1 requirement
(an invariant over a boolean predicate), **Strategy A** is the one that yields a
sound `proven`; the others are for surrounding code.

### Strategy A — model the predicate as a `spec fn` (the sound retrofit)

This is the money move and the exact analogue of Prusti's `#[pure]` predicate.
Your requirement names a predicate (say "a user with a session is logged in").
You write that predicate once as a `spec fn`, and prove the invariant against
it. Because the `spec fn` **is** the meaning, the proof is grounded in real
logic, not an assumed contract.

```rust
use vstd::prelude::*;

verus! {

pub struct User { pub logged_in: bool, pub has_session: bool }

// The requirement's predicates, as spec functions.
pub open spec fn logged_in(u: User) -> bool { u.logged_in }
pub open spec fn has_session(u: User) -> bool { u.has_session }

// The invariant: no user has a session without being logged in.
// Proved for ALL users of this shape.
proof fn invariant_holds(u: User)
    requires u.has_session ==> u.logged_in,   // the enforced constraint
    ensures has_session(u) ==> logged_in(u),
{
}

} // verus!
```

Run `verus` on the file; a clean run means the implication holds for every
`User`. If the `requires` line is removed, Verus reports the obligation might
not hold — correctly, because nothing then forces the invariant.

To prove the invariant about the **executable** value a function returns, give
that function an `ensures` tying its result to the spec predicate (§3.3's
`compute_min3` shape), then the invariant rides on the postcondition.

### Strategy B — annotate an existing `exec fn` in place, no API change

If you want to keep an ordinary function exactly where it is and not wrap it in
`verus! { }`, use the attribute macros, which attach a contract without touching
the signature (documented on the guide's `exec_attr` page):

```rust
#[verus_spec(sum =>
    requires x < 100, y < 100,
    ensures sum < 200,
)]
fn add_bounded(x: u32, y: u32) -> u32 {
    x + y
}
```

Rules: `#[verus_verify]` must precede `#[verus_spec]` if both are present, and
at most one `#[verus_spec]` per item. The guide recommends `#[verus_spec]` for
exec functions and `verus! { }` for `spec`/`proof` functions. **Verify the exact
macro names against your release** — these are newer and less stable than the
`verus! { }` core.

### Strategy C — `external_body`: trust the body, prove the contract

For a function whose body is outside the verifiable subset (unsafe, FFI, complex
iterators) but whose **contract** you want callers to rely on, verify the
signature and treat the body as a black box:

```rust
verus! {

#[verifier::external_body]
fn clamp_add(x: u32, y: u32) -> (r: u32)
    requires x < 100, y < 100,
    ensures r < 200,
{
    x + y   // body NOT checked against the ensures
}

} // verus!
```

This is a **trust boundary**: Verus believes the `ensures` without proof, so a
wrong contract here silently corrupts every proof downstream. Keep these few,
small, and reviewed.

### Strategy D — `assume_specification`: attach a spec to code you can't edit

For a function from `std` or a dependency you cannot annotate, declare an
assumed spec for it from your crate (documented on the guide's
`reference-assume-specification` page):

```rust
verus! {

pub assume_specification [<bool as Clone>::clone](b: &bool) -> (res: bool)
    ensures res == b;

} // verus!
```

The signature must match exactly (generics, trait bounds, all args named, no
`self`). Like Strategy C this is **unchecked** — Verus assumes it without
proof. For your own code prefer A/B/C, which are checked; reserve `assume_
specification` for third-party boundaries, and treat each one as an audited
assumption.

Note on plain Rust: any item **outside** `verus! { }` is automatically external
(ignored by Verus) — no attribute needed. You mix verified and ordinary Rust
freely in one crate; put the proofs where they earn their keep.

---

## 5. End-to-end worked example

A complete, runnable file that takes a requirement — _"a withdrawal never
overdraws an account"_ — and proves it. This is the shape an LLM assistant
should produce when asked to "prove REQxyz with Verus."

```rust
use vstd::prelude::*;

verus! {

pub struct Account { pub balance: u64 }

// The predicate the requirement is about, as a spec fn. Both operands are u64,
// so the comparison is well-typed in spec mode with no `int` cast needed.
pub open spec fn sufficient_funds(a: Account, amount: u64) -> bool {
    amount <= a.balance
}

// Executable withdrawal, with a contract. Precondition: the predicate holds.
// Postcondition: the new balance is exactly old minus amount — and because the
// precondition held, the u64 subtraction provably cannot underflow.
fn withdraw(a: Account, amount: u64) -> (res: Account)
    requires sufficient_funds(a, amount),
    ensures res.balance == a.balance - amount,
{
    Account { balance: a.balance - amount }
}

fn main() {
    let a = Account { balance: 100 };
    let b = withdraw(a, 30);
    assert(b.balance == 70);
}

} // verus!
```

Run:

```bash
./verus account.rs
```

Expected: `verification results:: N verified, 0 errors`.

Now make it fail on purpose to see the proof bite: change the precondition to
`requires true,` and Verus reports the subtraction `a.balance - amount` might
underflow — because without `sufficient_funds(a, amount)` the result is not
provably a valid `u64`. The verifier caught a real bug the type system alone
would not.

For a domain where "overdrawn" means something richer than "insufficient funds"
(a signed balance, an overdraft limit), model that condition as its own
`spec fn` the same way and reference it in the contract; keep the predicate's
operands in matching types (cast to `int`/`nat` with `as int` only inside the
spec when you genuinely need unbounded arithmetic).

### The workflow, as steps

1. Name the requirement's predicate(s); write each as a `pub open spec fn`.
2. Give the exec function(s) a `requires`/`ensures` that references those
   predicates.
3. Add the invariant as an `ensures` (or a `proof fn` over all inputs, as in
   §4.A).
4. Run `verus`. Read `verified: N, errors: M`.
5. If `errors > 0`: read the error location. Usual causes — missing loop
   invariant, a `forall` with no matching trigger, an arithmetic fact the
   solver needs spelled out via `assert(... ) by { lemma(); }`, or a genuinely
   false claim.
6. Iterate until `0 errors`. Commit the spec **with** the code — it is now part
   of the contract.

---

## 6. Running Verus on a whole crate

The documented, reliable method is to point Verus at the crate's entry file:

```bash
./verus src/main.rs                     # binary crate
./verus src/lib.rs --crate-type=lib     # library crate
```

Verus follows the module tree from there. A `cargo-verus`/`cargo verus`
subcommand may exist in newer releases for dependency-aware builds, but it was
not covered by the guide pages this doc was built from — **check your release's
`INSTALL.md` / `--help`** before relying on it; the entry-file method always
works.

### In CI

Treat it like any other checker: fail the build on a nonzero exit or any
`errors:` count above zero.

```bash
# fails the job if Verus reports any verification error
./verus src/lib.rs --crate-type=lib \
  || { echo "Verus verification failed"; exit 1; }
```

Pin the Verus release (the dated asset) and the toolchain it names, the same way
the repo already pins Creusot/Kani — a floating Verus version is a soundness and
reproducibility risk, not a convenience.

---

## 7. How this feeds provreq category-1

provreq's category-1 engines lower a gated requirement to a per-engine proof
harness (`src/kani.rs`, `src/creusot.rs`, `src/prusti.rs`). A Verus engine would
reuse the shared lowering (`lowering::lower_property` yields an engine-agnostic
`LoweredClaim { claim, quantified }`) and wrap the same claim as
`assert(forall|b: ty| claim)` inside a generated `verus! { }` harness — the
Verus analogue of `prusti_assert!(forall(...))`.

The load-bearing precondition is **Strategy A**: the subject's predicates must
exist as `spec fn`s. With that in place:

- **Author-side (your one-time commit):** predicates authored as `spec fn`
  (§4.A). This is the `#[pure]`-equivalent obligation, on a repo you own.
- **Tool-side (provreq, per `verify`):** generate the ephemeral assert harness,
  run `verus`, map `0 errors` → `proven`, restore the subject. Additive — your
  code is not permanently rewritten.

Because the harness asserts over your **real** `spec fn`s (not an assumed
contract), a clean run earns a genuine `proven`, the strongest rung. A subject
whose predicates are _not_ authored as `spec fn` yields an honest ceiling, never
a guess — see [`verus-engine-evaluation.md`](verus-engine-evaluation.md).

See also [`formalizing-requirements.md`](formalizing-requirements.md) for the
payload-free / leaf-crate patterns that keep a proof core small enough for a
prover to discharge quickly.

---

## 8. Checklist for an LLM coding assistant

When asked to verify a requirement with Verus in an owned repo:

- [ ] Confirm `verus` is installed and `./verus --version` runs; if not, do §2.
- [ ] Identify the requirement's predicate(s) and the invariant/postcondition.
- [ ] Write predicates as `pub open spec fn` (Strategy A). Do **not** reach for
      `assume_specification`/`external_body` for code you can edit — those are
      unchecked and cannot honestly earn `proven`.
- [ ] Put `requires`/`ensures` on the relevant exec functions; reference the
      spec predicates.
- [ ] For loops, add `invariant` + `decreases`. For `forall`, add a valid
      `#[trigger]`.
- [ ] Run `verus` and read the count. Never report success without a
      `0 errors` run — a spec that was not executed is not evidence.
- [ ] On failure, fix the **proof or the code**, not by weakening to `assume`.
      If you must scaffold with `assume`, say so explicitly and leave it as a
      flagged TODO, never as the final state.
- [ ] Commit the spec alongside the code and pin the Verus release used.

---

## References (official)

- Guide: <https://verus-lang.github.io/verus/guide/>
- Getting started (command line):
  <https://verus-lang.github.io/verus/guide/getting_started_cmd_line.html>
- `requires`/`ensures`:
  <https://verus-lang.github.io/verus/guide/requires_ensures.html>
- Modes, spec/proof functions, loops, quantifiers, `assert ... by`, opaque:
  the corresponding chapters of the guide.
- Attribute reference:
  <https://verus-lang.github.io/verus/guide/reference-attributes.html>
- Interacting with unverified code (`#[verifier::external_body]`,
  `assume_specification`) — this chapter replaced the old `external.html`:
  <https://verus-lang.github.io/verus/guide/interacting-with-unverified-code.html>
- `assume_specification` reference:
  <https://verus-lang.github.io/verus/guide/reference-assume-specification.html>
- Annotating plain exec fns / generating a spec from an exec fn (`dual_spec`):
  <https://verus-lang.github.io/verus/guide/exec_to_spec.html>
- Install / build: `INSTALL.md` and `BUILD.md` in
  <https://github.com/verus-lang/verus>

Note: the guide is a living mdBook and chapters get renamed — `external.html`,
`assume_specification.html`, `exec_funs_vs_spec_funs.html`, and `basics.html`
all 404 as of this writing (their content moved into the chapters above). If a
link here 404s, start from the guide root and the source under
`source/docs/guide/src/` in the repo.
