//! `git_status` — minimum-viable dirty-tree detector.
//!
//! M1: parses `.git/index` presence + walks for an obvious dirty
//! marker file (`.git/MERGE_HEAD` for conflict; `.git/index.lock`
//! suggests recent activity). A full porcelain implementation lands
//! in M2 once we link a typed git crate. The reported status is a
//! coarse `Clean | Modified | Conflicted` enum, sufficient for the
//! prompt symbol.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::git_status::GitStatusConfig,
    segment::StyledFragment,
};
use std::path::Path;

use crate::git_branch::find_git_dir;

pub struct GitStatusModule {
    cfg: GitStatusConfig,
}

impl GitStatusModule {
    pub fn new(cfg: GitStatusConfig) -> Self {
        Self { cfg }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeStatus {
    Clean,
    Modified,
    Conflicted,
}

pub fn classify(git_dir: &Path) -> WorktreeStatus {
    if git_dir.join("MERGE_HEAD").exists() {
        return WorktreeStatus::Conflicted;
    }
    if git_dir.join("index.lock").exists() {
        return WorktreeStatus::Modified;
    }
    // M1 doesn't actually parse the index; we conservatively
    // report Clean. Real porcelain comes in M2.
    WorktreeStatus::Clean
}

impl Module for GitStatusModule {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(git_dir) = find_git_dir(&ctx.cwd) else {
            return Ok(None);
        };
        let status = classify(&git_dir);
        let symbol = match status {
            WorktreeStatus::Clean => &self.cfg.clean_symbol,
            WorktreeStatus::Modified => &self.cfg.modified,
            WorktreeStatus::Conflicted => &self.cfg.conflicted,
        };
        let mut text = String::new();
        text.push_str(&self.cfg.prefix);
        text.push_str(symbol);
        text.push_str(&self.cfg.suffix);
        Ok(Some(Segment::new("git_status").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_git(tag: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-gs-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join(".git")).unwrap();
        p
    }

    #[test]
    fn clean_when_no_markers() {
        let dir = make_git("clean");
        let git_dir = dir.join(".git");
        assert_eq!(classify(&git_dir), WorktreeStatus::Clean);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn conflicted_when_merge_head() {
        let dir = make_git("conf");
        let git_dir = dir.join(".git");
        fs::write(git_dir.join("MERGE_HEAD"), "x").unwrap();
        assert_eq!(classify(&git_dir), WorktreeStatus::Conflicted);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn modified_when_index_lock() {
        let dir = make_git("mod");
        let git_dir = dir.join(".git");
        fs::write(git_dir.join("index.lock"), "").unwrap();
        assert_eq!(classify(&git_dir), WorktreeStatus::Modified);
        let _ = fs::remove_dir_all(&dir);
    }
}
