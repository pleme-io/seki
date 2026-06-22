//! The `discovered` tier — the companion default, *adapted to the
//! terminal and fleet context it actually finds itself in*.
//!
//! `prescribed_default()` is the fleet-perfect static default (the
//! companion prompt). `discovered()` starts from that and refines it
//! from what it can sniff out of the environment: are we in **mado**
//! (truecolor + emoji guaranteed), a **truecolor** terminal, a **nix
//! shell**, over **SSH**, on a **narrow** pane, or a **dumb/CI**
//! terminal that can't render any of it?
//!
//! Detection is a **pure function of an env lookup** (the TYPED-SPEC
//! Environment-trait discipline) so the whole adaptation matrix is
//! unit-testable without touching the real process environment.

use seki_core::SekiConfig;

use crate::companion_config::companion_config;

/// Terminal + fleet capabilities sniffed from the environment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Detected {
    /// 24-bit colour is safe (mado, or `COLORTERM=truecolor|24bit`).
    pub truecolor: bool,
    /// We're inside a mado session — richest rendering, no doubt.
    pub mado: bool,
    /// Inside a nix shell (`IN_NIX_SHELL`).
    pub nix_shell: bool,
    /// Connected over SSH — the host becomes load-bearing context.
    pub ssh: bool,
    /// Terminal width in columns, when the shell exported it.
    pub columns: Option<u16>,
    /// A dumb / CI / `NO_COLOR` terminal — degrade to a plain prompt.
    pub minimal: bool,
}

/// Sniff [`Detected`] from an env lookup. Pure: pass a closure over a
/// real or mock environment.
pub fn detect(env: impl Fn(&str) -> Option<String>) -> Detected {
    let get = |k: &str| env(k).filter(|v| !v.is_empty());
    let mado = get("MADO_SOCKET").is_some() || get("MADO_SESSION").is_some();
    let colorterm = get("COLORTERM").unwrap_or_default();
    let truecolor = mado || colorterm == "truecolor" || colorterm == "24bit";
    let term = get("TERM").unwrap_or_default();
    // `NO_COLOR` is an explicit operator opt-out → always plain. Otherwise
    // a terminal that proves it's rich (mado / truecolor) is never
    // minimal; a `dumb`/CI terminal that hasn't proven capability is.
    // A merely-absent TERM is NOT treated as dumb (safe rich default for
    // an interactive shell).
    let minimal =
        get("NO_COLOR").is_some() || (!mado && !truecolor && (term == "dumb" || get("CI").is_some()));
    Detected {
        truecolor,
        mado,
        nix_shell: get("IN_NIX_SHELL").is_some(),
        ssh: get("SSH_CONNECTION").is_some() || get("SSH_TTY").is_some(),
        columns: get("COLUMNS").and_then(|c| c.parse().ok()),
        minimal,
    }
}

/// Sniff from the real process environment — the entrypoint the
/// `TieredConfig::discovered()` impl calls.
#[must_use]
pub fn detect_from_env() -> Detected {
    detect(|k| std::env::var(k).ok())
}

/// Build the discovered config: the companion default adapted to
/// `d`, or a plain fallback when the terminal can't render the rich
/// prompt.
#[must_use]
pub fn discovered_config(d: &Detected) -> SekiConfig {
    if d.minimal {
        return minimal_config();
    }
    let mut c = companion_config();

    // Width-aware directory truncation — short on narrow panes (mado
    // splits, tear/tmux panes), longer when there's room.
    if let Some(cols) = d.columns {
        c.directory.truncation_length = if cols < 60 {
            1
        } else if cols < 100 {
            2
        } else if cols >= 200 {
            4
        } else {
            3
        };
    }

    // Over SSH the host is crucial context; companion already renders it
    // ssh-only, so it appears automatically — make that explicit so a
    // future base change can't silently drop it.
    if d.ssh {
        c.hostname.enabled = true;
        c.hostname.ssh_only = true;
    }

    c
}

/// A plain, robust prompt for dumb / CI / `NO_COLOR` terminals: path +
/// branch + a simple character, no emoji, minimal glyphs. Still a real,
/// working prompt — just one a basic terminal can render faithfully.
fn minimal_config() -> SekiConfig {
    let mut c = SekiConfig::bare();
    c.prompt_order = vec![
        "directory".to_owned(),
        "git_branch".to_owned(),
        "git_status".to_owned(),
        "character".to_owned(),
    ];
    c.directory.enabled = true;
    c.directory.format = "[$path]($style) ".to_owned();
    c.directory.truncation_length = 3;
    c.directory.home_symbol = "~".to_owned();

    c.git_branch.enabled = true;
    c.git_branch.format = "[$symbol$branch]($style) ".to_owned();
    c.git_branch.symbol = String::new();
    c.git_branch.truncation_length = u32::MAX;
    c.git_branch.truncation_symbol = "…".to_owned();

    // Coarse ASCII git status — no emoji.
    c.git_status.enabled = true;
    c.git_status.format = "[$all_status$ahead_behind]($style) ".to_owned();
    c.git_status.modified = "*".to_owned();
    c.git_status.staged = "+".to_owned();
    c.git_status.untracked = "?".to_owned();
    c.git_status.deleted = "x".to_owned();
    c.git_status.renamed = ">".to_owned();
    c.git_status.conflicted = "=".to_owned();
    c.git_status.stashed = "$".to_owned();
    c.git_status.ahead = "^${count}".to_owned();
    c.git_status.behind = "v${count}".to_owned();
    c.git_status.diverged = "^${ahead_count}v${behind_count}".to_owned();

    c.character.enabled = true;
    c.character.success_symbol = "$".to_owned();
    c.character.error_symbol = "!".to_owned();
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k: &str| {
            owned
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        }
    }

    #[test]
    fn mado_implies_truecolor_and_rich() {
        let d = detect(env_of(&[("MADO_SOCKET", "/tmp/mado.sock"), ("TERM", "mado")]));
        assert!(d.mado);
        assert!(d.truecolor);
        assert!(!d.minimal);
        // Rich, companion-based config (emoji-forward directory).
        let c = discovered_config(&d);
        assert!(c.directory.format.contains("📁"));
        assert!(c.rust.enabled, "companion rust segment carried through");
    }

    #[test]
    fn colorterm_truecolor_detected() {
        let d = detect(env_of(&[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]));
        assert!(d.truecolor);
        assert!(!d.minimal);
    }

    #[test]
    fn dumb_terminal_degrades_to_minimal() {
        let d = detect(env_of(&[("TERM", "dumb")]));
        assert!(d.minimal);
        let c = discovered_config(&d);
        // No emoji-forward symbols in the minimal prompt.
        assert!(!c.directory.format.contains("📁"));
        assert_eq!(c.git_status.modified, "*");
        assert!(c.prompt_order.contains(&"git_branch".to_owned()));
    }

    #[test]
    fn ci_and_no_color_are_minimal() {
        assert!(detect(env_of(&[("TERM", "xterm"), ("CI", "true")])).minimal);
        assert!(detect(env_of(&[("TERM", "xterm"), ("NO_COLOR", "1")])).minimal);
    }

    #[test]
    fn mado_overrides_ci_minimal() {
        // Inside mado we KNOW the terminal is rich even if CI is set.
        let d = detect(env_of(&[("MADO_SOCKET", "/x"), ("CI", "true"), ("TERM", "dumb")]));
        assert!(!d.minimal, "mado is always rich");
        assert!(d.truecolor);
    }

    #[test]
    fn width_tightens_truncation_on_narrow_panes() {
        let wide = discovered_config(&detect(env_of(&[("COLORTERM", "truecolor"), ("COLUMNS", "220")])));
        assert_eq!(wide.directory.truncation_length, 4);
        let narrow = discovered_config(&detect(env_of(&[("COLORTERM", "truecolor"), ("COLUMNS", "50")])));
        assert_eq!(narrow.directory.truncation_length, 1);
        let mid = discovered_config(&detect(env_of(&[("COLORTERM", "truecolor"), ("COLUMNS", "80")])));
        assert_eq!(mid.directory.truncation_length, 2);
    }

    #[test]
    fn ssh_keeps_host_visible() {
        let d = detect(env_of(&[("COLORTERM", "truecolor"), ("SSH_CONNECTION", "1.2.3.4 22 5.6.7.8 22")]));
        assert!(d.ssh);
        let c = discovered_config(&d);
        assert!(c.hostname.enabled && c.hostname.ssh_only);
    }
}
