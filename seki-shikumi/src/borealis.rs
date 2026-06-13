//! Borealis prompt palette — token hexes resolved from ishou.
//!
//! Colors are BORN in ishou (`ishou_tokens::BorealisPalette`). This
//! module is the seki-side adapter: it resolves each Borealis token to
//! its `#RRGGBB` string ONCE, so the prescribed `SekiConfig` builds its
//! `StyleSpec` strings from a token reference, never a hand-authored
//! hex. A Borealis token edit upstream flows here on the next compile.
//!
//! Per the pleme-io law: NEVER hand-author a hex in a downstream repo.
//! Every accent the prompt paints is one of these named accessors.

use ishou_tokens::BorealisPalette;

/// Resolve a Borealis token name to its `#RRGGBB` hex. Panics on an
/// unknown name — a typo is a compile-adjacent bug surfaced at the
/// first test render, never a silent wrong color.
fn hex(name: &str) -> String {
    BorealisPalette::night()
        .get(name)
        .unwrap_or_else(|| panic!("unknown Borealis token: {name}"))
        .hex()
}

/// The prompt accents, by SEMANTIC, each resolved from a Borealis token.
/// These map the spec §7 seki table onto named accessors so the
/// prescribed config reads as intent, not as hex.
pub struct PromptPalette {
    /// `ice_cyan` — the ❄ glyph, primary accent, hostname.
    pub ice_cyan: String,
    /// `ice_steel` — links / directory / continuation ❄ / mado_session.
    pub ice_steel: String,
    /// `aurora_green` — command / success / vicmd / git_branch.
    pub aurora_green: String,
    /// `aurora_red` — error / read-only / drift.
    pub aurora_red: String,
    /// `first_light` — warning / git_status / search.
    pub first_light: String,
    /// `solar_magenta` — keyword / vim replace.
    pub solar_magenta: String,
    /// `ember` — cmd_duration / rust / annotations.
    pub ember: String,
    /// `fable_violet` — THE agent accent (vigy / MCP / AI surfaces).
    pub fable_violet: String,
    /// `violet_bright` — agent attention state.
    pub violet_bright: String,
    /// `shadow1` — comments / muted / fresh-drift.
    pub shadow1: String,
    /// `snow0` — fleet_node / ishou_theme dim facts.
    pub snow0: String,
}

impl PromptPalette {
    /// Resolve every prompt accent from `BorealisPalette::night()`.
    #[must_use]
    pub fn night() -> Self {
        Self {
            ice_cyan: hex("ice_cyan"),
            ice_steel: hex("ice_steel"),
            aurora_green: hex("aurora_green"),
            aurora_red: hex("aurora_red"),
            first_light: hex("first_light"),
            solar_magenta: hex("solar_magenta"),
            ember: hex("ember"),
            fable_violet: hex("fable_violet"),
            violet_bright: hex("violet_bright"),
            shadow1: hex("shadow1"),
            snow0: hex("snow0"),
        }
    }

    /// `bold <hex>` style string for an accent.
    #[must_use]
    pub fn bold(accent: &str) -> String {
        format!("bold {accent}")
    }

    /// `dimmed <hex>` style string for an accent.
    #[must_use]
    pub fn dimmed(accent: &str) -> String {
        format!("dimmed {accent}")
    }

    /// `[❄](bold <hex>)` symbol markup for a character / continuation
    /// glyph. The ❄ is preserved fleet-wide.
    #[must_use]
    pub fn snowflake_bold(accent: &str) -> String {
        format!("[❄](bold {accent})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_resolves_the_spec_hexes_from_ishou() {
        let p = PromptPalette::night();
        // The §7 anchors — proof the seki accents track the BORN tokens.
        assert_eq!(p.ice_cyan, "#73C6D9");
        assert_eq!(p.aurora_green, "#67D191");
        assert_eq!(p.aurora_red, "#D86E67");
        assert_eq!(p.first_light, "#EDC980");
        assert_eq!(p.solar_magenta, "#C673A3");
        assert_eq!(p.ice_steel, "#6A90C0");
        assert_eq!(p.ember, "#E89772");
        // THE agent accent.
        assert_eq!(p.fable_violet, "#B69AE9");
        assert_eq!(p.violet_bright, "#C6A9FC");
    }

    #[test]
    fn style_helpers_compose_token_hexes() {
        let p = PromptPalette::night();
        assert_eq!(PromptPalette::bold(&p.aurora_green), "bold #67D191");
        assert_eq!(
            PromptPalette::snowflake_bold(&p.ice_cyan),
            "[❄](bold #73C6D9)"
        );
    }
}
