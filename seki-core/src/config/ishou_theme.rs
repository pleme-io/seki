//! Typed config for the `ishou_theme` segment.
//!
//! Pleme-io-native (Tier 4 — substrate-themed): surfaces the
//! fleet-prescribed `ishou_tokens::FleetTheme` variant in the prompt.
//! Tells the operator at a glance which design-system theme tier the
//! fleet binaries are constructed against — `Bare` (zero-opinion
//! floor) vs `PlemeDark` (the canonical pleme-io look).
//!
//! Because `FleetDefaults::prescribed()` is a compile-time-known
//! factory, the answer is the same for every render in the lifetime
//! of the process — there is no runtime probe, no env-var read, no
//! filesystem walk. The renderer just stamps the variant name.
//!
//! # Theme
//!
//! Nord-snowstorm dim `#D8DEE9` — the snowstorm-foreground accent the
//! rest of the ishou design system uses to denote "this is a cool
//! fact about the design system itself". Distinguished from the
//! aurora-yellow used by `shikumi_tier` (override semantics) and the
//! frost-blue used by `caixa` (typed-repo identity).
//!
//! # Why a segment for one constant?
//!
//! The fleet currently has exactly two `FleetTheme` variants
//! (`Bare`, `PlemeDark`) — but operators flip between them via the
//! ishou roll-out toggles (`ishou apply --tier bare`). When that
//! flip happens, every fleet binary recompiled against the new
//! prescription picks up the new theme on next launch; this segment
//! is the operator's confirmation that the recompile took.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IshouThemeConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$theme` — resolved FleetTheme variant name (`Bare` /
    ///   `PlemeDark`)
    ///
    /// Starship-style `[…]($style)` markup is stripped; the
    /// renderer applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Defaults to Nord-snowstorm
    /// dim `#D8DEE9` — the "fact about the design system" accent.
    pub style: StyleSpec,
}

impl Default for IshouThemeConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: true,
            format: "[ishou: $theme]($style)".to_owned(),
            style: StyleSpec::new(NordPalette::bold(&nord.snow_dim)),
        }
    }
}

impl IshouThemeConfig {
    /// Zero-opinion: nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
        }
    }
}

/// Pull the prescribed FleetTheme variant name. Exposed for
/// testability; the module impl is a thin wrapper.
///
/// Pulled out as a free function (per brief) so we touch only the
/// theme field, not the full ~150-byte `FleetDefaults` struct at
/// module-construction time.
pub fn prescribed_theme_name() -> &'static str {
    use ishou_tokens::FleetTheme;
    match ishou_tokens::FleetDefaults::prescribed().theme {
        FleetTheme::Bare => "Bare",
        FleetTheme::PlemeDark => "PlemeDark",
        FleetTheme::Vellum => "Vellum",
        FleetTheme::PolarVeil => "PolarVeil",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_enabled() {
        // Tier 4 default is `enabled = true` per the brief — cheap
        // (compile-time const), no probe, always-on surface.
        let cfg = IshouThemeConfig::default();
        assert!(cfg.enabled);
    }

    #[test]
    fn default_uses_nord_snowstorm_dim() {
        let cfg = IshouThemeConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #D8DEE9");
    }

    #[test]
    fn default_format_renders_ishou_prefix() {
        let cfg = IshouThemeConfig::default();
        assert!(cfg.format.contains("ishou:"));
        assert!(cfg.format.contains("$theme"));
    }

    #[test]
    fn bare_is_disabled() {
        let cfg = IshouThemeConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.format, "");
    }

    #[test]
    fn prescribed_theme_name_is_vellum() {
        // FleetDefaults::prescribed().theme is canonically
        // Vellum (the prescribed fleet theme) — this is the
        // load-bearing assertion that proves the ishou-tokens dep is
        // actually wired through compile-time.
        assert_eq!(prescribed_theme_name(), "Vellum");
    }
}
