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
        // M1: no auto-detect helpers yet. The discovered tier is
        // bare + (future) display-width-aware truncation_length,
        // terminal-theme-aware palette choice, IN_NIX_SHELL
        // pre-detection. Today returns bare so the contract still
        // distinguishes the four tiers structurally.
        Self::bare()
    }

    fn prescribed_default() -> Self {
        // The prescribed default is the emoji-forward "companion" prompt:
        // the cold Nord-frost base + a touch of Brazilian warmth, each
        // segment a friendly emoji, short-and-sweet (every segment
        // conditional, hostname ssh-only). The ❄ snowflake stays the
        // fleet signature. (vellum_config / blzsh_parity remain available
        // as alternate themes / the parity reference.)
        Self(companion_config::companion_config())
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
        // The prescribed default is now Vellum-themed but preserves
        // the blzsh-parity STRUCTURE (segments + order); only the
        // palette changed.
        let c = TieredSekiConfig::prescribed_default().0;
        // blzsh keeps these enabled
        assert!(c.character.enabled);
        assert!(c.git_branch.enabled);
        assert!(c.git_status.enabled);
        assert!(c.hostname.enabled);
        assert!(c.directory.enabled);
        assert!(c.cmd_duration.enabled);
        assert!(c.nix_shell.enabled);
        // blzsh keeps these disabled
        assert!(!c.rust.enabled);
        assert!(!c.kubernetes.enabled);
        assert!(!c.username.enabled);
        // blzsh order
        assert_eq!(c.prompt_order.first().map(String::as_str), Some("nix_shell"));
        assert_eq!(c.prompt_order.last().map(String::as_str), Some("character"));
    }

    #[test]
    fn discovered_returns_bare_in_m1() {
        assert_eq!(
            TieredSekiConfig::discovered(),
            TieredSekiConfig::bare(),
        );
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
