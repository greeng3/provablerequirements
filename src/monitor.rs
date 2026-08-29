//! What a runtime monitor reads: the trace the operator declares, and what is actually in it.
//!
//! Categories 1 and 2a bind to artifacts that sit still in the repository — a function, a TLA+
//! spec. A runtime monitor binds to a **trace**, which exists only because something ran. So the
//! operator supplies it (#230): provreq reads a log the subject already produces and never runs the
//! subject. Running it is category 3's problem, not this one.
//!
//! That makes the trace a declaration in the companion `provreq.yml`, the same move
//! [`crate::spec_paths`] and [`crate::tlc::Constants`] already make:
//!
//! ```yaml
//! monitor:
//!   trace: logs/events.jsonl
//!   format: jsonl
//!   time_field: ts
//!   events:
//!     accepted:  { name: msg_accepted, args: [id] }
//!     succeeded: { name: msg_done,     args: [id] }
//! ```
//!
//! Two things this module refuses to do, both the same refusal from #229's angle. A monitor's
//! evidence is [`crate::verdict::Basis::NotFalsified`] — *no violation appeared in what ran* — and
//! that claim is worth nothing without the trace behind it. So:
//!
//! - a **missing** trace is an error naming the path, never an empty trace monitored to a clean
//!   result;
//! - an **empty** trace is an error too. Zero records cannot falsify anything, so a monitor over
//!   one would report `not-falsified` having observed nothing at all.
//!
//! Implements: #230 (the operator declares the trace a monitor reads).

mod binding;
mod declaration;
mod mfotl;
mod run;
mod trace;

pub use binding::{RuntimeResolution, resolve};
pub use declaration::{Event, Monitor, TraceFormat};
pub use mfotl::{MonitorClaim, lower};
pub use run::{ENGINE, Outcome, monpoly_bin, run};
pub use trace::{Extent, current_fingerprint, occurrences};
