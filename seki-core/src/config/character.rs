//! Typed config for the trailing prompt character (`$` / `❯` / `❄`).
//!
//! Models starship's `[character]` section in full: success/error
//! symbols plus the four vim-mode symbols (normal/replace-one/replace/
//! visual), each carrying an embedded style (starship's
//! `"[❄](bold #88C0D0)"` grammar).

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterConfig {
    pub enabled: bool,
    /// Format string. Default matches starship's `"$symbol "`.
    pub format: String,
    pub success_symbol: String,
    pub error_symbol: String,
    pub vicmd_symbol: String,
    pub vimcmd_replace_one_symbol: String,
    pub vimcmd_replace_symbol: String,
    pub vimcmd_visual_symbol: String,
    /// Style applied when starship's `[$symbol]($style)` grammar
    /// isn't already embedded in the symbol. seki's `render` reads
    /// the embedded style first, falls back to this.
    pub style: StyleSpec,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "$symbol ".to_owned(),
            success_symbol: "❯".to_owned(),
            error_symbol: "❯".to_owned(),
            vicmd_symbol: "❮".to_owned(),
            vimcmd_replace_one_symbol: "❯".to_owned(),
            vimcmd_replace_symbol: "❯".to_owned(),
            vimcmd_visual_symbol: "❯".to_owned(),
            style: StyleSpec::new("bold green"),
        }
    }
}

impl CharacterConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            success_symbol: String::new(),
            error_symbol: String::new(),
            vicmd_symbol: String::new(),
            vimcmd_replace_one_symbol: String::new(),
            vimcmd_replace_symbol: String::new(),
            vimcmd_visual_symbol: String::new(),
            style: StyleSpec::default(),
        }
    }
}
