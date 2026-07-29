//! Which directories of a subject repository provreq walks — the one rule, shared by every walk.
//!
//! Found by the second dogfood pass (#143): `adopt` and `doorstop` pruned build directories, but
//! the two **adapters** that resolve bindings ([`crate::rust_adapter`], [`crate::tla_adapter`]) did
//! not, so every resolution traversed this repo's own 2.6 GB `target/`. A dry-run of one
//! requirement took 34 seconds of almost pure I/O — for 57 source files. Four separate walks had
//! four separate opinions about which files count, which is exactly the kind of disagreement one
//! shared rule exists to prevent.
//!
//! Pruning is not only about speed. A binding resolves against **the subject's source**, and
//! generated code under a build directory is not that: a name declared there is a copy or an
//! artifact, and finding it would park a correct binding as [`crate::rust_adapter::Resolution::Ambiguous`]
//! against a file the operator never wrote.

use std::path::Path;

/// Directories pruned by name: version control, and the build/dependency trees of the ecosystems a
/// subject is likely to be written in.
const PRUNE_DIRS: [&str; 4] = [".git", "target", "node_modules", ".venv"];

/// The Cache Directory Tagging Specification's signature — the first line of a `CACHEDIR.TAG` in a
/// directory whose contents are regenerable cache. Cargo writes one into `target/`.
const CACHEDIR_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

/// Whether a directory is pruned from a walk of the subject.
///
/// Two rules, because neither covers the other. **By name** catches `node_modules` and `.venv`,
/// which carry no marker. **By `CACHEDIR.TAG`** catches a cache whatever it is called — including a
/// cargo target directory relocated by `CARGO_TARGET_DIR`, which a name list can never anticipate,
/// and the build directory of an ecosystem this list has never heard of.
pub fn is_pruned_dir(path: &Path) -> bool {
    let named = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| PRUNE_DIRS.contains(&n));
    named || is_cache_dir(path)
}

/// Whether a directory is tagged as regenerable cache. A missing or unreadable tag simply means
/// "not tagged" — the walk goes in, which is the safe direction: pruning a directory the subject's
/// real source lives in would silently narrow what any binding can resolve against.
fn is_cache_dir(path: &Path) -> bool {
    std::fs::read(path.join("CACHEDIR.TAG")).is_ok_and(|tag| tag.starts_with(CACHEDIR_SIGNATURE))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: REQ060 — the named build and VCS directories are pruned, and ordinary source
    // directories are not. The names are the ones `adopt` and `doorstop` already pruned before this
    // rule was shared, so adopting it changes nothing for them.
    #[test]
    fn prunes_build_and_vcs_directories_by_name() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [".git", "target", "node_modules", ".venv"] {
            let dir = tmp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(is_pruned_dir(&dir), "{name} should be pruned");
        }
        for name in ["src", "tests", "requirements-doorstop"] {
            let dir = tmp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(!is_pruned_dir(&dir), "{name} should be walked");
        }
    }

    // Verifies: REQ060 — a cache directory is pruned by its tag whatever it is named, which is what
    // catches a `CARGO_TARGET_DIR` pointed somewhere a name list cannot anticipate.
    #[test]
    fn prunes_a_tagged_cache_directory_under_any_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("build-output");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_pruned_dir(&dir), "untagged, so it is the subject's own");

        std::fs::write(
            dir.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n# created by cargo\n",
        )
        .unwrap();
        assert!(is_pruned_dir(&dir), "tagged as cache, so pruned");
    }

    // Verifies: REQ060 — a file that merely happens to be named `CACHEDIR.TAG`-adjacent, or one
    // whose contents are not the signature, does not prune a real source directory. Pruning wrongly
    // narrows what every binding can resolve against, so the doubtful case walks in.
    #[test]
    fn an_unsigned_tag_does_not_prune() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("src");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("CACHEDIR.TAG"), "not the signature\n").unwrap();
        assert!(!is_pruned_dir(&dir));
    }
}
