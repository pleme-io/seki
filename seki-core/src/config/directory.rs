//! Typed config for the `directory` segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectoryConfig {
    pub enabled: bool,
    /// Format string. Default: `"[$path]($style)"`.
    pub format: String,
    pub truncation_length: u32,
    pub truncate_to_repo: bool,
    pub truncation_symbol: String,
    pub home_symbol: String,
    pub read_only: String,
    pub read_only_style: StyleSpec,
    pub style: StyleSpec,
    /// Trailing space before the next segment.
    pub suffix: String,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "[$path]($style)".to_owned(),
            truncation_length: 3,
            truncate_to_repo: true,
            truncation_symbol: "…/".to_owned(),
            home_symbol: "~".to_owned(),
            read_only: " 🔒".to_owned(),
            read_only_style: StyleSpec::new("red"),
            style: StyleSpec::new("bold cyan"),
            suffix: " ".to_owned(),
        }
    }
}

impl DirectoryConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            truncation_length: 0,
            truncate_to_repo: false,
            truncation_symbol: String::new(),
            home_symbol: String::new(),
            read_only: String::new(),
            read_only_style: StyleSpec::default(),
            style: StyleSpec::default(),
            suffix: String::new(),
        }
    }
}
