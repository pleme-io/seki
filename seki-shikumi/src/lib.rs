//! seki-shikumi — `TieredConfig` impl for [`seki_core::SekiConfig`].
//!
//! Implements the fleet-wide configuration prime directive: every
//! operator-facing typed config implements [`TieredConfig`] so
//! operators get `<app> config-show <tier>` + the `SEKI_TIER`
//! env-var override for free.
//!
//! - `bare()` → [`seki_core::SekiConfig::bare`] (zero opinions)
//! - `discovered()` → default-of-`bare` overlaid with auto-detect
//!   (M1: detect-helpers TBD; today returns `bare`)
//! - `prescribed_default()` → [`blzsh_parity::blzsh_parity_config`]
//!   (the M3 deliverable — exact match to blzsh's starship.toml)
//! - `extend(base)` → full replacement (default trait impl).

use seki_core::SekiConfig;
use shikumi::TieredConfig;

pub mod blzsh_parity;
pub mod companion_config;
pub mod discovered;
pub mod vellum;
pub mod vellum_config;

/// Newtype wrapper so we can `impl TieredConfig` from this crate
/// without touching seki-core. The renderer accepts a borrowed
/// `&SekiConfig`; consumers convert via `From<TieredSekiConfig>`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct TieredSekiConfig(pub SekiConfig);

impl From<SekiConfig> for TieredSekiConfig {
    fn from(value: SekiConfig) -> Self {
        Self(value)
    }
}

impl From<TieredSekiConfig> for SekiConfig {
    fn from(value: TieredSekiConfig) -> Self {
        value.0
    }
}

impl TieredConfig for TieredSekiConfig {
    fn bare() -> Self {
        Self(SekiConfig::bare())
    }

    fn discovered() -> Self {
        // The companion default, adapted to the terminal + fleet context
        // it finds itself in: mado / truecolor / nix-shell / SSH / pane
        // width / dumb-or-CI. See `discovered::detect` (pure, testable).
        Self(discovered::discovered_config(&discovered::detect_from_env()))
    }

    fn prescribed_default() -> Self {
        // The prescribed fleet default = best of the Nord + pleme-io
        // worlds, built on the blzsh-parity base:
        //
        //   * Nord-frost ❄ snowflake character + palette, and the
        //     substrate segments blzsh-parity already surfaces
        //     (workspace / tear-session / tear-pane / caixa kind /
        //     tend status / shikumi tier).
        //   * the Rust-dominant fleet's toolchain version, re-enabled
        //     here with the SINGLE-WIDTH `SignalMode::Glyph` Nerd-font
        //     glyph (`⊿`), never the double-width `🦀` emoji.
        //
        // STANDARDIZATION INVARIANT — every glyph the prompt can render
        // is single-width and lives in the operator's `Symbols Nerd Font
        // Mono`, so it sits cleanly in the monospace cell grid. This is
        // deliberately NOT the emoji-forward `companion_config`: its
        // color-emoji symbols (🌊🔖📁🌿🦀) are absent from the Nerd-font
        // stack, so terminals fall back to a color-emoji font whose glyph
        // metrics don't align to the fixed-width prompt and the cursor
        // visually drifts. The `prescribed_default_is_grid_aligned` test
        // is the forcing function that keeps any width-2 glyph out of the
        // default. (`blzsh_parity_config` stays the pure parity reference;
        // `companion_config` / `vellum_config` remain opt-in themes.)
        let mut cfg = blzsh_parity::blzsh_parity_config();

        const SIG: ishou_tokens::SekiSignals = ishou_tokens::SekiSignals::prescribed();
        let mut rust_symbol = SIG.lang_rust.render(ishou_tokens::SignalMode::Glyph).to_owned();
        rust_symbol.push(' ');
        cfg.rust = seki_core::config::rust::RustConfig {
            enabled: true,
            symbol: rust_symbol,
            ..seki_core::config::rust::RustConfig::default()
        };
        // Surface the rust segment just before the ❄ character anchor.
        if let Some(pos) = cfg.prompt_order.iter().position(|s| s == "character") {
            cfg.prompt_order.insert(pos, "rust".to_owned());
        }

        Self(cfg)
    }
}

/// Convenience: load the config at the tier resolved from the
/// `SEKI_TIER` env var (defaulting to `Default`).
pub fn load_from_env() -> SekiConfig {
    TieredSekiConfig::resolve_from_env("SEKI_TIER").0
}

#[cfg(test)]
mod tests {
    use super::*;
    use shikumi::ConfigTier;

    #[test]
    fn bare_has_empty_prompt_order() {
        let c = TieredSekiConfig::bare().0;
        assert!(c.prompt_order.is_empty());
        assert!(!c.directory.enabled);
        assert!(!c.git_branch.enabled);
    }

    #[test]
    fn prescribed_default_keeps_blzsh_structure() {
        // The prescribed fleet default builds on the blzsh-parity base:
        // same Nord-frost STRUCTURE (segments + order), plus the
        // single-width rust toolchain segment (best of Nord + pleme-io).
        let c = TieredSekiConfig::prescribed_default().0;
        // blzsh keeps these enabled
        assert!(c.character.enabled);
        assert!(c.git_branch.enabled);
        assert!(c.git_status.enabled);
        assert!(c.hostname.enabled);
        assert!(c.directory.enabled);
        assert!(c.cmd_duration.enabled);
        assert!(c.nix_shell.enabled);
        // pleme-io touch: the Rust-dominant fleet's toolchain segment is
        // re-enabled (silent outside Cargo repos) with a single-width
        // glyph — never the double-width 🦀 emoji.
        assert!(c.rust.enabled, "fleet default enables the rust segment");
        assert!(c.prompt_order.iter().any(|s| s == "rust"));
        assert!(
            !c.rust.symbol.contains('🦀'),
            "rust segment must use the single-width glyph, not the 🦀 emoji",
        );
        // still disabled
        assert!(!c.kubernetes.enabled);
        assert!(!c.username.enabled);
        // order anchors preserved: nix_shell first, ❄ character last,
        // rust spliced in just before the character anchor.
        assert_eq!(c.prompt_order.first().map(String::as_str), Some("nix_shell"));
        assert_eq!(c.prompt_order.last().map(String::as_str), Some("character"));
        let rust_pos = c.prompt_order.iter().position(|s| s == "rust").unwrap();
        let char_pos = c.prompt_order.iter().position(|s| s == "character").unwrap();
        assert_eq!(rust_pos + 1, char_pos, "rust sits immediately before ❄");
    }

    /// STANDARDIZATION FORCING FUNCTION — the prompt-misalignment class
    /// made unrepresentable.
    ///
    /// Every glyph the prescribed default prompt can render must be
    /// single-width (display width ≤ 1) so it sits cleanly in the
    /// terminal's fixed-width cell grid. Double-width emoji (🌊🔖📁🌿🦀,
    /// width 2) fall back to a color-emoji font whose metrics don't align
    /// to the monospace grid → the cursor visually drifts (the exact bug
    /// the emoji-forward `companion_config` shipped). Serializing the
    /// whole config catches a stray emoji in ANY segment's symbol/format
    /// field — enabled or not — so flipping a segment on later can never
    /// reintroduce the drift. Alternate themes (companion / vellum) may
    /// opt into emoji; the FLEET DEFAULT may not.
    #[test]
    fn prescribed_default_is_grid_aligned() {
        use unicode_width::UnicodeWidthChar;

        let cfg = TieredSekiConfig::prescribed_default();
        let serialized =
            serde_yaml::to_string(&cfg).expect("prescribed default serializes");

        let offenders: Vec<(char, usize)> = serialized
            .chars()
            .filter(|c| UnicodeWidthChar::width(*c).unwrap_or(0) > 1)
            .map(|c| (c, UnicodeWidthChar::width(c).unwrap_or(0)))
            .collect();

        assert!(
            offenders.is_empty(),
            "fleet default prompt config contains {} double-width glyph(s) that \
             will misalign the cursor — replace with single-width SignalMode::Glyph \
             forms: {:?}",
            offenders.len(),
            offenders
                .iter()
                .map(|(c, w)| format!("U+{:04X} (width {w})", *c as u32))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn discovered_always_yields_a_working_prompt() {
        // discovered() reads the real env; regardless of what it finds
        // (rich companion, width-adapted, or minimal fallback) it always
        // produces a non-empty, renderable prompt — never the empty
        // bare floor. The detection matrix itself is unit-tested in
        // `discovered::tests`.
        let c = TieredSekiConfig::discovered().0;
        assert!(!c.prompt_order.is_empty());
        assert!(c.character.enabled);
    }

    #[test]
    fn resolve_tier_dispatch() {
        assert_eq!(
            TieredSekiConfig::resolve_tier(ConfigTier::Bare),
            TieredSekiConfig::bare(),
        );
        assert_eq!(
            TieredSekiConfig::resolve_tier(ConfigTier::Default),
            TieredSekiConfig::prescribed_default(),
        );
    }

    #[test]
    fn diff_bare_vs_default_is_non_empty() {
        let bare = TieredSekiConfig::bare();
        let default = TieredSekiConfig::prescribed_default();
        let diff = default.diff_against(&bare);
        assert!(!diff.is_empty_diff());
    }
}
