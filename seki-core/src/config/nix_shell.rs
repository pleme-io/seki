//! Typed config for the `nix_shell` segment.

use crate::style::StyleSpec;
use ishou_tokens::{SekiSignals, SignalMode};
use serde::{Deserialize, Serialize};

/// The prescribed nix glyph for the `nix_shell` segment, sourced from
/// the fleet [`SekiSignals`] atlas. Emoji mode (`❄️`) matches seki's
/// historical `nix_shell` default symbol exactly — the adoption moves
/// the SOURCE to the fleet atlas, not the rendered symbol. (The
/// blzsh-parity prompt uses the single-width `Glyph` form `❄` directly,
/// sourced separately in `seki-shikumi::blzsh_parity`.)
fn nix_symbol() -> String {
    const PRESCRIBED: SekiSignals = SekiSignals::prescribed();
    format!("{} ", PRESCRIBED.lang_nix.render(SignalMode::Emoji))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NixShellConfig {
    pub enabled: bool,
    /// Format string. Default: `"via [$symbol$state( \\($name\\))]($style) "`.
    pub format: String,
    pub symbol: String,
    pub style: StyleSpec,
    /// Format when in an impure nix-shell. Substitutions:
    /// `{name}` → IN_NIX_SHELL value (`pure` / `impure`).
    pub impure_format: String,
    pub pure_format: String,
    /// Unknown env value — fallback symbol-only.
    pub unknown_format: String,
    pub prefix: String,
    pub suffix: String,
}

impl Default for NixShellConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "[$symbol]($style) ".to_owned(),
            // ❄️ from SekiSignals.lang_nix (emoji) + trailing space.
            symbol: nix_symbol(),
            style: StyleSpec::new("bold blue"),
            impure_format: "impure".to_owned(),
            pure_format: "pure".to_owned(),
            unknown_format: "nix".to_owned(),
            prefix: String::new(),
            suffix: " ".to_owned(),
        }
    }
}

impl NixShellConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            symbol: String::new(),
            style: StyleSpec::default(),
            impure_format: String::new(),
            pure_format: String::new(),
            unknown_format: String::new(),
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forcing function: the prescribed `nix_shell` symbol MUST equal
    /// the fleet [`SekiSignals`]`.lang_nix` emoji glyph (with seki's
    /// trailing space). Drift fails the build.
    #[test]
    fn nix_symbol_is_sourced_from_seki_signals() {
        let s = SekiSignals::prescribed();
        let expected = format!("{} ", s.lang_nix.render(SignalMode::Emoji));
        assert_eq!(NixShellConfig::default().symbol, expected);
        // Adoption is glyph-identical: the atlas emoji matches seki's
        // historical ❄️ symbol.
        assert_eq!(s.lang_nix.render(SignalMode::Emoji), "❄️");
    }
}
