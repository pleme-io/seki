//! Classic-Nord prompt palette — hexes resolved from ishou.
//!
//! The seki-core sibling of seki-shikumi's [`crate`]-external
//! `vellum::PromptPalette`: where that adapter resolves the warm Vellum
//! matte accents, this one resolves the CLASSIC Nord accents the
//! pleme-io-native segment defaults paint with. Colors are BORN in ishou
//! (`ishou_tokens::ColorPalette::pleme()`, itself sourced from
//! `irodori::NORD`); this module resolves each named accent to its
//! `#RRGGBB` string ONCE, so a segment's `StyleSpec` is built from a
//! token reference, never a hand-authored hex. A Nord token edit
//! upstream flows here on the next compile.
//!
//! Per the pleme-io law: NEVER hand-author a hex in a downstream repo.
//! Every accent the pleme-io-native segments paint is one of these
//! named accessors.

use ishou_tokens::ColorPalette;

/// The classic-Nord accents the pleme-io-native segment defaults use,
/// each resolved from a Nord token. Maps the recurring segment accents
/// (`success`/`error`/`warn`/dim/identity) onto named accessors so a
/// segment's `Default` reads as intent, not as hex.
pub struct NordPalette {
    /// `aurora_green` — readiness / clean / success.
    pub aurora_green: String,
    /// `aurora_red` — error / degraded / drift / cold-cache.
    pub aurora_red: String,
    /// `aurora_yellow` — warn / lukewarm / light-load.
    pub aurora_yellow: String,
    /// `aurora_orange` — busy / other-state.
    pub aurora_orange: String,
    /// `frost_1` (`#88C0D0`) — typed-repo identity (caixa / tear / vm).
    pub frost_cyan: String,
    /// `frost_2` (`#81A1C1`) — session / agent-ish steel accents.
    pub frost_steel: String,
    /// `snow_storm_0` (`#D8DEE9`) — dim facts / idle / fresh.
    pub snow_dim: String,
}

impl NordPalette {
    /// Resolve every pleme-io-native segment accent from
    /// `ColorPalette::pleme()`.
    #[must_use]
    pub fn pleme() -> Self {
        let p = ColorPalette::pleme();
        Self {
            aurora_green: p.aurora_green.hex(),
            aurora_red: p.aurora_red.hex(),
            aurora_yellow: p.aurora_yellow.hex(),
            aurora_orange: p.aurora_orange.hex(),
            frost_cyan: p.frost_1.hex(),
            frost_steel: p.frost_2.hex(),
            snow_dim: p.snow_storm_0.hex(),
        }
    }

    /// `bold <hex>` style string for an accent.
    #[must_use]
    pub fn bold(accent: &str) -> String {
        format!("bold {accent}")
    }

    /// `dimmed <hex>` style string for an accent (SGR-dim).
    #[must_use]
    pub fn dimmed(accent: &str) -> String {
        format!("dimmed {accent}")
    }

    /// `dim <hex>` style string for an accent (the short SGR-dim form
    /// some segments author).
    #[must_use]
    pub fn dim(accent: &str) -> String {
        format!("dim {accent}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_resolves_the_classic_nord_hexes_from_ishou() {
        // Proof the seki-native accents track the BORN Nord tokens —
        // the exact hexes the segment defaults paint with, now sourced.
        let p = NordPalette::pleme();
        assert_eq!(p.aurora_green, "#A3BE8C");
        assert_eq!(p.aurora_red, "#BF616A");
        assert_eq!(p.aurora_yellow, "#EBCB8B");
        assert_eq!(p.aurora_orange, "#D08770");
        assert_eq!(p.frost_cyan, "#88C0D0");
        assert_eq!(p.frost_steel, "#81A1C1");
        assert_eq!(p.snow_dim, "#D8DEE9");
    }

    #[test]
    fn style_helpers_compose_token_hexes() {
        let p = NordPalette::pleme();
        assert_eq!(NordPalette::bold(&p.aurora_green), "bold #A3BE8C");
        assert_eq!(NordPalette::dimmed(&p.snow_dim), "dimmed #D8DEE9");
        assert_eq!(NordPalette::dim(&p.snow_dim), "dim #D8DEE9");
    }
}
