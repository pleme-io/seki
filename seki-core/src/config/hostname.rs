//! Typed config for the `hostname` segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostnameConfig {
    pub enabled: bool,
    /// Show only when connected via SSH.
    pub ssh_only: bool,
    /// Truncate the hostname at this character — `"."` keeps just
    /// the short hostname before the first dot.
    pub trim_at: String,
    pub style: StyleSpec,
    /// Format string, mirroring starship's `"[$hostname](dimmed $style) · "`
    /// — `$hostname` is the substitution.
    pub format: String,
}

impl Default for HostnameConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ssh_only: true,
            trim_at: ".".to_owned(),
            style: StyleSpec::new("bold dimmed green"),
            format: "[$hostname]($style) in ".to_owned(),
        }
    }
}

impl HostnameConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            ssh_only: false,
            trim_at: String::new(),
            style: StyleSpec::default(),
            format: String::new(),
        }
    }
}
