//! Which parts of a subject repository provreq walks — the one rule, shared by every walk.
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

/// Directories pruned by name: the build/dependency trees of the ecosystems a subject is likely to
/// be written in. Version control and virtualenv directories are hidden, so [`is_pruned_dir`]'s
/// dot-rule already covers them.
const PRUNE_DIRS: [&str; 2] = ["target", "node_modules"];

/// The Cache Directory Tagging Specification's signature — the first line of a `CACHEDIR.TAG` in a
/// directory whose contents are regenerable cache. Cargo writes one into `target/`.
const CACHEDIR_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

/// Whether a directory is pruned from a walk of the subject.
///
/// Three rules, because none covers the others. **Hidden** (a leading `.`) catches the tooling a
/// repository accumulates — `.git`, `.venv`, `.github`, an agent's home — none of which is source
/// the operator wrote, and no name list can enumerate them; a crate never keeps its source in one.
/// **By name** catches `node_modules` and `target`, which are neither hidden nor always tagged.
/// **By `CACHEDIR.TAG`** catches a cache whatever it is called — including a cargo target directory
/// relocated by `CARGO_TARGET_DIR`, which a name list can never anticipate, and the build directory
/// of an ecosystem this list has never heard of.
///
/// The hidden rule was added after a live run grounded nothing: this repo exposes the agent's home
/// as `.claude-home/`, holding scratch *copies of provreq's own source*, so every binding in REQ047
/// parked as `Ambiguous` against six candidates — five of them files nobody wrote here.
///
/// `depth` is the entry's distance from the root of the walk, and **the root itself is never
/// pruned**: `walkdir` applies a `filter_entry` predicate to depth 0 as well, so a subject checked
/// out at a dotted path (`~/.local/src/thing`) would otherwise prune its own root and resolve every
/// binding against nothing at all.
///
/// `filter_entry` hands its predicate every entry, files included, so a caller must ask this only
/// of a directory and [`is_pruned_file`] only of a file. Both adapters once asked this of
/// everything (#294), which had the hidden rule excluding hidden *files* — the right answer for an
/// AppleDouble sidecar, by an argument that was never about files, and `.rustfmt.toml` excluded
/// alongside it. Each rule now answers for the kind of entry it is an argument about.
pub fn is_pruned_dir(path: &Path, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str());
    let hidden = name.is_some_and(|n| n.starts_with('.'));
    let named = name.is_some_and(|n| PRUNE_DIRS.contains(&n));
    hidden || named || is_cache_dir(path)
}

/// Whether a file is excluded from a walk of the subject.
///
/// The operating system's own records rather than the author's: `.DS_Store` holds how a folder was
/// last displayed, and a `._`-prefixed AppleDouble sidecar holds the metadata of the file beside
/// it. The sidecar is why this is a rule and not something each walk improvises — it is named after
/// the file it shadows and carries that file's extension, so `._auth.rs` passes an adapter's `.rs`
/// test and would otherwise stand as a second declaration of a name the author wrote once.
///
/// Those two names and nothing else, and deliberately **not** hidden files at large.
/// [`is_pruned_dir`]'s hidden rule is a claim about directories — a crate keeps no source in one —
/// and it does not hold of files, which an author hides routinely: `.rustfmt.toml` is the subject's
/// own. Excluding wrongly narrows what every binding can resolve against, which is the more
/// damaging of the two errors here as it is for [`is_cache_dir`].
pub fn is_pruned_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == ".DS_Store" || n.starts_with("._"))
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
            assert!(is_pruned_dir(&dir, 1), "{name} should be pruned");
        }
        for name in ["src", "tests", "requirements-doorstop"] {
            let dir = tmp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(!is_pruned_dir(&dir, 1), "{name} should be walked");
        }
    }

    // Verifies: REQ060 — a cache directory is pruned by its tag whatever it is named, which is what
    // catches a `CARGO_TARGET_DIR` pointed somewhere a name list cannot anticipate.
    #[test]
    fn prunes_a_tagged_cache_directory_under_any_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("build-output");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            !is_pruned_dir(&dir, 1),
            "untagged, so it is the subject's own"
        );

        std::fs::write(
            dir.join("CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n# created by cargo\n",
        )
        .unwrap();
        assert!(is_pruned_dir(&dir, 1), "tagged as cache, so pruned");
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
        assert!(!is_pruned_dir(&dir, 1));
    }

    // Verifies: REQ060 — a hidden directory is pruned whatever it is called. The case that found
    // this: `.claude-home/` exposes an agent's scratch copies of the subject's *own* source, so
    // every binding in a real run parked as `Ambiguous` against files nobody wrote in the subject.
    #[test]
    fn prunes_hidden_directories_no_name_list_could_enumerate() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [".claude-home", ".github", ".cargo", ".anything-at-all"] {
            let dir = tmp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(
                is_pruned_dir(&dir, 1),
                "{name} is tooling, not subject source"
            );
        }
    }

    // Verifies: REQ060 — the root of the walk is never pruned, however it is named. `walkdir` hands
    // its `filter_entry` predicate the depth-0 entry too, so without this a subject checked out at
    // a dotted path would prune itself and resolve every binding against an empty tree.
    #[test]
    fn never_prunes_the_root_of_the_walk() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(".dotfiles");
        std::fs::create_dir_all(&root).unwrap();
        assert!(
            !is_pruned_dir(&root, 0),
            "the subject's own root is never pruned"
        );
        assert!(is_pruned_dir(&root, 1), "the same name below the root is");
    }

    // Verifies: REQ060 / #294 — the operating system's own records are excluded by a rule about
    // files, no longer as a side effect of a rule about directories. The AppleDouble sidecar is why
    // the rule is needed: it is named after the file it shadows and carries that file's extension.
    #[test]
    fn prunes_the_operating_systems_own_files() {
        for name in [".DS_Store", "._auth.rs", "._spec.tla", "._.DS_Store"] {
            assert!(
                is_pruned_file(Path::new(name)),
                "{name} is the operating system's record, not the author's source"
            );
        }
    }

    // Verifies: REQ060 / #294 — and nothing more than those. The hidden rule is a claim about
    // directories that is false of files, which an author hides routinely, so it does not cross
    // over: excluding a file wrongly narrows what every binding can resolve against.
    #[test]
    fn reads_files_the_author_wrote_however_they_are_named() {
        for name in [
            "auth.rs",
            "Spec.tla",
            ".rustfmt.toml",
            "_auth.rs",
            "DS_Store.rs",
        ] {
            assert!(
                !is_pruned_file(Path::new(name)),
                "{name} may be the subject's own"
            );
        }
    }
}
