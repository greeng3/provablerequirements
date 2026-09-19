//! The end-to-end formalization guide, embedded in the binary so it travels on every
//! install path (build-from-source, release tarball, baked image). `provreq guide` prints
//! it or writes it to a file; the same markdown is the source of truth in `docs/`.

use anyhow::{Context, Result};
use std::path::Path;

/// The guide text, compiled in from the source-of-truth markdown.
pub const GUIDE: &str = include_str!("../docs/end-to-end-guide.md");

/// Print the guide to stdout, or write it to `out` when a path is given.
pub fn run_guide(out: Option<&Path>) -> Result<()> {
    match out {
        Some(path) => {
            std::fs::write(path, GUIDE)
                .with_context(|| format!("failed to write the guide to {}", path.display()))?;
            println!("Wrote the provreq guide to {}", path.display());
        }
        None => print!("{GUIDE}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The embedded guide must actually carry the workflow, or `provreq guide` ships an
    // empty promise. Anchor on the pipeline spine and the fidelity gap that motivated it
    // (#469) rather than on incidental prose.
    #[test]
    fn embedded_guide_covers_the_pipeline_and_fidelity() {
        assert!(GUIDE.len() > 2000, "guide is suspiciously short");
        for anchor in [
            "provreq init",
            "draft --translate",
            "draft --check",
            "draft --ground",
            "draft --admit",
            "draft --writeback",
            "provreq verify",
            "definitional",
            "observed",
            "probed",
            "weakest binding",
        ] {
            assert!(GUIDE.contains(anchor), "guide is missing `{anchor}`");
        }
    }

    #[test]
    fn writes_the_guide_to_a_file() {
        let dir = std::env::temp_dir().join(format!("provreq-guide-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("guide.md");
        run_guide(Some(&out)).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), GUIDE);
        std::fs::remove_dir_all(&dir).ok();
    }
}
