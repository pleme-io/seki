//! Typed config for the `nix_shell` segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

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
            symbol: "❄️ ".to_owned(),
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
