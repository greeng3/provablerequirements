//! Payload-free proof cores for provreq's own category-1 self-requirements.
//!
//! Creusot compiles the whole crate before running any harness and intercepts
//! `#[derive(PartialEq/Eq)]` to demand `DeepModel` on reachable types — a wall the app crate's ~96
//! derives turn into an open-ended arc (#531). So each self-requirement's decision lives here as a
//! payload-free core the app delegates to, and `verify.crates` points the requirement at this crate
//! (#483); because the crate is excluded from the root workspace, `cargo creusot` compiles only it
//! plus creusot-std.
//!
//! One module per requirement. Each holds: payload-free proxy type(s) that deliberately derive no
//! `PartialEq` (that derive is what triggers the `DeepModel` demand — the proofs use `match`, never
//! `==`, on these types), the pure decision the app delegates to, a proof predicate over a bounded
//! case, a `#[cfg(kani)]` `Arbitrary`, and `#[cfg(creusot)]` `#[logic]` pearlite mirrors that
//! `creusot::with_mirrors` redirects the harness onto.

pub mod fragment;
pub mod model_binding;
pub mod readiness;
pub mod triage;
