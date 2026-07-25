//! The build-env half of Design C's strategy select — **detect and advise, not detect and exec.**
//!
//! Design C's front door picks a build environment per subject: the subject ships a dev-container,
//! or it does not. [ADR #104](../docs/devcontainer-branch-decision.md) decided what provreq does
//! with that fact: it **reports** it and explains a missing engine in terms of it. It never reaches
//! into a container. There is deliberately no docker probe here — probing a privilege we have
//! decided not to use would be speculative work and a misleading signal.
//!
//! Why this earns its place: `provreq engines` can say `Creusot MISSING` all day without telling
//! the operator anything they can act on. The heavy tier (Creusot, Prusti, MonPoly) has no native
//! install recipe by decision, so "missing" is only actionable in terms of the subject's own build
//! environment — which is exactly what this module resolves.
//!
//! Implements: REQ048.

use std::path::{Path, PathBuf};

/// The build environment a subject offers, as resolved from its dev-container config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEnv {
    /// No dev-container config in the subject. Native provisioning is the only path, so the
    /// light tier is reachable (`provreq install …`) and the heavy tier is not.
    Native,
    /// The subject's dev-container is a **prebuilt image**. This is the common case, not the
    /// exception: this repo's own devcontainer.json names an image rather than building a
    /// Dockerfile, which is why there is often no Dockerfile to inherit at all.
    Image { config: PathBuf, image: String },
    /// The subject's dev-container is **built from a Dockerfile** in the repo.
    Build { config: PathBuf, dockerfile: String },
    /// A dev-container config exists but could not be read or understood. Reported as its own
    /// state rather than collapsed into `Native` — "there is no dev-container" and "there is one
    /// and provreq could not read it" are different facts, and the operator can act on the second.
    Unreadable { config: PathBuf, reason: String },
}

impl BuildEnv {
    /// One operator-facing line naming the strategy and where it came from.
    pub fn describe(&self) -> String {
        match self {
            BuildEnv::Native => "native — no dev-container config in this subject".to_string(),
            BuildEnv::Image { config, image } => {
                format!(
                    "dev-container ({}) — prebuilt image {image}",
                    config.display()
                )
            }
            BuildEnv::Build { config, dockerfile } => {
                format!("dev-container ({}) — builds {dockerfile}", config.display())
            }
            BuildEnv::Unreadable { config, reason } => {
                format!(
                    "dev-container ({}) — present but unreadable: {reason}",
                    config.display()
                )
            }
        }
    }

    /// Whether the subject offers a dev-container at all. An unreadable config still counts as
    /// one offered — the operator has something to fix, not something to add.
    pub fn has_dev_container(&self) -> bool {
        !matches!(self, BuildEnv::Native)
    }
}

/// The engine layer this project publishes — a concrete thing to point an operator at, rather
/// than advice to go build the heavy tier themselves (which the Design-C decision says not to).
const PROVREQ_ENGINE_IMAGE: &str = "ghcr.io/greeng3/provreq-devcontainer";

/// What the operator can actually do about the engines that are missing, given what this subject
/// offers. Pure over its inputs, so the wording is testable without probing anything.
///
/// `missing_light` carries each engine's `provreq install` argument, because a light-tier engine's
/// answer is a command; `missing_heavy` carries names only, because a heavy-tier engine's answer
/// is a build environment.
pub fn advice(
    env: &BuildEnv,
    missing_light: &[(String, &'static str)],
    missing_heavy: &[String],
) -> Vec<String> {
    let mut lines = Vec::new();

    if !missing_light.is_empty() {
        let names = missing_light
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let commands = missing_light
            .iter()
            .map(|(_, arg)| format!("`provreq install {arg}`"))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("{names}: install natively — {commands}."));
    }

    if missing_heavy.is_empty() {
        return lines;
    }

    let names = missing_heavy.join(", ");
    lines.push(format!(
        "{names}: no native install by decision (docs/design-c-decision.md) — {}.",
        heavy_tier_advice(env)
    ));
    lines
}

/// What to do about a heavy-tier engine, given this subject's environment. Carries **no engine
/// names**, so both `engines` (which reports several at once) and `install` (which has already
/// named the one the operator asked for) can say the same true thing without repeating themselves.
pub fn heavy_tier_advice(env: &BuildEnv) -> String {
    match env {
        BuildEnv::Native => format!(
            "this subject ships no dev-container, so the engine layer is unreachable here; add \
             a dev-container carrying it ({PROVREQ_ENGINE_IMAGE} is one), or verify with the \
             light tier only"
        ),
        // Do not tell an operator to extend our own image with our own image. If they are already
        // running it and an engine is still absent, the image is stale, not missing a layer.
        BuildEnv::Image { image, .. } if image.starts_with(PROVREQ_ENGINE_IMAGE) => format!(
            "this subject's dev-container already runs provreq's engine image ({image}), which \
             carries the heavy tier — so a missing engine here means the container is stale; \
             re-pull it and re-open the subject"
        ),
        BuildEnv::Image { image, .. } => format!(
            "this subject's dev-container runs {image}; if that image does not carry the engine \
             layer, extend it with one ({PROVREQ_ENGINE_IMAGE} carries it) and re-open the \
             subject in it"
        ),
        BuildEnv::Build { dockerfile, .. } => format!(
            "this subject's dev-container builds {dockerfile}; add the engine layer there and \
             rebuild it"
        ),
        BuildEnv::Unreadable { reason, .. } => format!(
            "this subject has a dev-container provreq could not read ({reason}), so it cannot \
             say whether that environment carries the engine layer"
        ),
    }
}

/// The dev-container config locations the spec allows, in resolution order.
const CONFIG_CANDIDATES: [&str; 2] = [".devcontainer/devcontainer.json", ".devcontainer.json"];

/// Resolve the subject's build environment. Never fails: an unreadable or unparseable config is a
/// reported state, not an error, because a missing engine still has to be explained.
pub fn detect(subject_root: &Path) -> BuildEnv {
    let Some(config) = locate_config(subject_root) else {
        return BuildEnv::Native;
    };
    let text = match std::fs::read_to_string(&config) {
        Ok(t) => t,
        Err(e) => {
            return BuildEnv::Unreadable {
                config,
                reason: format!("could not read it: {e}"),
            }
        }
    };
    classify(config, &text)
}

/// Find the subject's dev-container config: the two spec locations, then a single-level
/// `.devcontainer/<folder>/devcontainer.json` (the spec's multi-config layout).
fn locate_config(subject_root: &Path) -> Option<PathBuf> {
    for candidate in CONFIG_CANDIDATES {
        let path = subject_root.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    let nested = std::fs::read_dir(subject_root.join(".devcontainer")).ok()?;
    let mut found: Vec<PathBuf> = nested
        .flatten()
        .map(|e| e.path().join("devcontainer.json"))
        .filter(|p| p.is_file())
        .collect();
    // Sorted so a subject with several sub-configs resolves deterministically rather than by
    // whatever order the filesystem hands back.
    found.sort();
    found.into_iter().next()
}

/// Classify a config's text. `image` wins over `build` when both are present, matching the
/// devcontainer spec's own precedence.
fn classify(config: PathBuf, text: &str) -> BuildEnv {
    let value: serde_json::Value = match serde_json::from_str(&strip_jsonc(text)) {
        Ok(v) => v,
        Err(e) => {
            return BuildEnv::Unreadable {
                config,
                reason: format!("not valid devcontainer JSON: {e}"),
            }
        }
    };

    if let Some(image) = value.get("image").and_then(|v| v.as_str()) {
        return BuildEnv::Image {
            config,
            image: image.to_string(),
        };
    }
    // `build.dockerfile` is the spec's key; `dockerFile` is the older top-level spelling, still
    // found in the wild and cheap to accept.
    let dockerfile = value
        .get("build")
        .and_then(|b| b.get("dockerfile"))
        .and_then(|v| v.as_str())
        .or_else(|| value.get("dockerFile").and_then(|v| v.as_str()));
    match dockerfile {
        Some(dockerfile) => BuildEnv::Build {
            config,
            dockerfile: dockerfile.to_string(),
        },
        None => BuildEnv::Unreadable {
            config,
            reason: "no `image` and no `build.dockerfile` — provreq cannot tell what this \
                     dev-container runs"
                .to_string(),
        },
    }
}

/// Strip JSONC to JSON: line/block comments and trailing commas, both legal in devcontainer.json
/// (this repo's own config uses comments) and both rejected by a strict JSON parser.
///
/// String-aware, so a `//` or a comma inside a value survives — an image tag with a `//` in it
/// would otherwise be silently truncated into an unreadable config.
///
/// `// ponytail: ~40 lines instead of a JSONC dependency, and its failure mode is an honest
/// `Unreadable`, never a wrong answer. Take the crate if devcontainer parsing ever needs more.`
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }

    strip_trailing_commas(&out)
}

/// Remove a comma that is followed only by whitespace and a closing `}`/`]`. Runs after comment
/// stripping, so a comma left dangling by a removed comment is caught too.
fn strip_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;

    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject_with(config_path: &str, body: &str) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join(config_path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, body).expect("write config");
        tmp
    }

    // Verifies: REQ048 — a subject with no dev-container config resolves to native, which is what
    // makes "the heavy tier is unreachable here" an honest statement rather than a guess.
    #[test]
    fn no_config_is_native() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(detect(tmp.path()), BuildEnv::Native);
        assert!(!BuildEnv::Native.has_dev_container());
    }

    // Verifies: REQ048 — an `image:` config is first-class. This is the shape of this repo's own
    // devcontainer.json, comments and all, and ADR #104 (E4) records it as the common case.
    #[test]
    fn image_config_with_comments_and_trailing_comma_resolves() {
        let tmp = subject_with(
            ".devcontainer/devcontainer.json",
            r#"{
                // Pull the prebuilt image instead of building locally.
                "name": "Subject",
                /* block comment */
                "image": "ghcr.io/example/dev:latest",
            }"#,
        );
        match detect(tmp.path()) {
            BuildEnv::Image { image, config } => {
                assert_eq!(image, "ghcr.io/example/dev:latest");
                assert!(config.ends_with(".devcontainer/devcontainer.json"));
            }
            other => panic!("expected an image dev-container, got {other:?}"),
        }
    }

    // Verifies: REQ048 — the Dockerfile-building case, under both the spec's `build.dockerfile`
    // and the older top-level `dockerFile` spelling.
    #[test]
    fn dockerfile_config_resolves_under_either_spelling() {
        let spec = subject_with(
            ".devcontainer/devcontainer.json",
            r#"{"build": {"dockerfile": "Dockerfile", "context": ".."}}"#,
        );
        assert!(matches!(
            detect(spec.path()),
            BuildEnv::Build { ref dockerfile, .. } if dockerfile == "Dockerfile"
        ));

        let legacy = subject_with(".devcontainer.json", r#"{"dockerFile": "Dockerfile.dev"}"#);
        assert!(matches!(
            detect(legacy.path()),
            BuildEnv::Build { ref dockerfile, .. } if dockerfile == "Dockerfile.dev"
        ));
    }

    // Verifies: REQ048 — a present-but-broken config is its own state. Collapsing it into
    // `Native` would tell the operator to add a dev-container they already have.
    #[test]
    fn unreadable_config_is_not_reported_as_native() {
        let malformed = subject_with(".devcontainer/devcontainer.json", "{ not json at all ");
        let env = detect(malformed.path());
        assert!(matches!(env, BuildEnv::Unreadable { .. }), "{env:?}");
        assert!(env.has_dev_container(), "a broken config is still a config");

        let empty = subject_with(
            ".devcontainer/devcontainer.json",
            r#"{"name": "no env here"}"#,
        );
        assert!(matches!(detect(empty.path()), BuildEnv::Unreadable { .. }));
    }

    // Verifies: REQ048 — the spec's multi-config layout resolves, deterministically when there
    // are several.
    #[test]
    fn nested_config_folder_resolves_deterministically() {
        let tmp = subject_with(
            ".devcontainer/zzz-last/devcontainer.json",
            r#"{"image": "z"}"#,
        );
        std::fs::create_dir_all(tmp.path().join(".devcontainer/aaa-first")).expect("mkdir");
        std::fs::write(
            tmp.path().join(".devcontainer/aaa-first/devcontainer.json"),
            r#"{"image": "a"}"#,
        )
        .expect("write");
        assert!(matches!(
            detect(tmp.path()),
            BuildEnv::Image { ref image, .. } if image == "a"
        ));
    }

    // Verifies: REQ048 — comment stripping is string-aware. A `//` inside a value is data, and
    // truncating it would turn a perfectly good config into an unreadable one.
    #[test]
    fn comment_stripping_does_not_touch_string_contents() {
        let tmp = subject_with(
            ".devcontainer/devcontainer.json",
            r#"{"image": "registry.example.com//weird:1.0", "n": "a,"}"#,
        );
        assert!(matches!(
            detect(tmp.path()),
            BuildEnv::Image { ref image, .. } if image == "registry.example.com//weird:1.0"
        ));
    }

    // Verifies: REQ048 — a light-tier engine's advice is the command that installs it, and a
    // heavy-tier engine's advice is about the build env. The two never blur.
    #[test]
    fn advice_separates_the_command_answer_from_the_build_env_answer() {
        let light = vec![("Kani".to_string(), "kani")];
        let heavy = vec!["Creusot".to_string()];
        let lines = advice(&BuildEnv::Native, &light, &heavy);

        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("`provreq install kani`"), "{lines:?}");
        assert!(
            !lines[0].contains("dev-container"),
            "a light-tier engine's answer is a command, not an environment: {lines:?}"
        );
        assert!(lines[1].contains("Creusot"), "{lines:?}");
        assert!(
            lines[1].contains("no dev-container"),
            "a native subject cannot reach the heavy tier, and must be told why: {lines:?}"
        );
    }

    // Verifies: REQ048 — heavy-tier advice names what THIS subject actually offers, which is the
    // whole point of detecting: generic "use a devcontainer" is what we are replacing.
    #[test]
    fn heavy_tier_advice_names_this_subjects_environment() {
        let heavy = vec!["Prusti".to_string()];

        let image = BuildEnv::Image {
            config: PathBuf::from(".devcontainer/devcontainer.json"),
            image: "ghcr.io/example/subject:2".to_string(),
        };
        let lines = advice(&image, &[], &heavy);
        assert!(lines[0].contains("ghcr.io/example/subject:2"), "{lines:?}");

        let build = BuildEnv::Build {
            config: PathBuf::from(".devcontainer/devcontainer.json"),
            dockerfile: "Dockerfile.verify".to_string(),
        };
        let lines = advice(&build, &[], &heavy);
        assert!(lines[0].contains("Dockerfile.verify"), "{lines:?}");

        let broken = BuildEnv::Unreadable {
            config: PathBuf::from(".devcontainer/devcontainer.json"),
            reason: "not valid devcontainer JSON".to_string(),
        };
        let lines = advice(&broken, &[], &heavy);
        assert!(
            lines[0].contains("could not read"),
            "an unreadable config must not be advised as if it were absent: {lines:?}"
        );
    }

    // Verifies: REQ048 — advice never tells an operator to extend provreq's own engine image
    // with provreq's own engine image. Running it already and still missing an engine means the
    // container is stale, which is a different action.
    #[test]
    fn advice_does_not_tell_you_to_extend_the_image_you_are_running() {
        let ours = BuildEnv::Image {
            config: PathBuf::from(".devcontainer/devcontainer.json"),
            image: format!("{PROVREQ_ENGINE_IMAGE}:latest"),
        };
        let line = heavy_tier_advice(&ours);
        assert!(line.contains("stale"), "{line}");
        assert!(
            !line.contains("extend it"),
            "extending our image with our image is not advice: {line}"
        );

        let theirs = BuildEnv::Image {
            config: PathBuf::from(".devcontainer/devcontainer.json"),
            image: "ghcr.io/example/other:1".to_string(),
        };
        assert!(heavy_tier_advice(&theirs).contains("extend it"));
    }

    // Verifies: REQ048 — nothing missing, nothing to advise. The build-env line still prints (the
    // caller does that), but no engine advice is invented.
    #[test]
    fn nothing_missing_yields_no_advice() {
        assert!(advice(&BuildEnv::Native, &[], &[]).is_empty());
    }

    // Verifies: REQ048 — provreq's own dev-container resolves, since it is the closest thing to a
    // real subject on hand and the ADR's evidence rests on what it says.
    #[test]
    fn this_repos_own_dev_container_resolves_to_its_image() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
        match detect(repo) {
            BuildEnv::Image { image, .. } => assert!(
                image.contains("provreq-devcontainer"),
                "unexpected image {image}"
            ),
            other => panic!("this repo ships an image dev-container, got {other:?}"),
        }
    }
}
