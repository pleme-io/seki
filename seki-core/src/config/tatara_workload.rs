//! Typed config for the `tatara_workload` segment.
//!
//! Pleme-io-native (Tier 3): surfaces the running-allocation count
//! from the tatara workload scheduler.
//!
//! # Theme
//!
//! Nord-frost cyan `#88C0D0` — neutral fleet-state.
//!
//! # Probe budget
//!
//! Subprocess to `tatara node list --format=json` with a hard
//! `command_timeout_ms` bound. Gracefully absent when tatara is
//! missing / errored / timed out.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TataraWorkloadConfig {
    pub enabled: bool,
    pub format: String,
    pub style: StyleSpec,
    pub command: String,
    pub command_timeout_ms: u64,
    pub cache_ttl_secs: u64,
}

impl Default for TataraWorkloadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: "[$status]($style)".to_owned(),
            style: StyleSpec::new("bold #88C0D0"),
            command: "tatara".to_owned(),
            command_timeout_ms: 300,
            cache_ttl_secs: 30,
        }
    }
}

impl TataraWorkloadConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
            command: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
