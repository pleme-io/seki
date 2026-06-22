//! `git_status` — working-tree status as emoji/glyph indicators.
//!
//! Reads the porcelain-v2 status (`git status --porcelain=v2 --branch`)
//! in a single bounded `git` invocation and counts the staged /
//! modified / deleted / renamed / untracked / conflicted entries plus
//! the ahead/behind distance from the `# branch.ab` header. Stash depth
//! is read fork-free from `.git/logs/refs/stash`. The configured symbol
//! for each non-zero category is emitted (with `${count}` substituted),
//! in starship's `$all_status` order, then `$ahead_behind`, rendered
//! through the shared [`seki_core::format`] engine so the typed `format`
//! field is the single source of truth.
//!
//! A fully-clean, up-to-date tree renders nothing (the segment returns
//! `None`) unless a `clean_symbol` is configured — the companion
//! prompt's "conditional, short and sweet" contract.
//!
//! Performance: one `git` fork, hard-bounded by [`STATUS_TIMEOUT`] so a
//! pathological repo can never hang the prompt; the common path is a
//! few milliseconds. (A fork-free `gix` reader is the documented future
//! optimisation; it carries a heavy dependency + a still-maturing status
//! API, so the bounded single fork is the shipped destination.)

use seki_core::{
    Module, RenderContext, Segment, SekiResult, config::git_status::GitStatusConfig,
    segment::StyledFragment,
};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::git_branch::find_git_dir;

/// Hard ceiling on the `git status` fork. A prompt must never block; on
/// a repo slow enough to exceed this the segment renders nothing rather
/// than stall the operator.
const STATUS_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct GitStatusModule {
    cfg: GitStatusConfig,
}

impl GitStatusModule {
    pub fn new(cfg: GitStatusConfig) -> Self {
        Self { cfg }
    }
}

/// Typed working-tree status — one count per category.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GitStatus {
    pub staged: u32,
    pub modified: u32,
    pub deleted: u32,
    pub renamed: u32,
    pub untracked: u32,
    pub conflicted: u32,
    pub stashed: u32,
    pub ahead: u32,
    pub behind: u32,
}

impl GitStatus {
    /// Nothing to show: a pristine, up-to-date, stash-free tree.
    pub fn is_clean(&self) -> bool {
        self.staged == 0
            && self.modified == 0
            && self.deleted == 0
            && self.renamed == 0
            && self.untracked == 0
            && self.conflicted == 0
            && self.stashed == 0
            && self.ahead == 0
            && self.behind == 0
    }

    /// The nine counts as a single space-separated line — the daemon
    /// wire format. Dependency-free + typed (no `format!()`): each count
    /// is a typed `u32` rendered via its `Display`.
    #[must_use]
    pub fn to_wire(&self) -> String {
        let mut out = String::new();
        for n in [
            self.staged,
            self.modified,
            self.deleted,
            self.renamed,
            self.untracked,
            self.conflicted,
            self.stashed,
            self.ahead,
            self.behind,
        ] {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&n.to_string());
        }
        out
    }

    /// Parse the wire form. `None` unless exactly nine `u32` fields are
    /// present (a malformed line is rejected, never silently truncated).
    #[must_use]
    pub fn from_wire(line: &str) -> Option<GitStatus> {
        let nums: Vec<u32> = line
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        if nums.len() != 9 {
            return None;
        }
        Some(GitStatus {
            staged: nums[0],
            modified: nums[1],
            deleted: nums[2],
            renamed: nums[3],
            untracked: nums[4],
            conflicted: nums[5],
            stashed: nums[6],
            ahead: nums[7],
            behind: nums[8],
        })
    }
}

impl Module for GitStatusModule {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // Freshness first: prefer the FS-watch-fed hot daemon cache
        // (instant, never stale — see `fetch_status`), falling back to an
        // always-fresh live `git` fork. Either way the data is current at
        // render time; staleness is unrepresentable.
        let Some(status) = fetch_status(&ctx.cwd) else {
            return Ok(None);
        };
        let body = self.render_body(&status);
        if body.is_empty() {
            return Ok(None);
        }
        Ok(Some(Segment::new("git_status").push(StyledFragment::new(
            body,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Resolve the working-tree status for `cwd`, preferring the hot
/// [refresh daemon](crate::git_status::run_status_daemon) cache and
/// falling back to a live, always-fresh computation.
///
/// The daemon keeps each watched repo's status hot via OS filesystem
/// events (`notify`), so a cache hit is both instant **and** current —
/// the never-stale guarantee. A cache miss (no daemon running) computes
/// live, which is equally fresh, just one `git` fork slower. The result
/// is identical either way; only the cost differs.
#[must_use]
pub fn fetch_status(cwd: &Path) -> Option<GitStatus> {
    #[cfg(unix)]
    {
        if let Some(status) = query_daemon(cwd) {
            return Some(status);
        }
    }
    compute_status(cwd)
}

/// Compute working-tree status live: one bounded `git status` fork +
/// fork-free stash count. Always fresh. `None` when `cwd` is not a repo
/// or `git` errored/timed out.
#[must_use]
pub fn compute_status(cwd: &Path) -> Option<GitStatus> {
    let git_dir = find_git_dir(cwd)?;
    let raw = run_git_status(cwd)?;
    let mut status = parse_porcelain_v2(&raw);
    status.stashed = count_stash(&git_dir);
    Some(status)
}

impl GitStatusModule {
    /// Build `$all_status` + `$ahead_behind` from the counts and render
    /// the typed `format` through the shared engine. Returns an empty
    /// string when there is nothing to show.
    fn render_body(&self, st: &GitStatus) -> String {
        if st.is_clean() {
            if self.cfg.clean_symbol.is_empty() {
                return String::new();
            }
            let clean = self.cfg.clean_symbol.clone();
            return seki_core::format::render(&self.cfg.format, |name| match name {
                "all_status" => Some(clean.clone()),
                "ahead_behind" => Some(String::new()),
                _ => None,
            });
        }

        let all_status = build_all_status(&self.cfg, st);
        let ahead_behind = build_ahead_behind(&self.cfg, st);
        seki_core::format::render(&self.cfg.format, |name| match name {
            "all_status" => Some(all_status.clone()),
            "ahead_behind" => Some(ahead_behind.clone()),
            "conflicted" => Some(present(&self.cfg.conflicted, st.conflicted)),
            "stashed" => Some(present(&self.cfg.stashed, st.stashed)),
            "deleted" => Some(present(&self.cfg.deleted, st.deleted)),
            "renamed" => Some(present(&self.cfg.renamed, st.renamed)),
            "modified" => Some(present(&self.cfg.modified, st.modified)),
            "staged" => Some(present(&self.cfg.staged, st.staged)),
            "untracked" => Some(present(&self.cfg.untracked, st.untracked)),
            _ => None,
        })
    }
}

/// Concatenate the configured symbols for every non-zero category, in
/// starship's `$all_status` order.
fn build_all_status(cfg: &GitStatusConfig, st: &GitStatus) -> String {
    let mut out = String::new();
    for (count, symbol) in [
        (st.conflicted, &cfg.conflicted),
        (st.stashed, &cfg.stashed),
        (st.deleted, &cfg.deleted),
        (st.renamed, &cfg.renamed),
        (st.modified, &cfg.modified),
        (st.staged, &cfg.staged),
        (st.untracked, &cfg.untracked),
    ] {
        out.push_str(&present(symbol, count));
    }
    out
}

/// Resolve the `$ahead_behind` token: `diverged` when both, else the
/// one-sided glyph, else `up_to_date`.
fn build_ahead_behind(cfg: &GitStatusConfig, st: &GitStatus) -> String {
    if st.ahead > 0 && st.behind > 0 {
        cfg.diverged
            .replace("${ahead_count}", &st.ahead.to_string())
            .replace("${behind_count}", &st.behind.to_string())
    } else if st.ahead > 0 {
        cfg.ahead.replace("${count}", &st.ahead.to_string())
    } else if st.behind > 0 {
        cfg.behind.replace("${count}", &st.behind.to_string())
    } else {
        cfg.up_to_date.clone()
    }
}

/// A category symbol when its count is non-zero (with `${count}`
/// substituted), or empty otherwise. Symbols are otherwise literal —
/// only `${count}` is interpreted, so a `$`/`[`/`(` inside a symbol is
/// never mis-parsed.
fn present(symbol: &str, count: u32) -> String {
    if count == 0 || symbol.is_empty() {
        return String::new();
    }
    symbol.replace("${count}", &count.to_string())
}

/// Parse `git status --porcelain=v2 --branch` output into counts.
///
/// Each changed entry's `<XY>` field has X = the staged (index) side
/// and Y = the worktree side, each one of `.MTADRC`. We count both
/// sides: a staged-and-then-re-modified file lights both indicators,
/// which is the honest signal.
pub fn parse_porcelain_v2(out: &str) -> GitStatus {
    let mut st = GitStatus::default();
    for line in out.lines() {
        let mut fields = line.split(' ');
        match fields.next() {
            Some("#") => {
                // Header. We only care about `# branch.ab +A -B`.
                if fields.next() == Some("branch.ab") {
                    for tok in fields {
                        if let Some(a) = tok.strip_prefix('+') {
                            st.ahead = a.parse().unwrap_or(0);
                        } else if let Some(b) = tok.strip_prefix('-') {
                            st.behind = b.parse().unwrap_or(0);
                        }
                    }
                }
            }
            Some("?") => st.untracked += 1,
            Some("u") => st.conflicted += 1,
            Some("1") | Some("2") => {
                if let Some(xy) = fields.next() {
                    let mut cs = xy.chars();
                    let x = cs.next().unwrap_or('.');
                    let y = cs.next().unwrap_or('.');
                    if x != '.' {
                        st.staged += 1;
                    }
                    if x == 'D' {
                        st.deleted += 1;
                    }
                    if x == 'R' || x == 'C' {
                        st.renamed += 1;
                    }
                    match y {
                        'M' | 'T' => st.modified += 1,
                        'D' => st.deleted += 1,
                        'R' | 'C' => st.renamed += 1,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    st
}

/// Count stash entries fork-free from the stash reflog. Absent file →
/// zero (the common case — most repos have no stash).
pub fn count_stash(git_dir: &Path) -> u32 {
    let path = git_dir.join("logs/refs/stash");
    match std::fs::read_to_string(&path) {
        Ok(contents) => contents.lines().filter(|l| !l.trim().is_empty()).count() as u32,
        Err(_) => 0,
    }
}

/// Run `git status --porcelain=v2 --branch` in `cwd`, returning its
/// stdout. Bounded by [`STATUS_TIMEOUT`]: the child is read on a worker
/// thread (so a large status can't deadlock the pipe) and killed if it
/// overruns. `None` on spawn failure, non-zero exit, or timeout.
fn run_git_status(cwd: &Path) -> Option<String> {
    let mut child = Command::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        use std::io::Read;
        let _ = stdout.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(STATUS_TIMEOUT) {
        Ok(buf) => {
            // Reap; treat a non-zero exit (e.g. not-a-repo) as no status.
            match child.wait() {
                Ok(s) if s.success() => Some(buf),
                _ => None,
            }
        }
        Err(_) => {
            // Timed out — kill and show nothing.
            let _ = child.kill();
            let _ = child.wait();
            None
        }
    }
}

// ── Refresh daemon: socket address + client ───────────────────────────
//
// The daemon itself (the `notify` FS-watcher + hot cache + server loop)
// lives in the `seki` binary crate so the heavy `notify` dependency
// stays out of the library. The library owns the *contract*: where the
// socket lives, the per-repo cache key, and the lightweight client a
// prompt render uses to ask for a hot status.

/// The refresh daemon's unix-socket address. Honors `SEKI_STATUSD_SOCK`
/// for an explicit override; otherwise `$XDG_RUNTIME_DIR/seki/statusd.sock`
/// (per-user by construction), falling back to a per-user dir under the
/// system temp dir.
#[must_use]
pub fn socket_path() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(explicit) = std::env::var_os("SEKI_STATUSD_SOCK") {
        return PathBuf::from(explicit);
    }
    if let Some(xdg) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(xdg).join("seki").join("statusd.sock");
    }
    let mut dir = std::env::temp_dir();
    let mut name = String::from("seki-statusd-");
    name.push_str(&std::env::var("USER").unwrap_or_else(|_| "anon".to_owned()));
    dir.push(name);
    dir.join("statusd.sock")
}

/// The repository root (worktree top) containing `cwd`, or `None` if not
/// inside a repo. This is the daemon's per-repo cache key — so two
/// subdirectories of the same repo share one hot, FS-watched entry.
#[must_use]
pub fn repo_root(cwd: &Path) -> Option<std::path::PathBuf> {
    find_git_dir(cwd).and_then(|g| g.parent().map(Path::to_path_buf))
}

/// Ask a running daemon for `cwd`'s hot status. `None` (fast) when no
/// daemon is listening — the caller then computes live.
#[cfg(unix)]
fn query_daemon(cwd: &Path) -> Option<GitStatus> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let mut stream = UnixStream::connect(socket_path()).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(200)))
        .ok()?;
    let mut req = cwd.to_string_lossy().into_owned();
    req.push('\n');
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    GitStatus::from_wire(resp.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use seki_core::config::git_status::GitStatusConfig;

    fn companion_cfg() -> GitStatusConfig {
        // The emoji-forward companion symbols + the blzsh format.
        GitStatusConfig {
            format: "[$all_status$ahead_behind]($style) ".to_owned(),
            modified: "🟡".to_owned(),
            staged: "🟢".to_owned(),
            untracked: "⚪".to_owned(),
            deleted: "🔴".to_owned(),
            renamed: "🔁".to_owned(),
            conflicted: "💥".to_owned(),
            stashed: "📦".to_owned(),
            ahead: "⇡${count}".to_owned(),
            behind: "⇣${count}".to_owned(),
            diverged: "⇕${ahead_count}⇣${behind_count}".to_owned(),
            up_to_date: String::new(),
            clean_symbol: String::new(),
            ..GitStatusConfig::default()
        }
    }

    #[test]
    fn parses_ahead_behind() {
        let st = parse_porcelain_v2("# branch.ab +3 -2\n");
        assert_eq!(st.ahead, 3);
        assert_eq!(st.behind, 2);
    }

    #[test]
    fn counts_modified_staged_untracked() {
        let out = "\
# branch.ab +0 -0
1 .M N... 100644 100644 100644 aaa bbb src/lib.rs
1 M. N... 100644 100644 100644 ccc ddd Cargo.toml
? notes.txt
? scratch.rs
";
        let st = parse_porcelain_v2(out);
        assert_eq!(st.modified, 1, "one worktree-modified");
        assert_eq!(st.staged, 1, "one index-staged");
        assert_eq!(st.untracked, 2, "two untracked");
        assert_eq!(st.conflicted, 0);
    }

    #[test]
    fn counts_conflicts_renames_deletes() {
        let out = "\
u UU N... 1 2 3 h1 h2 h3 both.rs
2 R. N... 100644 100644 100644 e f R100 new.rs\told.rs
1 .D N... 100644 100644 000000 g h gone.rs
";
        let st = parse_porcelain_v2(out);
        assert_eq!(st.conflicted, 1);
        assert_eq!(st.renamed, 1);
        assert_eq!(st.deleted, 1);
        assert_eq!(st.staged, 1, "the rename is staged (X=R)");
    }

    #[test]
    fn clean_tree_renders_nothing() {
        let m = GitStatusModule::new(companion_cfg());
        assert_eq!(m.render_body(&GitStatus::default()), "");
    }

    #[test]
    fn dirty_tree_renders_emoji_cluster() {
        let m = GitStatusModule::new(companion_cfg());
        let st = GitStatus {
            modified: 2,
            staged: 1,
            untracked: 4,
            ahead: 1,
            ..GitStatus::default()
        };
        // order: conflicted,stashed,deleted,renamed,modified,staged,untracked + ahead_behind
        assert_eq!(m.render_body(&st), "🟡🟢⚪⇡1 ");
    }

    #[test]
    fn diverged_uses_both_counts() {
        let m = GitStatusModule::new(companion_cfg());
        let st = GitStatus {
            ahead: 2,
            behind: 3,
            ..GitStatus::default()
        };
        assert_eq!(m.render_body(&st), "⇕2⇣3 ");
    }

    #[test]
    fn clean_symbol_renders_when_configured() {
        let mut cfg = companion_cfg();
        cfg.clean_symbol = "✓".to_owned();
        let m = GitStatusModule::new(cfg);
        assert_eq!(m.render_body(&GitStatus::default()), "✓ ");
    }

    #[test]
    fn wire_roundtrips() {
        let st = GitStatus {
            staged: 1,
            modified: 2,
            deleted: 3,
            renamed: 4,
            untracked: 5,
            conflicted: 6,
            stashed: 7,
            ahead: 8,
            behind: 9,
        };
        assert_eq!(GitStatus::from_wire(&st.to_wire()), Some(st));
        assert_eq!(GitStatus::default().to_wire(), "0 0 0 0 0 0 0 0 0");
    }

    #[test]
    fn wire_rejects_malformed() {
        assert_eq!(GitStatus::from_wire("1 2 3"), None);
        assert_eq!(GitStatus::from_wire("not numbers"), None);
        assert_eq!(GitStatus::from_wire(""), None);
        assert_eq!(GitStatus::from_wire("1 2 3 4 5 6 7 8 9 10"), None);
    }
}
