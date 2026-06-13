//! Borealis-themed prescribed `SekiConfig` — the fleet default prompt.
//!
//! Structurally identical to `blzsh_parity_config` (same segments, same
//! layout, same ❄), but every accent is re-pointed to a BORN Borealis
//! token via [`crate::borealis::PromptPalette`] (spec §7 seki table).
//! The ❄ snowflake glyph is preserved fleet-wide.
//!
//! Built by overriding only the color-carrying fields of the
//! blzsh-parity base, so the two configs never drift in structure —
//! only in palette. The vigy / MCP-activity segments take **fable
//! violet**, the agent accent.
//!
//! `TieredConfig::prescribed_default()` reads from here.

use seki_core::SekiConfig;
use seki_core::style::StyleSpec;

use crate::blzsh_parity::blzsh_parity_config;
use crate::borealis::PromptPalette;

/// The Borealis prescribed default. Same shape as blzsh-parity; every
/// accent resolved from a Borealis token.
#[must_use]
pub fn borealis_config() -> SekiConfig {
    let p = PromptPalette::night();
    let mut c = blzsh_parity_config();

    // [character] — ❄ glyphs, Borealis accents (spec §7):
    // success=ice_cyan, error=aurora_red, vicmd=aurora_green,
    // replace=solar_magenta, visual=first_light.
    c.character.success_symbol = PromptPalette::snowflake_bold(&p.ice_cyan);
    c.character.error_symbol = PromptPalette::snowflake_bold(&p.aurora_red);
    c.character.vicmd_symbol = PromptPalette::snowflake_bold(&p.aurora_green);
    c.character.vimcmd_replace_one_symbol = PromptPalette::snowflake_bold(&p.solar_magenta);
    c.character.vimcmd_replace_symbol = PromptPalette::snowflake_bold(&p.solar_magenta);
    c.character.vimcmd_visual_symbol = PromptPalette::snowflake_bold(&p.first_light);
    c.character.style = StyleSpec::new(PromptPalette::bold(&p.ice_cyan));

    // continuation ❄ = ice_steel.
    c.continuation_prompt = format!("{} ", PromptPalette::snowflake_bold(&p.ice_steel));

    // git_branch = aurora_green.
    c.git_branch.style = StyleSpec::new(&p.aurora_green);
    // git_status = first_light bold.
    c.git_status.style = StyleSpec::new(PromptPalette::bold(&p.first_light));

    // hostname = ice_cyan + SGR-dim.
    c.hostname.style = StyleSpec::new(PromptPalette::dimmed(&p.ice_cyan));

    // directory = ice_steel bold; read_only = aurora_red.
    c.directory.style = StyleSpec::new(PromptPalette::bold(&p.ice_steel));
    c.directory.read_only_style = StyleSpec::new(&p.aurora_red);

    // cmd_duration = ember.
    c.cmd_duration.style = StyleSpec::new(PromptPalette::bold(&p.ember));

    // nix_shell ❄ = ice_cyan.
    c.nix_shell.style = StyleSpec::new(PromptPalette::bold(&p.ice_cyan));

    // env_var TEAR_SESSION_NAME = ice_cyan bold (WORKSPACE stays the
    // typed `dimmed italic` SGR — no hex).
    if let Some(tear) = c.env_var.entries.get_mut("TEAR_SESSION_NAME") {
        tear.style = StyleSpec::new(PromptPalette::bold(&p.ice_cyan));
    }

    // custom.tear_pane = ice_cyan dimmed.
    if let Some(tear_pane) = c.custom.entries.get_mut("tear_pane") {
        tear_pane.style = StyleSpec::new(PromptPalette::dimmed(&p.ice_cyan));
    }

    // vigy = fable_violet bold — THE agent segment (spec §7). MCP /
    // AI / reconciler surfaces wear the agent-reserved accent.
    c.vigy.style = StyleSpec::new(PromptPalette::bold(&p.fable_violet));

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borealis_character_uses_borealis_accents_and_keeps_the_snowflake() {
        let c = borealis_config();
        // ❄ preserved fleet-wide; accents are BORN Borealis tokens.
        assert_eq!(c.character.success_symbol, "[❄](bold #73C6D9)"); // ice_cyan
        assert_eq!(c.character.error_symbol, "[❄](bold #D86E67)"); // aurora_red
        assert_eq!(c.character.vicmd_symbol, "[❄](bold #67D191)"); // aurora_green
        assert_eq!(c.character.vimcmd_replace_symbol, "[❄](bold #C673A3)"); // solar_magenta
        assert_eq!(c.character.vimcmd_visual_symbol, "[❄](bold #EDC980)"); // first_light
        // Every character glyph still carries the ❄.
        for sym in [
            &c.character.success_symbol,
            &c.character.error_symbol,
            &c.character.vicmd_symbol,
        ] {
            assert!(sym.contains('❄'), "lost the snowflake: {sym}");
        }
    }

    #[test]
    fn borealis_continuation_uses_ice_steel_snowflake() {
        let c = borealis_config();
        assert_eq!(c.continuation_prompt, "[❄](bold #6A90C0) "); // ice_steel
    }

    #[test]
    fn borealis_segment_accents_track_the_spec_table() {
        let c = borealis_config();
        assert_eq!(c.git_branch.style.as_str(), "#67D191"); // aurora_green
        assert_eq!(c.git_status.style.as_str(), "bold #EDC980"); // first_light
        assert_eq!(c.hostname.style.as_str(), "dimmed #73C6D9"); // ice_cyan
        assert_eq!(c.directory.style.as_str(), "bold #6A90C0"); // ice_steel
        assert_eq!(c.directory.read_only_style.as_str(), "#D86E67"); // aurora_red
        assert_eq!(c.cmd_duration.style.as_str(), "bold #E89772"); // ember
        assert_eq!(c.nix_shell.style.as_str(), "bold #73C6D9"); // ice_cyan
    }

    #[test]
    fn vigy_is_the_agent_segment_in_fable_violet() {
        // THE agent segment wears the agent-reserved accent (spec §7).
        let c = borealis_config();
        assert_eq!(c.vigy.style.as_str(), "bold #B69AE9"); // fable_violet
    }

    #[test]
    fn borealis_preserves_blzsh_structure() {
        // Same segments enabled/disabled + order as blzsh-parity — only
        // the palette changed.
        let c = borealis_config();
        assert!(c.character.enabled);
        assert!(c.git_branch.enabled);
        assert!(!c.rust.enabled);
        assert_eq!(c.prompt_order.first().map(String::as_str), Some("nix_shell"));
        assert_eq!(c.prompt_order.last().map(String::as_str), Some("character"));
    }
}
