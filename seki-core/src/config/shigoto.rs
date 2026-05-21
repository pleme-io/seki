//! Typed config for the `shigoto` segment.
//!
//! Pleme-io-native (Tier 3): surfaces active shigoto job-DAG state
//! when a shigoto daemon is reachable on the host.
//!
//! # Theme
//!
//! - Nord-aurora orange `#D08770` when any job is running or pending
//! - Nord-aurora green `#A3BE8C` when the scheduler is reachable but idle
//!
//! # Probe budget
//!
//! HTTP GET against `SHIGOTO_ADDR` (default `http://127.0.0.1:38830`)
//! with a hard `command_timeout_ms` bound. Gracefully absent on any
//! failure.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShigotoConfig {
    pub enabled: bool,
    pub format: String,
    pub active_style: StyleSpec,
    pub idle_style: StyleSpec,
    pub addr: String,
    pub snapshot_path: String,
    pub command_timeout_ms: u64,
    pub cache_ttl_secs: u64,
}

impl Default for ShigotoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: "[$status]($style)".to_owned(),
            active_style: StyleSpec::new("bold #D08770"),
            idle_style: StyleSpec::new("bold #A3BE8C"),
            addr: "$env".to_owned(),
            snapshot_path: "/v1/snapshot".to_owned(),
            command_timeout_ms: 200,
            cache_ttl_secs: 5,
        }
    }
}

impl ShigotoConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            active_style: StyleSpec::default(),
            idle_style: StyleSpec::default(),
            addr: String::new(),
            snapshot_path: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
