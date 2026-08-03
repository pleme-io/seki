//! Locating the file that marks a project root.
//!
//! ── ★ WHY THIS EXISTS: THE `stable` BUG, AND ITS TWIN ───────────────────
//! The `rust` segment printed `stable` in every directory on this machine.
//! Two defects stacked:
//!
//!   1. `read_toolchain_channel` looked ONLY in `ctx.cwd`, so anywhere
//!      below a repo root it found no `rust-toolchain.toml`.
//!   2. The caller turned that miss into `unwrap_or_else(|| "stable")` and
//!      rendered it — a value presented as measured that was never
//!      measured.
//!
//! Fixing (2) alone would have replaced a wrong reading with silence.
//! Fixing (1) is what makes the segment tell the truth from a subdirectory,
//! which is where a shell actually spends its life.
//!
//! The reviewable point: **(1) was not unique to `rust`**. `nix_flake_drift`
//! gated on `ctx.cwd.join("flake.nix").is_file()` and had the identical
//! blind spot — silent in every subdirectory of every flake repo in the
//! fleet, which is nearly every directory the operator stands in. It read
//! as "no drift to report" and was really "never looked". Two independent
//! copies of walk-up-to-find-a-marker, one correct and one absent, is the
//! signal that the walk belongs here rather than in each segment.
//!
//! ── ★ IT RETURNS THE DIRECTORY, NOT A BOOL ──────────────────────────────
//! A `bool` would have fixed detection and left `nix_flake_drift` running
//! `nix flake metadata` in `cwd`, where there is no `flake.nix` — detection
//! would say yes and the probe would fail, which is a worse failure than
//! the silence it replaced because it looks like a broken nix. Callers need
//! the ROOT, so that is what this returns.

use std::path::{Path, PathBuf};

/// Walk from `start` toward the filesystem root, returning the first
/// directory that directly contains `marker`.
///
/// Returns the *directory*, not the marker path — callers need somewhere to
/// run a command, not just proof of existence.
///
/// Symlinks are not resolved and the walk is not bounded by a filesystem
/// boundary: `Path::parent` terminates at the root, so the loop is finite
/// regardless. Deliberately not bounded by "the nearest `.git`" either —
/// a flake or toolchain file above a repo root is unusual but is still the
/// one in effect, and inventing a stop condition here would reintroduce
/// exactly the "we did not look" class this module exists to remove.
#[must_use]
pub fn find_root_containing(start: &Path, marker: &str) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if cur.join(marker).is_file() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

/// The first ancestor directory containing ANY of `markers`, in the order
/// given at each level.
///
/// ── ★ WHY "ANY" IS A SEPARATE FUNCTION AND NOT A LOOP AT THE CALLER ────
/// A caller that walks once per marker asks the wrong question: it finds
/// the nearest ancestor with marker[0] even when a CLOSER ancestor has
/// marker[1]. Rust's toolchain pin is exactly this shape —
/// `rust-toolchain.toml` and the extension-less `rust-toolchain` are
/// alternatives at the same level — and the single-marker version silently
/// dropped the plain file when this module was first wired in. The
/// repo's own test caught it, which is why the case below exists.
///
/// Precedence is per LEVEL, not per marker: the nearest directory wins,
/// and only within one directory does `markers` order break the tie.
#[must_use]
pub fn find_root_containing_any(start: &Path, markers: &[&str]) -> Option<PathBuf> {
    let mut cur = start;
    loop {
        if markers.iter().any(|m| cur.join(m).is_file()) {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

/// `find_root_containing` reduced to a yes/no.
///
/// Provided so a caller that genuinely only needs detection does not
/// hand-roll a second walk, but prefer [`find_root_containing`] whenever
/// the answer feeds a command's working directory.
#[must_use]
pub fn has_ancestor_file(start: &Path, marker: &str) -> bool {
    find_root_containing(start, marker).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmpdir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("seki-project-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("temp dir");
        base
    }

    #[test]
    fn a_marker_in_the_starting_directory_is_found() {
        let root = tmpdir("here");
        fs::write(root.join("flake.nix"), "{}").unwrap();
        assert_eq!(find_root_containing(&root, "flake.nix"), Some(root));
    }

    /// The actual bug: a shell sitting three levels down inside a repo.
    /// Before the walk, this returned nothing and the segment went silent.
    #[test]
    fn a_marker_above_the_starting_directory_is_found() {
        let root = tmpdir("above");
        fs::write(root.join("flake.nix"), "{}").unwrap();
        let deep = root.join("modules/pleme/shared");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            find_root_containing(&deep, "flake.nix"),
            Some(root),
            "a subdirectory must resolve to the repo root, not to nothing"
        );
    }

    /// The negative control. Without it, a `find_root_containing` that
    /// always returned `Some(start)` would pass both cases above — the
    /// mutation that proves the search is a search.
    #[test]
    fn an_absent_marker_is_none_not_the_starting_directory() {
        let root = tmpdir("absent");
        let deep = root.join("a/b");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(
            find_root_containing(&deep, "definitely-not-here.toml"),
            None,
            "absence must be None; returning the cwd would let a caller \
             run a command in a directory that has no project"
        );
    }

    /// A directory named like the marker is not the marker. `is_file`
    /// rather than `exists`, so `flake.nix/` cannot satisfy the probe.
    #[test]
    fn a_directory_named_like_the_marker_does_not_count() {
        let root = tmpdir("dir");
        fs::create_dir_all(root.join("flake.nix")).unwrap();
        assert_eq!(find_root_containing(&root, "flake.nix"), None);
    }

    /// The regression that shipped for exactly one build: `rust` accepts
    /// either `rust-toolchain.toml` or an extension-less `rust-toolchain`,
    /// and searching for only the first made the plain file invisible.
    #[test]
    fn any_marker_matches_and_the_nearest_level_still_wins() {
        let root = tmpdir("any");
        fs::write(root.join("rust-toolchain"), "1.89.0\n").unwrap();
        let deep = root.join("crates/inner");
        fs::create_dir_all(&deep).unwrap();
        let markers = ["rust-toolchain.toml", "rust-toolchain"];
        assert_eq!(
            find_root_containing_any(&deep, &markers),
            Some(root.clone()),
            "the plain file must be found even though it is listed second"
        );
        // A CLOSER directory holding the second-listed marker beats a
        // farther one holding the first — the property a per-marker loop
        // at the caller gets wrong.
        fs::write(root.join("rust-toolchain.toml"), "[toolchain]\n").unwrap();
        let mid = root.join("crates");
        fs::write(mid.join("rust-toolchain"), "1.90.0\n").unwrap();
        assert_eq!(find_root_containing_any(&deep, &markers), Some(mid));
    }

    /// The NEAREST root wins — a nested flake shadows the outer one, which
    /// is what `nix` itself does.
    #[test]
    fn the_nearest_ancestor_wins() {
        let outer = tmpdir("nearest");
        fs::write(outer.join("flake.nix"), "{}").unwrap();
        let inner = outer.join("sub");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("flake.nix"), "{}").unwrap();
        let deep = inner.join("x");
        fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_root_containing(&deep, "flake.nix"), Some(inner));
    }
}
