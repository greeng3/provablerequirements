//! What a category-3 UI check drives: the deployment the operator declares, and the steps a
//! browser will take against it.
//!
//! (This is the *subject's* UI — the thing being checked. provreq's own web surface lives in
//! `web/`, outside `src/` entirely.)
//!
//! Every earlier category binds to something that holds still long enough to be hashed. Category 1
//! binds to a function, 2a to a TLA+ spec, 2b to a **trace** — a file that exists only because
//! something ran, which is why #230 settled that the operator supplies it and provreq never runs
//! the subject. Category 3 goes one step further out: it binds to a **deployment that is currently
//! up**.
//!
//! The same rule carries over, and it is what keeps this module small. **The operator declares the
//! deployment; provreq never starts it, health-waits on it, or tears it down.** No lifecycle code,
//! and the seam is the one category 2b already proved out:
//!
//! ```yaml
//! ui:
//!   endpoint: http://localhost:4444      # the WebDriver grid (optional — see below)
//!   base_url: http://localhost:8080      # the deployment under check
//!   steps:
//!     open_cart:  { goto: /cart }
//!     checkout:   { click: "button[data-test=checkout]" }
//!     sees_total: { text_present: "Order total" }
//! ```
//!
//! Two asymmetries are deliberate, and both are about *whose* fact a field is.
//!
//! - `base_url` and `steps` describe the **subject**: what is under check and what is done to it.
//!   They are required, they live in the committed manifest, and they are what the drift axis
//!   fingerprints.
//! - `endpoint` describes the **environment provreq is running in** — which grid happens to be
//!   reachable from this machine. It is optional here and overridable by `WEBDRIVER_URL`, because
//!   an address that differs per operator does not belong welded into a committed file. #225 is
//!   open against exactly that mistake for the LLM endpoint; there is no reason to make it twice.
//!
//! Whatever evidence this eventually produces is [`crate::verdict::Basis::NotFalsified`] and
//! nothing stronger (#229): a UI driver observes one run of one deployment, so it can establish
//! that a requirement **fails** — with the most legible counterexample in the tool — and can never
//! establish that one holds.
//!
//! Implements: #239 (the operator declares the deployment a UI check drives).

mod binding;
mod declaration;
mod script;

pub use binding::{resolve, UiResolution};
pub use declaration::{current_fingerprint, Step, Ui};
pub use script::{lower, UiClaim};
