//! Typed config for the `tear` segment.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the current tear (terminal multiplexer) session + pane identity
//! when the shell is running inside a tear pane. Tells the operator
//! at a glance which session/pane they're driving — load-bearing
//! when a single operator drives multiple parallel sessions on the
//! same workstation.
//!
//! # Theme
//!
//! Nord-frost cyan `#88C0D0` — pleme-io's load-bearing frost colour,
//! shared with the snowflake glyph + `nix_shell` segment.
//!
//! # Probe budget
//!
//! Env-var read only (`TEAR_SESSION_NAME` + `TEAR_PANE_ID`). Bypasses
//! `scan_timeout_ms` per the doc's env-var exemption.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the tear prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TearConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$session` — `TEAR_SESSION_NAME` env var
    /// - `$pane` — first `pane_id_len` chars of `TEAR_PANE_ID`
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Number of leading chars of `TEAR_PANE_ID` to surface. Mirrors
    /// the blzsh-parity convention (`${TEAR_PANE_ID:0:6}` in the
    /// reference starship.toml's `[custom.tear_pane]`).
    pub pane_id_len: usize,
    /// Style applied to the rendered text. Nord-frost cyan `#88C0D0`.
    pub style: StyleSpec,
}

impl Default for TearConfig {
    fn default() -> Self {
        Self {
            // Default-OFF per docs/PLEME-IO-SEGMENTS.md M3 Tier 2:
            // every Tier 2 segment except shikumi_config opts out by
            // default. Operators flip `enabled = true` once their
            // fleet posture makes the segment meaningful.
            enabled: false,
            // `⧉` (squared-overlap tile), NOT a leading `~`. A literal
            // `~` first-in-prompt reads as a home-directory path
            // segment — it directly caused the 2026-06-11 misdiagnosis
            // where `~ mado-…` in the prompt was mistaken for a `cd`
            // having silently moved the shell to `$HOME`. The glyph is
            // unambiguously a session/pane tile, never a path. Guarded
            // by `default_format_is_not_path_like`.
            format: "[⧉ $session] [pane $pane]($style)".to_owned(),
            pane_id_len: 6,
            style: StyleSpec::new("bold #88C0D0"),
        }
    }
}

impl TearConfig {
    /// Zero-opinion: nothing read, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            pane_id_len: 0,
            style: StyleSpec::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tear_segment_default_format_is_not_path_like() {
        // Regression for the 2026-06-11 misdiagnosis: the tear
        // segment's leading icon must NOT be a path-like character.
        // A literal `~` (or `/`, or `./`) first-in-prompt reads as a
        // directory segment — the operator saw `~ mado-…` and
        // concluded the shell had silently `cd`'d to `$HOME`. The
        // icon is a session/pane tile glyph, never a path token.
        let fmt = TearConfig::default().format;

        // The first non-markup char is the icon. Markup chars `[` `]`
        // `(` and whitespace are stripped/ignored by the renderer.
        let icon = fmt
            .chars()
            .find(|c| !matches!(c, '[' | ']' | '(' | ' '))
            .expect("default format has a visible icon");

        const PATH_LIKE: &[char] = &['~', '/', '.', '$'];
        assert!(
            !PATH_LIKE.contains(&icon),
            "tear segment icon {icon:?} reads as a path token in `{fmt}` — \
             use a non-path glyph (e.g. ⧉) so the prompt can't be mistaken for a cwd"
        );

        // The substitutions must survive the icon change — a format
        // that lost `$session` would render a bare glyph forever.
        assert!(
            fmt.contains("$session"),
            "default format must still substitute the session name"
        );
    }
}
