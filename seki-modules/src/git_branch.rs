//! `git_branch` — current branch name, read from `.git/HEAD`.
//!
//! We walk the CWD up to filesystem root looking for a `.git`
//! directory. If found, we parse `.git/HEAD` — either a `ref:
//! refs/heads/<branch>` symbolic ref or a detached SHA. Detached
//! HEAD renders as the truncated SHA.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::git_branch::GitBranchConfig,
    segment::StyledFragment,
};
use std::fs;
use std::path::{Path, PathBuf};

pub struct GitBranchModule {
    cfg: GitBranchConfig,
}

impl GitBranchModule {
    pub fn new(cfg: GitBranchConfig) -> Self {
        Self { cfg }
    }
}

impl Module for GitBranchModule {
    fn name(&self) -> &'static str {
        "git_branch"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(git_dir) = find_git_dir(&ctx.cwd) else {
            return Ok(None);
        };
        let Some(branch) = read_head_branch(&git_dir) else {
            return Ok(None);
        };
        let truncated =
            truncate_branch(&branch, self.cfg.truncation_length as usize, &self.cfg.truncation_symbol);
        // Typed `format` is authoritative (e.g. companion's
        // " [$symbol$branch]"), rendered through the shared engine so
        // the leading space + 🌿 symbol land consistently.
        let text = seki_core::format::render(&self.cfg.format, |name| match name {
            "symbol" => Some(self.cfg.symbol.clone()),
            "branch" => Some(truncated.clone()),
            _ => None,
        });
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(Segment::new("git_branch").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

pub fn find_git_dir(start: &Path) -> Option<PathBuf> {
    let mut cur: &Path = start;
    loop {
        let candidate = cur.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        cur = cur.parent()?;
    }
}

pub fn read_head_branch(git_dir: &Path) -> Option<String> {
    let head_path = git_dir.join("HEAD");
    let head = fs::read_to_string(&head_path).ok()?;
    let head = head.trim();
    if let Some(rest) = head.strip_prefix("ref: refs/heads/") {
        return Some(rest.to_owned());
    }
    if head.len() >= 7 {
        return Some(head[..7].to_owned());
    }
    None
}

pub fn truncate_branch(branch: &str, max: usize, suffix: &str) -> String {
    if max == 0 || max == usize::MAX || branch.len() <= max {
        return branch.to_owned();
    }
    let mut s: String = branch.chars().take(max).collect();
    s.push_str(suffix);
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempdir_helper::tempdir;

    mod tempdir_helper {
        use std::path::PathBuf;
        pub struct Tmp(pub PathBuf);
        impl Drop for Tmp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        pub fn tempdir(tag: &str) -> Tmp {
            let mut p = std::env::temp_dir();
            p.push(format!("seki-test-{}-{}", tag, std::process::id()));
            std::fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
    }

    #[test]
    fn parses_ref_branch_from_head() {
        let dir = tempdir("gb-ref");
        let git_dir = dir.0.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        assert_eq!(read_head_branch(&git_dir).as_deref(), Some("main"));
    }

    #[test]
    fn parses_detached_head_as_short_sha() {
        let dir = tempdir("gb-det");
        let git_dir = dir.0.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(
            git_dir.join("HEAD"),
            "deadbeefcafe1234567890abcdef0123456789ab\n",
        )
        .unwrap();
        assert_eq!(read_head_branch(&git_dir).as_deref(), Some("deadbee"));
    }

    #[test]
    fn truncate_branch_respects_max() {
        assert_eq!(truncate_branch("main", 10, "…"), "main");
        assert_eq!(truncate_branch("feature/very-long-name", 4, "…"), "feat…");
    }
}
