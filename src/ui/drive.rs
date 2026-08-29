//! Driving the step script against a real browser, and reading what came back honestly (#245).
//!
//! # ⚠️ A step that fails to *run* is not a refutation
//!
//! This is the category-3 trap, and it is the mirror of the one [`crate::monitor::run`] exists to
//! survive. There, every answer arrived on exit code 0, so a refusal could pass for a clean run.
//! Here the danger runs the other way and is more tempting, because the wrong answer *looks like
//! the tool working*:
//!
//! | what happened | who decided it | verdict |
//! | --- | --- | --- |
//! | the grid would not seat a session | the environment | `inconclusive` |
//! | `goto` could not load the page | the environment | `inconclusive` |
//! | `click` matched no element | **the script** | `inconclusive` |
//! | the asserted text was absent | **the deployment** | `fails`, with a witness |
//!
//! A selector that has gone stale means the check never became a check. Reporting it as a failed
//! requirement would manufacture exactly the counterexample category 3 exists to produce — a red
//! result nobody can act on, because nothing about the requirement was ever tested. **Only the
//! observation step can refute**; #243 already refuses to lower a claim that asserts anything else.
//!
//! # What a `holds` from here is worth
//!
//! The weakest rung in the tool, and deliberately so: [`crate::verdict::Basis::NotFalsified`] over
//! **one execution of one deployment**. The inversion is worth stating plainly, because it is
//! backwards from every other engine here — a category-3 `fails` is the *strongest* evidence
//! provreq deals in (a real counterexample from a real system), and its `holds` is the weakest.
//! Both come out of the same run.
//!
//! # The settle window
//!
//! A deployment is not obliged to be finished rendering when the driver looks. So the observation
//! is **polled for presence** until it appears or the window closes — one loop that is correct in
//! both directions: an asserted text gets the window to show up, and a text asserted *absent* gets
//! the whole window to betray itself. Looking exactly once would turn a slow page into a fabricated
//! `fails`, and a slow page into a fabricated `holds`, depending only on which way the claim was
//! written.
//!
//! Implements: #245.

use super::declaration::{Step, Ui};
use super::script::UiClaim;
use crate::verdict::{Basis, Evidence};
use std::time::Duration;

/// The engine's name wherever it is reported. Selenium is what is really driven — reached as a
/// **grid service**, not a `PATH` binary — and the W3C protocol is what is really spoken, so the
/// name says both rather than the registry's old `Selenium/Playwright` slash.
pub const ENGINE: &str = "Selenium (WebDriver)";

/// How long the observation is watched for. A fixed window rather than a declared one: no subject
/// has asked for a different value yet, and an operator-tunable timeout is a knob that gets turned
/// until a flaky check goes green.
///
/// `// ponytail: fixed 5s window + 250ms poll. Move to `ui.settle_seconds` when a real subject
/// needs longer — the reach line already names the window, so a verdict says which one it used.`
const SETTLE: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(250);

/// How long any one WebDriver call may take. Session creation is the slow one (the grid may be
/// starting a container), so it is generous; a hung grid must still not hang `verify` forever.
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// What the run established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The script ran and the deployment answered as the claim says it would. The weakest rung
    /// there is — see the module docs.
    NotFalsified { browser: String },
    /// The script ran and the deployment answered the other way. `at` is the URL the browser was
    /// on when it looked, which is what makes the refutation replayable.
    Refuted { at: String, browser: String },
    /// The claim was never put to the deployment. Everything that is not the deployment's own
    /// answer lands here — see the table in the module docs.
    Inconclusive { reason: String },
}

impl Outcome {
    /// A `holds` from this engine can only ever be the empirical rung, asserted where the type
    /// system can see it so a later edit cannot quietly hand a browser a stronger basis.
    pub const BASIS: Basis = Basis::NotFalsified;

    /// Map what the browser established into [`Evidence`]. The mapping lives here, in the engine,
    /// so [`crate::verdict`] never learns what a browser is (D2).
    pub fn into_evidence(&self, ui: &Ui, claim: &UiClaim) -> Evidence {
        match self {
            Outcome::NotFalsified { browser } => {
                Evidence::not_falsified(ENGINE, reach(ui, claim, browser))
            }
            Outcome::Refuted { at, browser } => {
                let mut detail = vec![reach(ui, claim, browser)];
                detail.extend(claim.describe());
                Evidence::fails(ENGINE, Some(refutation(at, claim)), detail)
            }
            Outcome::Inconclusive { reason } => Evidence::inconclusive(
                ENGINE,
                vec![
                    reason.clone(),
                    format!(
                        "the deployment under check is {} — nothing about the requirement was \
                         established either way",
                        ui.base_url()
                    ),
                ],
            ),
        }
    }
}

/// What a `holds` from this run covers, in the operator's terms. The counterpart of 2b's
/// [`crate::monitor::Extent`]: an empirical rung that does not say what it saw overclaims by
/// omission (#229), and here what was seen is *one* run, of *one* deployment, in *one* browser.
fn reach(ui: &Ui, claim: &UiClaim, browser: &str) -> String {
    format!(
        "one execution of the deployment at {} in {browser}, watching {}s for the result — {}",
        ui.base_url(),
        SETTLE.as_secs(),
        claim.describe().join("; ")
    )
}

/// The refutation as one line the operator can act on: where the browser was, and what the page
/// did or did not show.
fn refutation(at: &str, claim: &UiClaim) -> String {
    let text = claim.expect.argument();
    if claim.expect_absent {
        format!("the page at {at} showed `{text}`, which this requirement says it never does")
    } else {
        format!("the page at {at} did not show `{text}`")
    }
}

/// Drive the claim's script against the declared deployment.
///
/// Runs on its own OS thread with its own single-threaded runtime, and that is load-bearing rather
/// than incidental: `verify` is synchronous and is called straight out of an async axum handler
/// (`POST /api/requirements/:id/verify`), where blocking on a future from the calling thread
/// panics. A fresh thread has no runtime context to conflict with, so the same code path serves the
/// CLI and the web surface — which is the whole point of `verify` being one flow.
pub fn run(ui: &Ui, claim: &UiClaim) -> Outcome {
    let Some(endpoint) = ui.endpoint() else {
        return Outcome::Inconclusive {
            reason: format!(
                "no WebDriver grid is configured, so there is nothing to drive the browser with. \
                 Set `{}` in the environment, or `ui.endpoint` in provreq.yml if every operator \
                 reaches the same grid. This is an address on this machine, not a fact about the \
                 subject — provreq will not guess one",
                super::declaration::ENDPOINT_VAR
            ),
        };
    };
    let ran = match in_runtime(drive(&endpoint, ui, claim)) {
        Ok(inner) => inner,
        Err(reason) => Err(reason),
    };
    match ran {
        Err(reason) => Outcome::Inconclusive { reason },
        // One condition, correct in both directions: the loop stops as soon as the text is
        // present, so presence refutes an `expect_absent` claim and satisfies the other.
        Ok(ran) if ran.present != claim.expect_absent => Outcome::NotFalsified {
            browser: ran.browser,
        },
        Ok(ran) => Outcome::Refuted {
            at: ran.at,
            browser: ran.browser,
        },
    }
}

/// What one execution came back with.
struct Ran {
    present: bool,
    at: String,
    browser: String,
}

/// Block on a future from synchronous code that may itself be inside a runtime. See [`run`].
fn in_runtime<F>(fut: F) -> Result<F::Output, String>
where
    F: std::future::Future + Send,
    F::Output: Send,
{
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("could not start a runtime to reach the grid: {e}"))?;
                Ok(rt.block_on(fut))
            })
            .join()
            .map_err(|_| "the WebDriver session panicked".to_string())?
    })
}

async fn drive(endpoint: &str, ui: &Ui, claim: &UiClaim) -> Result<Ran, String> {
    let session = Session::open(endpoint).await?;
    // The session is deleted whatever happened — a leaked session holds a grid slot until it times
    // out, so the next run would queue behind this one's mistake.
    let ran = session.script(ui, claim).await;
    session.close().await;
    ran
}

struct Session {
    http: reqwest::Client,
    endpoint: String,
    id: String,
    browser: String,
}

impl Session {
    async fn open(endpoint: &str) -> Result<Session, String> {
        let http = reqwest::Client::builder()
            .timeout(CALL_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| format!("could not build an HTTP client for the grid: {e}"))?;
        let endpoint = endpoint.trim_end_matches('/').to_string();
        // Ask the grid what it has, and then ask for that.
        //
        // ⚠️ Measured, and the reason this is not one call: an **empty** `alwaysMatch` — "provreq
        // has no opinion, give me anything" — does not match a slot. The grid does not refuse it;
        // it *queues the request until the session timeout*, so a perfectly healthy grid answers
        // nothing for a minute and the driver reports "could not be reached". A wrong reason for a
        // working system is worse than a slow one. The grid's own stereotype is the honest source
        // for which browser to ask for: it is the operator's statement about what they run.
        let browser = first_offered_browser(&http, &endpoint).await?;
        let value = call(
            http.post(format!("{endpoint}/session"))
                .json(&serde_json::json!({
                    "capabilities": { "alwaysMatch": { "browserName": browser } }
                })),
            "opening a browser session",
        )
        .await?;
        let id = value
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "the grid at {endpoint} answered without a session id, so no browser was \
                         seated"
                )
            })?
            .to_string();
        let browser = describe_browser(value.get("capabilities"));
        let session = Session {
            http,
            endpoint,
            id,
            browser,
        };
        // Element lookups retry for the settle window instead of failing the instant a page is
        // still rendering. Without this a `click` on a slow page is reported as a stale selector.
        let _ = call(
            session
                .http
                .post(session.url("/timeouts"))
                .json(&serde_json::json!({ "implicit": SETTLE.as_millis() as u64 })),
            "setting the element-lookup timeout",
        )
        .await;
        Ok(session)
    }

    fn url(&self, suffix: &str) -> String {
        format!("{}/session/{}{}", self.endpoint, self.id, suffix)
    }

    /// Perform the claim's actions, then watch for its observation.
    async fn script(&self, ui: &Ui, claim: &UiClaim) -> Result<Ran, String> {
        for step in &claim.steps {
            match step {
                Step::Goto(path) => self.goto(&resolve_url(ui.base_url(), path)).await?,
                Step::Click(css) => self.click(css).await?,
                // Unreachable through `lower`, which refuses an observation used as an action
                // (#243). Reported rather than asserted: this module does not get to assume
                // another one held.
                Step::TextPresent(t) => {
                    return Err(format!(
                        "the script was asked to perform `text_present: \"{t}\"`, which is an \
                         observation — a driver reads it rather than does it"
                    ));
                }
            }
        }
        let Step::TextPresent(text) = &claim.expect else {
            return Err(format!(
                "the claim asserts `{}`, which is an action the driver performs itself — nothing \
                 the deployment decides",
                claim.expect.key()
            ));
        };
        let present = self.watch_for(text).await?;
        Ok(Ran {
            present,
            at: self
                .current_url()
                .await
                .unwrap_or_else(|_| "the page".into()),
            browser: self.browser.clone(),
        })
    }

    /// Poll the page for `text` until it appears or the settle window closes. Presence is the
    /// early exit in both directions — see the module docs.
    async fn watch_for(&self, text: &str) -> Result<bool, String> {
        let deadline = std::time::Instant::now() + SETTLE;
        loop {
            if self.body_text().await?.contains(text) {
                return Ok(true);
            }
            if std::time::Instant::now() >= deadline {
                return Ok(false);
            }
            tokio::time::sleep(POLL).await;
        }
    }

    async fn goto(&self, url: &str) -> Result<(), String> {
        call(
            self.http
                .post(self.url("/url"))
                .json(&serde_json::json!({ "url": url })),
            &format!("navigating to {url}"),
        )
        .await
        .map(|_| ())
    }

    async fn click(&self, css: &str) -> Result<(), String> {
        let element = self.find(css).await?;
        call(
            self.http
                .post(self.url(&format!("/element/{element}/click")))
                .json(&serde_json::json!({})),
            &format!("clicking `{css}`"),
        )
        .await
        .map(|_| ())
    }

    async fn find(&self, css: &str) -> Result<String, String> {
        let value = call(
            self.http
                .post(self.url("/element"))
                .json(&serde_json::json!({ "using": "css selector", "value": css })),
            &format!("looking for `{css}`"),
        )
        .await?;
        // A found element arrives as a one-key object whose key is the W3C's own constant
        // (`element-6066-11e4-a52e-4f735466cecf`). Read as *the* value rather than by that key on
        // purpose: a UUID typed from memory is a bug no unit test can see — this one was typed
        // wrong, and only the live grid found it. The protocol guarantees the single entry.
        value
            .as_object()
            .and_then(|found| found.values().next())
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("`{css}` matched nothing on the page"))
    }

    /// The page's **visible** text, which is what "the page shows this" has to mean. Reading the
    /// HTML source instead would match text inside a script tag or an attribute — a check that
    /// passes on a page showing the operator nothing.
    async fn body_text(&self) -> Result<String, String> {
        let body = self.find("body").await?;
        let value = call(
            self.http.get(self.url(&format!("/element/{body}/text"))),
            "reading the page text",
        )
        .await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    async fn current_url(&self) -> Result<String, String> {
        let value = call(self.http.get(self.url("/url")), "reading the current URL").await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    async fn close(self) {
        let _ = call(
            self.http.delete(self.url("")),
            "closing the browser session",
        )
        .await;
    }
}

/// One WebDriver call, unwrapped to its `value`. A non-2xx answer carries the protocol's own
/// message, which is far more useful than the status alone (`no such element` names the mistake).
async fn call(req: reqwest::RequestBuilder, what: &str) -> Result<serde_json::Value, String> {
    let response = req
        .send()
        .await
        .map_err(|e| format!("{what}: the grid could not be reached ({e})"))?;
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or(serde_json::Value::Null);
    if status.is_success() {
        return Ok(body
            .get("value")
            .cloned()
            .unwrap_or(serde_json::Value::Null));
    }
    let message = body
        .pointer("/value/message")
        .and_then(|m| m.as_str())
        .unwrap_or("no message");
    Err(format!(
        "{what}: WebDriver answered {status} — {}",
        brief(message)
    ))
}

/// A WebDriver error message's first line, capped. Selenium appends build info and a stack trace
/// to everything; the first line is the part that names what went wrong.
fn brief(message: &str) -> String {
    const MAX: usize = 200;
    let line = message
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no message");
    match line.char_indices().nth(MAX) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_string(),
    }
}

/// The browser that answered, for the verdict's reach line. `unknown` rather than invented when the
/// grid does not say.
fn describe_browser(capabilities: Option<&serde_json::Value>) -> String {
    let field = |name: &str| {
        capabilities
            .and_then(|c| c.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    match (field("browserName"), field("browserVersion")) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        (Some(name), None) => name,
        _ => "an unnamed browser".to_string(),
    }
}

/// A declared `goto` path against the deployment's base URL.
///
/// An absolute URL is taken as written — a step that names another host is the operator saying so,
/// and rewriting it would drive somewhere they did not ask for.
fn resolve_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Probe the grid the operator's environment points at (REQ024: a probe exists only for an engine
/// provreq can really run).
///
/// This is why [`crate::engine::EngineProbe`] could not be reused, and the difference is a gain
/// rather than a workaround: a `PATH` probe answers "a file with this name exists", while
/// `GET /status` answers "**this grid can seat a session right now**", which is the thing that has
/// to be true for a verdict to be obtainable.
///
/// - no endpoint at all, or nothing answering → [`crate::engine::EngineStatus::Missing`]: an
///   address is a fact about this machine, and it is the operator's environment to fix.
/// - answering but `ready: false` → `Unusable`, carrying the grid's own words. A grid with no free
///   node is present and cannot start, which is exactly REQ051's distinction.
pub fn detect_grid(companion_root: Option<&std::path::Path>) -> crate::engine::EngineStatus {
    use crate::engine::EngineStatus;
    let Some(endpoint) = super::declaration::endpoint(companion_root) else {
        return EngineStatus::Missing;
    };
    match in_runtime(grid_status(&endpoint)).and_then(|inner| inner) {
        Ok(value) => grid_state(&value),
        Err(_) => EngineStatus::Missing,
    }
}

/// What a grid's own `/status` says it is. Pure over the answer, so the reading is tested without
/// a grid — the same split that keeps [`crate::monitor::Outcome::read`] testable without MonPoly.
fn grid_state(status: &serde_json::Value) -> crate::engine::EngineStatus {
    use crate::engine::EngineStatus;
    let ready = status
        .get("ready")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !ready {
        return EngineStatus::Unusable {
            reason: brief(
                status
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("the grid is not ready to seat a session"),
            ),
        };
    }
    EngineStatus::Available {
        version: grid_version(status),
    }
}

/// The browser the grid's first slot offers — what a session request must name. An error rather
/// than a guess when the grid seats nothing: `chrome` assumed against a Firefox-only grid would
/// queue exactly as an empty capability set does, and report the same wrong reason.
async fn first_offered_browser(http: &reqwest::Client, endpoint: &str) -> Result<String, String> {
    let status = call(
        http.get(format!("{endpoint}/status")),
        "asking the grid what it can seat",
    )
    .await?;
    offered_browsers(&status).into_iter().next().ok_or_else(|| {
        format!(
            "the grid at {endpoint} is answering but offers no browser to seat a session in — it \
             has no nodes registered, so there is nothing to drive the check with"
        )
    })
}

async fn grid_status(endpoint: &str) -> Result<serde_json::Value, String> {
    let http = reqwest::Client::builder()
        .timeout(CONNECT_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|e| format!("could not build an HTTP client for the grid: {e}"))?;
    call(
        http.get(format!("{}/status", endpoint.trim_end_matches('/'))),
        "asking the grid whether it is ready",
    )
    .await
}

/// What a ready grid *is*, as a version string: the browsers it can seat. More useful than the
/// grid's own build number, which says nothing about whether a check can run.
fn grid_version(status: &serde_json::Value) -> String {
    let mut seen: Vec<String> = Vec::new();
    for stereotype in stereotypes(status) {
        let name = describe_browser(Some(stereotype));
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    if seen.is_empty() {
        return "unknown".to_string();
    }
    format!("grid seating {}", seen.join(", "))
}

/// The browser names a grid's slots offer, in the grid's own order and without repeats.
fn offered_browsers(status: &serde_json::Value) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for stereotype in stereotypes(status) {
        if let Some(name) = stereotype.get("browserName").and_then(|n| n.as_str())
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_string());
        }
    }
    names
}

/// Every slot stereotype a `/status` answer declares — one place that knows the shape, since both
/// the probe and the session request read it.
fn stereotypes(status: &serde_json::Value) -> Vec<&serde_json::Value> {
    status
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|node| node.get("slots").and_then(|s| s.as_array()))
        .flatten()
        .filter_map(|slot| slot.get("stereotype"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::Status;
    use std::collections::BTreeMap;

    fn ui() -> Ui {
        Ui::new(
            "http://deployment.test:8080",
            Some("http://grid.test:4444".to_string()),
            BTreeMap::from([
                ("open_cart".to_string(), Step::Goto("/cart".into())),
                (
                    "checkout".to_string(),
                    Step::Click("button[data-test=checkout]".into()),
                ),
                (
                    "sees_total".to_string(),
                    Step::TextPresent("Order total".into()),
                ),
            ]),
        )
    }

    fn claim() -> UiClaim {
        UiClaim {
            steps: vec![Step::Click("button[data-test=checkout]".into())],
            expect: Step::TextPresent("Order total".into()),
            expect_absent: false,
        }
    }

    // Verifies: #245 — a `holds` from a browser is the EMPIRICAL rung and nothing stronger, and it
    // carries what it actually saw: one run, one deployment, one browser, one window. An empirical
    // claim with no reach attached overclaims by omission (#229).
    #[test]
    fn a_clean_run_earns_not_falsified_and_says_what_it_saw() {
        let e = Outcome::NotFalsified {
            browser: "chrome 124.0".into(),
        }
        .into_evidence(&ui(), &claim());

        assert_eq!(e.status, Status::Holds);
        assert_eq!(e.basis, Some(Basis::NotFalsified));
        assert_eq!(e.basis, Some(Outcome::BASIS));
        assert!(e.detail[0].contains("one execution"), "{:?}", e.detail);
        assert!(
            e.detail[0].contains("http://deployment.test:8080"),
            "{:?}",
            e.detail
        );
        assert!(e.detail[0].contains("chrome 124.0"), "{:?}", e.detail);
        assert!(e.detail[0].contains("Order total"), "{:?}", e.detail);
    }

    // Verifies: #245 — a refutation is replayable: it names where the browser was, what the page
    // failed to show, and the script that got it there. This is the strongest evidence in the
    // tool, and it is worth nothing if the operator cannot reproduce it.
    #[test]
    fn a_refutation_names_the_page_and_the_script_that_reached_it() {
        let e = Outcome::Refuted {
            at: "http://deployment.test:8080/cart".into(),
            browser: "chrome 124.0".into(),
        }
        .into_evidence(&ui(), &claim());

        assert_eq!(e.status, Status::Fails);
        assert_eq!(e.basis, None, "a refutation claims no rung");
        let witness = e
            .witness
            .expect("a refutation without a witness is unactionable");
        assert!(
            witness.contains("http://deployment.test:8080/cart"),
            "{witness}"
        );
        assert!(witness.contains("did not show `Order total`"), "{witness}");
        assert!(
            e.detail
                .iter()
                .any(|d| d.contains("click the first element")),
            "the script must be in the detail: {:?}",
            e.detail
        );
    }

    // Verifies: #245 — a claim asserting ABSENCE is refuted by presence, and the witness says so
    // in the operator's own terms rather than reporting a bare "did not show".
    #[test]
    fn refuting_an_absence_claim_says_the_text_appeared() {
        let absent = UiClaim {
            expect_absent: true,
            ..claim()
        };
        let e = Outcome::Refuted {
            at: "http://deployment.test:8080/cart".into(),
            browser: "chrome 124.0".into(),
        }
        .into_evidence(&ui(), &absent);
        let witness = e.witness.expect("witness");
        assert!(witness.contains("showed `Order total`"), "{witness}");
        assert!(witness.contains("never does"), "{witness}");
    }

    // Verifies: #245 — THE rule of this slice. Everything that is not the deployment's own answer
    // is `inconclusive`, never `fails`. A stale selector reported as a failed requirement is a
    // fabricated counterexample — the same overclaim as #233's refusal-read-as-a-pass, running in
    // the more tempting direction because a red result looks like the tool working.
    #[test]
    fn a_script_that_could_not_run_is_inconclusive_never_a_refutation() {
        let e = Outcome::Inconclusive {
            reason: "clicking `button[data-test=checkout]`: WebDriver answered 404 Not Found — no \
                     such element"
                .into(),
        }
        .into_evidence(&ui(), &claim());

        assert_eq!(e.status, Status::Unknown);
        assert_eq!(e.basis, None);
        assert!(e.witness.is_none(), "nothing was witnessed");
        assert!(e.detail[0].contains("no such element"), "{:?}", e.detail);
        assert!(
            e.detail[1].contains("nothing about the requirement was established"),
            "{:?}",
            e.detail
        );
    }

    // Verifies: #245 — with no grid configured the run is inconclusive and says whose problem it
    // is. "No grid on this machine" is the operator's environment; "no engine wired" would be
    // ours. Rendering them the same sends them looking in the wrong place (#239).
    #[test]
    fn no_configured_grid_is_an_inconclusive_that_names_the_environment() {
        // The fixture declares an endpoint, so strip it: this is the machine with nothing set.
        let bare = Ui::new("http://deployment.test:8080", None, BTreeMap::new());
        let temp = temp_env_unset();
        let outcome = run(&bare, &claim());
        drop(temp);
        let Outcome::Inconclusive { reason } = &outcome else {
            panic!("there is nothing to drive the browser with: {outcome:?}")
        };
        assert!(reason.contains("WEBDRIVER_URL"), "{reason}");
        assert!(reason.contains("not a fact about the subject"), "{reason}");
    }

    /// `WEBDRIVER_URL` removed for the duration, restored on drop — the process environment is
    /// shared by every test in this binary.
    struct EnvGuard(Option<String>);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: edition 2024 marks env mutation unsafe (other threads may read concurrently).
            // These tests run single-threaded over one shared var, guarded by set/restore.
            unsafe {
                match self.0.take() {
                    Some(v) => std::env::set_var(super::super::declaration::ENDPOINT_VAR, v),
                    None => std::env::remove_var(super::super::declaration::ENDPOINT_VAR),
                }
            }
        }
    }
    fn temp_env_unset() -> EnvGuard {
        let prior = std::env::var(super::super::declaration::ENDPOINT_VAR).ok();
        // SAFETY: see EnvGuard::drop — single-threaded test mutation of one process-wide var.
        unsafe {
            std::env::remove_var(super::super::declaration::ENDPOINT_VAR);
        }
        EnvGuard(prior)
    }

    // Verifies: #245 — a declared path is resolved against the deployment, and an absolute URL is
    // left alone. Joining `http://host:8080` and `/cart` wrongly is a check that drives the wrong
    // page and reports honestly about it.
    #[test]
    fn a_declared_path_resolves_against_the_deployment() {
        assert_eq!(
            resolve_url("http://d.test:8080", "/cart"),
            "http://d.test:8080/cart"
        );
        assert_eq!(
            resolve_url("http://d.test:8080/", "cart"),
            "http://d.test:8080/cart"
        );
        assert_eq!(
            resolve_url("http://d.test:8080", "http://other.test/x"),
            "http://other.test/x"
        );
    }

    // Verifies: #245 — the probe reads a grid's OWN account of itself. `ready: false` is present
    // but unusable (REQ051), not missing: the operator has a grid and has to free a slot, and
    // telling them to install one would be the wrong advice.
    #[test]
    fn a_grid_that_is_not_ready_is_unusable_rather_than_missing() {
        use crate::engine::EngineStatus;
        let status = serde_json::json!({
            "ready": false,
            "message": "Selenium Grid has 1 node(s), none of them ready",
        });
        let EngineStatus::Unusable { reason } = grid_state(&status) else {
            panic!("a grid answering `ready: false` is present and cannot start, not missing")
        };
        assert!(reason.contains("none of them ready"), "{reason}");
        assert_eq!(
            grid_version(&status),
            "unknown",
            "a grid with no nodes has no browsers to name"
        );

        let ready = serde_json::json!({
            "ready": true,
            "nodes": [{ "slots": [
                { "stereotype": { "browserName": "chrome", "browserVersion": "124.0" } },
                { "stereotype": { "browserName": "chrome", "browserVersion": "124.0" } },
            ]}],
        });
        assert_eq!(grid_version(&ready), "grid seating chrome 124.0");
    }

    // Verifies: #245 — a session request names a browser the grid actually offers, read from the
    // grid's own stereotype. MEASURED, and the live run is the only thing that found it: an empty
    // `alwaysMatch` ("anything you have") matches no slot, and Selenium Grid does not refuse it —
    // it QUEUES the request until the session timeout. A healthy grid then answers nothing for a
    // minute and the driver reports "could not be reached", which is a wrong reason rather than a
    // slow one. Guessing `chrome` would fail identically against a Firefox-only grid.
    #[test]
    fn the_session_asks_for_a_browser_the_grid_says_it_has() {
        let status = serde_json::json!({
            "ready": true,
            "nodes": [
                { "slots": [
                    { "stereotype": { "browserName": "chrome", "browserVersion": "124.0" } },
                    { "stereotype": { "browserName": "chrome", "browserVersion": "124.0" } },
                ]},
                { "slots": [{ "stereotype": { "browserName": "firefox" } }]},
            ],
        });
        assert_eq!(offered_browsers(&status), vec!["chrome", "firefox"]);
        // A grid with nothing registered offers nothing — the caller must say so rather than send
        // a request that hangs.
        assert!(offered_browsers(&serde_json::json!({ "ready": false })).is_empty());
    }

    // Verifies: #245 against the REAL GRID — the direction that must hold. Drives a page that
    // really contains the text and requires `holds`/`not-falsified`. Needs a grid that can reach
    // `UI_TEST_BASE_URL`, which is why it is opt-in: the deployment is the operator's (#230).
    #[test]
    #[ignore = "requires a WebDriver grid (WEBDRIVER_URL) and a deployment (UI_TEST_BASE_URL)"]
    fn real_grid_holds_when_the_page_shows_the_text() {
        let (base, text) = real_target();
        let ui = Ui::new(base, None, BTreeMap::new());
        let claim = UiClaim {
            steps: vec![Step::Goto("/".into())],
            expect: Step::TextPresent(text),
            expect_absent: false,
        };
        let outcome = run(&ui, &claim);
        let Outcome::NotFalsified { .. } = &outcome else {
            panic!("the page really does show this: {outcome:?}")
        };
        let e = outcome.into_evidence(&ui, &claim);
        assert_eq!(e.status, Status::Holds);
        assert_eq!(e.basis, Some(Basis::NotFalsified));
    }

    // Verifies: #245 against the REAL GRID — the same drive, from inside a running tokio runtime.
    // That is not a contrived case: `provreq verify` is `#[tokio::main]` and the server drives this
    // from an axum handler, so EVERY real caller is in this position and the test above is the only
    // one that is not. Green there and broken here is the shape of a bug that ships.
    #[test]
    #[ignore = "requires a WebDriver grid (WEBDRIVER_URL) and a deployment (UI_TEST_BASE_URL)"]
    fn real_grid_drives_from_inside_an_async_runtime() {
        let (base, text) = real_target();
        let ui = Ui::new(base, None, BTreeMap::new());
        let claim = UiClaim {
            steps: vec![Step::Goto("/".into())],
            expect: Step::TextPresent(text),
            expect_absent: false,
        };
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let outcome = rt.block_on(async { run(&ui, &claim) });
        let Outcome::NotFalsified { .. } = &outcome else {
            panic!("the page really does show this: {outcome:?}")
        };
    }

    // Verifies: #245 against the REAL GRID — the direction that must refute, with a witness naming
    // the page. The other half of the guard: a driver that cannot fail is not a check.
    #[test]
    #[ignore = "requires a WebDriver grid (WEBDRIVER_URL) and a deployment (UI_TEST_BASE_URL)"]
    fn real_grid_refutes_and_witnesses_text_the_page_does_not_show() {
        let (base, _) = real_target();
        let ui = Ui::new(base, None, BTreeMap::new());
        let claim = UiClaim {
            steps: vec![Step::Goto("/".into())],
            expect: Step::TextPresent("no page anywhere contains this exact sentence".into()),
            expect_absent: false,
        };
        let outcome = run(&ui, &claim);
        let Outcome::Refuted { at, .. } = &outcome else {
            panic!("the page does not show that: {outcome:?}")
        };
        assert!(at.starts_with("http"), "the witness names the page: {at}");
    }

    // Verifies: #245 against the REAL GRID — THE rule, driven end to end rather than asserted on a
    // constructed outcome. A selector that matches nothing must reach `inconclusive`; this is the
    // path that would otherwise manufacture a counterexample out of a stale script.
    #[test]
    #[ignore = "requires a WebDriver grid (WEBDRIVER_URL) and a deployment (UI_TEST_BASE_URL)"]
    fn real_grid_reports_a_selector_that_matches_nothing_as_inconclusive() {
        let (base, text) = real_target();
        let ui = Ui::new(base, None, BTreeMap::new());
        let claim = UiClaim {
            steps: vec![
                Step::Goto("/".into()),
                Step::Click("#no-such-element-anywhere".into()),
            ],
            expect: Step::TextPresent(text),
            expect_absent: false,
        };
        let outcome = run(&ui, &claim);
        let Outcome::Inconclusive { reason } = &outcome else {
            panic!("a script that could not run is never a verdict: {outcome:?}")
        };
        assert!(reason.contains("#no-such-element-anywhere"), "{reason}");
    }

    /// The deployment the real-grid tests drive, and text it really shows. Both are the operator's
    /// to supply — provreq never starts a deployment (#230).
    fn real_target() -> (String, String) {
        (
            std::env::var("UI_TEST_BASE_URL").expect("UI_TEST_BASE_URL"),
            std::env::var("UI_TEST_TEXT").expect("UI_TEST_TEXT"),
        )
    }
}
