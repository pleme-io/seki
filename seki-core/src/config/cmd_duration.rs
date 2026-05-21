//! Typed config for the `cmd_duration` segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CmdDurationConfig {
    pub enabled: bool,
    /// Minimum duration in milliseconds before the segment renders.
    pub min_time: u64,
    pub style: StyleSpec,
    /// Format string. Substitution: `$duration`.
    pub format: String,
    pub show_milliseconds: bool,
}

impl Default for CmdDurationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_time: 2_000,
            style: StyleSpec::new("yellow bold"),
            format: "took [$duration]($style) ".to_owned(),
            show_milliseconds: false,
        }
    }
}

impl CmdDurationConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            min_time: 0,
            style: StyleSpec::default(),
            format: String::new(),
            show_milliseconds: false,
        }
    }
}
