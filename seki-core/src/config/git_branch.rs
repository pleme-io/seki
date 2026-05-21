//! Typed config for the `git_branch` segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitBranchConfig {
    pub enabled: bool,
    /// Format string. Default: `" [$symbol$branch]($style)"`.
    pub format: String,
    pub symbol: String,
    pub style: StyleSpec,
    pub truncation_length: u32,
    pub truncation_symbol: String,
    pub only_attached: bool,
    /// Optional prefix wrapping the branch name (e.g. `"on "`).
    pub prefix: String,
    pub suffix: String,
}

impl Default for GitBranchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: " [$symbol$branch]($style)".to_owned(),
            symbol: " ".to_owned(),
            style: StyleSpec::new("bold purple"),
            truncation_length: u32::MAX,
            truncation_symbol: "…".to_owned(),
            only_attached: false,
            prefix: "on ".to_owned(),
            suffix: " ".to_owned(),
        }
    }
}

impl GitBranchConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            symbol: String::new(),
            style: StyleSpec::default(),
            truncation_length: 0,
            truncation_symbol: String::new(),
            only_attached: false,
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}
