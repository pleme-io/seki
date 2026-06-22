//! Companion-themed prescribed `SekiConfig` — the fleet default prompt.
//!
//! An **emoji-forward, compact "companion"** reskin of the Nord
//! blzsh-parity prompt. Same segment *structure* (the structural
//! contract is preserved — every segment renders only when relevant),
//! but each symbol becomes a friendly emoji and the icy Nord frost base
//! gains a **touch of Brazilian warmth**: the 🌿 branch sprig, the 🌊
//! session tide, the ⏱ ember duration — Aurora green / orange accents
//! over the cold frost.
//!
//! Short and sweet by construction: every segment is *conditional*, so
//! a plain local dir is just `📁 ~/dir ❄`, while a nix-shell + tear
//! session + dirty repo blooms into a full companion
//! `❄ 🌊 sess 📁 …/nix 🌿 main 🟡 ❄`. Hostname is **ssh-only** (you
//! know your own machine; it appears the moment you're remote — crucial
//! fleet context, zero local cost).
//!
//! The ❄ snowflake (the nix glyph + the prompt character) is preserved
//! fleet-wide — the cold Nord signature stays the anchor.
//!
//! `TieredConfig::prescribed_default()` reads from here.

use seki_core::SekiConfig;

use crate::blzsh_parity::blzsh_parity_config;

/// The companion prescribed default. Same Nord-frost palette + segment
/// structure as blzsh-parity; symbols become emojis and the warm
/// accents add the Brazilian touch.
#[must_use]
pub fn companion_config() -> SekiConfig {
    let mut c = blzsh_parity_config();

    // nix_shell — keep the ❄ (the nix glyph + fleet signature, frost cyan).
    // The cold anchor; unchanged from blzsh.

    // directory — 📁 the anchor. Renders on every prompt; everything else
    // is conditional, so this emoji + the path is the "short" baseline.
    // Trailing space (every segment owns its own trailing space, none
    // lead) so the path never abuts the next segment / the ❄ character.
    c.directory.format = "📁 [$path]($style) ".to_owned();

    // git_branch — 🌿 the green sprig: the warm, growing, Brazilian-nature
    // touch on an otherwise icy prompt. Nord aurora green retained.
    // Trailing space (consistent spacing rule): `🌿 main ` then git_status
    // or the ❄ character, never `main🟡` / `main❄`.
    c.git_branch.symbol = "🌿 ".to_owned();
    c.git_branch.format = "[$symbol$branch]($style) ".to_owned();

    // git_status — emoji status dots (warm flag-ish hues), kept compact.
    // ahead/behind/diverged keep their fleet ⇡⇣⇕ glyphs + counts (crucial),
    // the dirty cluster collapses to single-glyph emoji so it stays short.
    c.git_status.modified = "🟡".to_owned();
    c.git_status.staged = "🟢".to_owned();
    c.git_status.untracked = "⚪".to_owned();
    c.git_status.deleted = "🔴".to_owned();
    c.git_status.renamed = "🔁".to_owned();
    c.git_status.conflicted = "💥".to_owned();
    c.git_status.stashed = "📦".to_owned();

    // hostname — 🖥 + ssh-only: silent locally (you know your machine),
    // appears the instant you're on a remote fleet node (crucial context).
    c.hostname.ssh_only = true;
    c.hostname.format = "🖥 [$hostname](dimmed $style) ".to_owned();

    // env_var — 📦 the tend workspace, 🌊 the praça session tide.
    if let Some(ws) = c.env_var.entries.get_mut("WORKSPACE") {
        ws.format = "📦 [$env_value]($style) ".to_owned();
    }
    if let Some(tear) = c
        .env_var
        .entries
        .get_mut(ishou_tokens::FleetStateVar::TearSessionName.name())
    {
        tear.format = "🌊 [$env_value]($style) ".to_owned();
    }

    // custom tear_pane — 🔖 a tiny pane tag (only inside a tear pane).
    if let Some(tp) = c.custom.entries.get_mut("tear_pane") {
        tp.format = "🔖 [$output]($style) ".to_owned();
    }

    // cmd_duration — ⏱ the ember: the companion "that took a while" beat,
    // only when a command ran > 2s. Warm Nord orange retained.
    c.cmd_duration.format = "⏱ [$duration]($style) ".to_owned();

    // rust — 🦀 the fleet's mother tongue. pleme-io is a Rust-dominant
    // fleet, so the toolchain channel is *standard context* — but the
    // segment is conditional (only when a Cargo.toml / rust-toolchain is
    // detected), so it's silent outside Rust work and present across the
    // fleet. Inserted just before the ❄ character. Nord aurora red, and
    // (per the fleet spacing rule) it owns its own trailing space.
    c.rust.enabled = true;
    c.rust.symbol = "🦀 ".to_owned();
    c.rust.prefix = String::new();
    c.rust.suffix = " ".to_owned();
    c.rust.style = seki_core::style::StyleSpec::new("bold #BF616A"); // nord aurora red
    if let Some(pos) = c.prompt_order.iter().position(|s| s == "character") {
        c.prompt_order.insert(pos, "rust".to_owned());
    }

    // character — the ❄ stays (fleet signature; success frost cyan, error
    // aurora red). The warmth lives in the accents above, not the anchor.

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companion_is_emoji_forward() {
        let c = companion_config();
        assert!(c.directory.format.contains("📁"), "dir → 📁");
        assert!(c.git_branch.symbol.contains("🌿"), "branch → 🌿");
        assert!(c.cmd_duration.format.contains("⏱"), "duration → ⏱");
        assert!(c.hostname.format.contains("🖥"), "host → 🖥");
        let tear = ishou_tokens::FleetStateVar::TearSessionName.name();
        assert!(c.env_var.entries[tear].format.contains("🌊"), "session → 🌊");
    }

    #[test]
    fn companion_keeps_the_fleet_snowflake_and_structure() {
        let c = companion_config();
        // ❄ preserved on the nix glyph + the prompt character.
        assert!(c.nix_shell.symbol.contains('❄'), "nix keeps ❄");
        assert!(c.character.success_symbol.contains('❄'), "prompt keeps ❄");
        // Same conditional segment set + order as the parity base.
        assert!(c.directory.enabled);
        assert!(c.git_branch.enabled);
        assert!(c.cmd_duration.enabled);
        assert_eq!(c.prompt_order.first().map(String::as_str), Some("nix_shell"));
        assert_eq!(c.prompt_order.last().map(String::as_str), Some("character"));
    }

    #[test]
    fn companion_hostname_is_ssh_only_for_local_compactness() {
        // Short-and-sweet locally: the host only appears when remote.
        assert!(companion_config().hostname.ssh_only);
    }

    #[test]
    fn companion_keeps_nord_palette() {
        // "nord typical theme, cold" — the base palette stays classic Nord
        // frost; warmth is an accent, not a repaint.
        let c = companion_config();
        assert_eq!(c.git_branch.style.as_str(), "#A3BE8C"); // nord aurora green
        assert_eq!(c.cmd_duration.style.as_str(), "bold #D08770"); // nord aurora orange
        assert_eq!(c.directory.style.as_str(), "bold #81A1C1"); // nord frost blue
    }
}
