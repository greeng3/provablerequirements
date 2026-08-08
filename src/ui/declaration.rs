//! Reading the `ui:` block out of the companion manifest.

use std::collections::BTreeMap;
use std::path::Path;

/// The environment variable that points provreq at a WebDriver grid, overriding any `endpoint:` in
/// the manifest. Mirrors `MONPOLY_BIN` (#233) and `TLA2TOOLS_JAR`.
pub const ENDPOINT_VAR: &str = "WEBDRIVER_URL";

/// One thing a browser does to the deployment, as the operator declared it.
///
/// Three, because three is what makes a check: go somewhere, do something, observe something. A
/// step vocabulary grows on demand from real requirements, never ahead of them — every entry here
/// has to be driven over the wire in slice 4 and lowered to from a claim in slice 3, so an unused
/// one is a liability in two places at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Navigate to a path, resolved against `base_url`.
    Goto(String),
    /// Click the first element matching a CSS selector.
    Click(String),
    /// Assert that some text is present in the page.
    TextPresent(String),
}

impl Step {
    /// The manifest key this step was written under — what an error message should say, because it
    /// is what the operator will search their manifest for.
    pub fn key(&self) -> &'static str {
        match self {
            Step::Goto(_) => "goto",
            Step::Click(_) => "click",
            Step::TextPresent(_) => "text_present",
        }
    }

    pub fn argument(&self) -> &str {
        match self {
            Step::Goto(a) | Step::Click(a) | Step::TextPresent(a) => a,
        }
    }
}

/// A subject's declared UI check. Absent for every subject that has not configured one, which is
/// every subject that worked before this existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ui {
    /// The deployment under check, exactly as the operator wrote it.
    base_url: String,
    /// The grid address from the manifest, if any. Not the answer on its own — [`Ui::endpoint`]
    /// applies the `WEBDRIVER_URL` override.
    declared_endpoint: Option<String>,
    steps: BTreeMap<String, Step>,
}

#[derive(serde::Deserialize)]
struct ManifestUi {
    #[serde(default)]
    ui: Option<UiBlock>,
}

#[derive(serde::Deserialize)]
struct UiBlock {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    steps: BTreeMap<String, StepBlock>,
}

/// A step as written: at most one of these keys, and at least one.
#[derive(serde::Deserialize)]
struct StepBlock {
    #[serde(default)]
    goto: Option<String>,
    #[serde(default)]
    click: Option<String>,
    #[serde(default)]
    text_present: Option<String>,
}

impl Ui {
    /// Read `ui:` from the companion `provreq.yml`.
    ///
    /// `Ok(None)` — no UI check configured — for a missing file, a missing block, or a manifest
    /// that will not parse, the same forgiving read as [`crate::spec_paths::SpecPaths::load`],
    /// [`crate::tlc::Constants::load`] and [`crate::monitor::Monitor::load`]: a subject that never
    /// configured this must not be broken by the field existing.
    ///
    /// `Err` once the block **is** there and says something provreq cannot act on. Silence is a
    /// choice not to configure; a half-written declaration is a mistake the operator is looking
    /// straight at, and dropping it would strand them debugging a check that quietly never ran.
    pub fn load(companion_root: &Path) -> Result<Option<Ui>, String> {
        let Ok(text) = std::fs::read_to_string(companion_root.join(crate::adopt::MANIFEST_FILE))
        else {
            return Ok(None);
        };
        let Ok(manifest) = serde_yaml::from_str::<ManifestUi>(&text) else {
            return Ok(None);
        };
        let Some(block) = manifest.ui else {
            return Ok(None);
        };
        Ui::from_block(block).map(Some)
    }

    fn from_block(block: UiBlock) -> Result<Ui, String> {
        let base_url = http_url(
            "ui.base_url",
            &block.base_url,
            Some(
                "name the deployment the check drives, as a URL that is already up (for example \
             `http://localhost:8080`)",
            ),
        )?
        .ok_or_else(|| {
            "`ui.base_url` in provreq.yml is empty — name the deployment the check drives, as a \
             URL that is already up (for example `http://localhost:8080`)"
                .to_string()
        })?;

        // Declared-but-unusable is an error; simply absent is not, because `WEBDRIVER_URL` is the
        // other half of this field and the manifest is the wrong place to pin a per-operator
        // address.
        let declared_endpoint =
            http_url("ui.endpoint", block.endpoint.as_deref().unwrap_or(""), None)?;

        if block.steps.is_empty() {
            return Err(
                "`ui.steps` in provreq.yml is empty — a UI check with no steps drives \
                        nothing, so declare at least one (for example `sees_total: { text_present: \
                        \"Order total\" }`)"
                    .into(),
            );
        }
        let steps = block
            .steps
            .into_iter()
            .map(|(alias, s)| declared_step(&alias, s).map(|step| (alias, step)))
            .collect::<Result<BTreeMap<_, _>, String>>()?;

        Ok(Ui {
            base_url,
            declared_endpoint,
            steps,
        })
    }

    /// Build from already-resolved parts — for tests, and any future caller that is not reading a
    /// manifest. Mirrors [`crate::monitor::Monitor::new`].
    pub fn new(
        base_url: impl Into<String>,
        declared_endpoint: Option<String>,
        steps: BTreeMap<String, Step>,
    ) -> Ui {
        Ui {
            base_url: base_url.into(),
            declared_endpoint,
            steps,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Where to reach a WebDriver grid: `WEBDRIVER_URL` if set, else the manifest's `endpoint:`,
    /// else `None`.
    ///
    /// `None` is not a load-time error and must not be reported as one. "No grid configured" is a
    /// fact about this machine and is the operator's to fix in their environment; "no engine
    /// wired" is a fact about provreq. Rendering them the same would tell an operator to go
    /// looking in the wrong place — the distinction #231 keeps between "never occurred" and "could
    /// not read the trace", one category further out.
    pub fn endpoint(&self) -> Option<String> {
        resolve_endpoint(
            std::env::var(ENDPOINT_VAR).ok().as_deref(),
            self.declared_endpoint.as_deref(),
        )
    }

    /// The step an operator's binding names, looked up by the **alias** they declared it under —
    /// the same handle/spelling split [`crate::monitor::Monitor::event`] draws. Resolving a
    /// requirement's terms against these is the next slice's job; this module only reads.
    pub fn step(&self, alias: &str) -> Option<&Step> {
        self.steps.get(alias)
    }

    /// Every declared alias — what an unresolvable binding is told it could have named.
    pub fn aliases(&self) -> Vec<String> {
        self.steps.keys().cloned().collect()
    }

    pub fn steps(&self) -> &BTreeMap<String, Step> {
        &self.steps
    }

    /// What this check *is*, as a stable string: the deployment and the steps, in declaration
    /// order.
    ///
    /// The grid endpoint is deliberately **not** in here. It is where the browser ran, not what was
    /// checked — and since it can come from `WEBDRIVER_URL`, folding it in would make a verdict go
    /// stale the moment a colleague ran with a different grid, reporting a drift in the subject
    /// that never happened. Where a verdict was produced is already the environment axis (REQ049).
    fn identity(&self) -> String {
        let mut out = format!("base_url={}\n", self.base_url);
        for (alias, step) in &self.steps {
            out.push_str(&format!("{alias}: {}={}\n", step.key(), step.argument()));
        }
        out
    }
}

/// Which grid address wins. Pure over both inputs so it is testable without touching the process
/// environment — [`Ui::endpoint`] is the one line that reads it, the same split that keeps
/// [`crate::verdict_store`] filesystem-free.
///
/// An env var set to whitespace is treated as unset: exporting an empty string is how a shell says
/// "no value", and honoring it literally would produce an endpoint no driver can reach.
fn resolve_endpoint(from_env: Option<&str>, declared: Option<&str>) -> Option<String> {
    from_env
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .or(declared)
        .map(str::to_string)
}

/// The fingerprint of the subject's declared UI check *right now* — the 8th drift axis.
///
/// `None` when no UI check is configured or the block will not load, which is every subject with no
/// category-3 requirement.
///
/// Unlike `spec_fingerprint` (#120) and `trace_fingerprint` (#230), there is no file on the far end
/// to hash: a running deployment has no bytes to read. So this hashes the **declaration**, and is
/// honest about what that buys — it sees the operator's check change, and it is **blind to the
/// deployment moving underneath it while the URL stays the same**. That gap is accepted knowingly.
/// Closing it would mean hashing the response of an operator-named version endpoint, which only
/// works if the subject exposes one and would manufacture phantom drift whenever it flaked.
pub fn current_fingerprint(companion_root: &Path) -> Option<String> {
    let ui = Ui::load(companion_root).ok()??;
    Some(fingerprint(&ui.identity()))
}

fn fingerprint(identity: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    identity.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Validate one URL-valued field. `Ok(None)` for empty; `Err` for present-but-unusable.
///
/// A scheme check rather than a parse: a WebDriver endpoint and a deployment are both reached over
/// HTTP, and the failure worth catching here is an operator writing a bare host or a file path,
/// which a driver would report far downstream as a connection error with no hint of the cause.
fn http_url(field: &str, raw: &str, hint: Option<&str>) -> Result<Option<String>, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if !value.starts_with("http://") && !value.starts_with("https://") {
        let mut msg = format!(
            "`{field}` in provreq.yml is `{value}`, which is not an HTTP URL — write it with a \
             scheme (`http://` or `https://`)"
        );
        if let Some(h) = hint {
            msg.push_str(&format!("; {h}"));
        }
        return Err(msg);
    }
    Ok(Some(value.to_string()))
}

/// One step, validated. Exactly one action key: zero declares nothing, and two would leave the
/// order they run in unstated — silently picking one would make the manifest mean something the
/// operator cannot see by reading it.
fn declared_step(alias: &str, block: StepBlock) -> Result<Step, String> {
    let candidates: Vec<Step> = [
        block.goto.map(Step::Goto),
        block.click.map(Step::Click),
        block.text_present.map(Step::TextPresent),
    ]
    .into_iter()
    .flatten()
    .collect();

    match candidates.len() {
        1 => {
            let step = candidates.into_iter().next().expect("length checked");
            if step.argument().trim().is_empty() {
                return Err(format!(
                    "step `{alias}` in provreq.yml has an empty `{}` — give it the {} it acts on",
                    step.key(),
                    match step {
                        Step::Goto(_) => "path",
                        Step::Click(_) => "CSS selector",
                        Step::TextPresent(_) => "text",
                    }
                ));
            }
            Ok(step)
        }
        0 => Err(format!(
            "step `{alias}` in provreq.yml declares no action — give it one of `goto`, `click` or \
             `text_present`"
        )),
        n => Err(format!(
            "step `{alias}` in provreq.yml declares {n} actions ({}) — a step is one action, so \
             split it into separate steps in the order they should run",
            candidates
                .iter()
                .map(Step::key)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "ui:\n  \
                            endpoint: http://localhost:4444\n  \
                            base_url: http://localhost:8080\n  \
                            steps:\n    \
                            open_cart: { goto: /cart }\n    \
                            checkout: { click: \"button[data-test=checkout]\" }\n    \
                            sees_total: { text_present: \"Order total\" }\n";

    fn companion_with(text: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join(crate::adopt::MANIFEST_FILE), text).expect("manifest");
        tmp
    }

    // Verifies: #239 — the forgiving read. A subject that predates this field is not broken by it.
    #[test]
    fn no_manifest_no_block_and_unparseable_all_mean_not_configured() {
        let empty = tempfile::tempdir().expect("tempdir");
        assert_eq!(Ui::load(empty.path()), Ok(None), "no manifest at all");

        let no_block = companion_with("subject: ../\n");
        assert_eq!(
            Ui::load(no_block.path()),
            Ok(None),
            "manifest without `ui:`"
        );

        let junk = companion_with("ui: [this is not a block\n");
        assert_eq!(
            Ui::load(junk.path()),
            Ok(None),
            "manifest that will not parse"
        );
    }

    // Verifies: #239 — strict once present, which is the other half of the same rule.
    #[test]
    fn a_declared_check_loads_every_step_under_its_alias() {
        let tmp = companion_with(MANIFEST);
        let ui = Ui::load(tmp.path()).expect("loads").expect("configured");

        assert_eq!(ui.base_url(), "http://localhost:8080");
        assert_eq!(ui.step("open_cart"), Some(&Step::Goto("/cart".into())));
        assert_eq!(
            ui.step("checkout"),
            Some(&Step::Click("button[data-test=checkout]".into()))
        );
        assert_eq!(
            ui.step("sees_total"),
            Some(&Step::TextPresent("Order total".into()))
        );
        assert_eq!(ui.aliases(), vec!["checkout", "open_cart", "sees_total"]);
        assert_eq!(ui.step("nope"), None);
    }

    // Verifies: #239 — a block that IS there and is half-written names the key, rather than being
    // read as "not configured" and leaving the operator debugging a check that never ran.
    #[test]
    fn a_half_written_block_names_the_key_it_cannot_act_on() {
        let no_base = companion_with("ui:\n  steps:\n    a: { goto: / }\n");
        let err = Ui::load(no_base.path()).expect_err("empty base_url is an error");
        assert!(err.contains("ui.base_url"), "{err}");

        let no_steps = companion_with("ui:\n  base_url: http://localhost:8080\n");
        let err = Ui::load(no_steps.path()).expect_err("no steps is an error");
        assert!(err.contains("ui.steps"), "{err}");
        assert!(err.contains("drives nothing"), "{err}");
    }

    // Verifies: #239 — a bare host reaches the driver as an unexplained connection failure, so it
    // is refused here where the operator can still see which field caused it.
    #[test]
    fn a_url_without_a_scheme_is_refused_naming_the_field() {
        let tmp = companion_with("ui:\n  base_url: localhost:8080\n  steps:\n    a: { goto: / }\n");
        let err = Ui::load(tmp.path()).expect_err("no scheme is an error");
        assert!(err.contains("ui.base_url"), "{err}");
        assert!(err.contains("http://"), "{err}");

        let tmp = companion_with(
            "ui:\n  base_url: http://x.test\n  endpoint: 172.17.0.2:4444\n  steps:\n    \
             a: { goto: / }\n",
        );
        let err = Ui::load(tmp.path()).expect_err("no scheme is an error");
        assert!(err.contains("ui.endpoint"), "{err}");
    }

    // Verifies: #239 — zero actions declares nothing; two leave the running order unstated, and
    // silently picking one would make the manifest mean something unreadable from the manifest.
    #[test]
    fn a_step_is_exactly_one_action() {
        let none = companion_with("ui:\n  base_url: http://x.test\n  steps:\n    a: {}\n");
        let err = Ui::load(none.path()).expect_err("no action is an error");
        assert!(err.contains("declares no action"), "{err}");
        assert!(err.contains('a'), "{err}");

        let two = companion_with(
            "ui:\n  base_url: http://x.test\n  steps:\n    a: { goto: /, click: \"#go\" }\n",
        );
        let err = Ui::load(two.path()).expect_err("two actions is an error");
        assert!(err.contains("declares 2 actions"), "{err}");
        assert!(err.contains("goto") && err.contains("click"), "{err}");

        let blank =
            companion_with("ui:\n  base_url: http://x.test\n  steps:\n    a: { click: \"\" }\n");
        let err = Ui::load(blank.path()).expect_err("an empty selector is an error");
        assert!(err.contains("CSS selector"), "{err}");
    }

    // Verifies: #239 — the endpoint is environment config, not subject config. A manifest without
    // one still loads (the check is fully declared without it), and `WEBDRIVER_URL` wins when set,
    // so no operator has to edit a committed file to point at their own grid. #225 is open against
    // exactly that mistake for the LLM endpoint.
    #[test]
    fn the_endpoint_is_optional_and_the_env_var_overrides_it() {
        let declared = Some("http://in-manifest:4444");

        assert_eq!(
            resolve_endpoint(None, declared).as_deref(),
            Some("http://in-manifest:4444"),
            "unset falls back to the manifest"
        );
        assert_eq!(
            resolve_endpoint(Some("http://from-env:4444"), declared).as_deref(),
            Some("http://from-env:4444"),
            "the env var wins, so no operator edits a committed file to reach their own grid"
        );
        assert_eq!(
            resolve_endpoint(Some("http://from-env:4444"), None).as_deref(),
            Some("http://from-env:4444"),
            "the env var alone is enough"
        );
        assert_eq!(
            resolve_endpoint(Some("  "), declared).as_deref(),
            Some("http://in-manifest:4444"),
            "an env var exported empty means unset, not an unreachable endpoint"
        );
        assert_eq!(
            resolve_endpoint(None, None),
            None,
            "unset and undeclared is not an error — it is a fact about this machine"
        );

        // And a manifest with no `endpoint:` still loads: the check is fully declared without it.
        let tmp = companion_with("ui:\n  base_url: http://x.test\n  steps:\n    a: { goto: / }\n");
        let ui = Ui::load(tmp.path()).expect("loads").expect("configured");
        assert_eq!(ui.declared_endpoint, None);
    }

    // Verifies: #239 — the drift axis sees the check change.
    #[test]
    fn the_fingerprint_moves_when_the_declared_check_moves() {
        let empty = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            current_fingerprint(empty.path()),
            None,
            "no UI check configured has no fingerprint to compare"
        );

        let tmp = companion_with(MANIFEST);
        let before = current_fingerprint(tmp.path()).expect("configured");

        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            MANIFEST.replace("Order total", "Grand total"),
        )
        .expect("manifest");
        assert_ne!(
            current_fingerprint(tmp.path()),
            Some(before.clone()),
            "an edited step is a different check"
        );

        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            MANIFEST.replace("localhost:8080", "staging.test:8080"),
        )
        .expect("manifest");
        assert_ne!(
            current_fingerprint(tmp.path()),
            Some(before.clone()),
            "a different deployment is a different check"
        );
    }

    // Verifies: #239 — the grid address is where the browser ran, not what was checked. Folding it
    // into the fingerprint would make a verdict go stale the moment a colleague ran against their
    // own grid, reporting a drift in the subject that never happened. REQ049 already carries
    // "where this was proved".
    #[test]
    fn the_fingerprint_ignores_the_grid_endpoint() {
        let tmp = companion_with(MANIFEST);
        let before = current_fingerprint(tmp.path()).expect("configured");

        std::fs::write(
            tmp.path().join(crate::adopt::MANIFEST_FILE),
            MANIFEST.replace("localhost:4444", "172.17.0.2:4444"),
        )
        .expect("manifest");
        assert_eq!(
            current_fingerprint(tmp.path()),
            Some(before),
            "a different grid is the same check"
        );
    }
}
