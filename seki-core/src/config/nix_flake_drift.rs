//! Typed config for the `nix_flake_drift` segment.
//!
//! Pleme-io-native (Tier 3): surfaces the count of pleme-io fleet
//! inputs in cwd's `flake.nix` whose locked rev is behind upstream
//! HEAD.
//!
//! # Theme
//!
//! - Nord-aurora red `#BF616A` — drift ≥ 1
//! - Snowstorm dim white `#D8DEE9` — drift == 0
//!
//! # Probe budget
//!
//! Hard-bounded by `command_timeout_ms`. Gracefully absent outside
//! a flake dir.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NixFlakeDriftConfig {
    pub enabled: bool,
    pub format: String,
    pub drift_style: StyleSpec,
    pub fresh_style: StyleSpec,
    pub nix_command: String,
    pub git_command: String,
    pub command_timeout_ms: u64,
    pub cache_ttl_secs: u64,
    pub input_prefix: String,
}

impl Default for NixFlakeDriftConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: "[$status]($style)".to_owned(),
            drift_style: StyleSpec::new("bold #BF616A"),
            fresh_style: StyleSpec::new("dim #D8DEE9"),
            nix_command: "nix".to_owned(),
            git_command: "git".to_owned(),
            command_timeout_ms: 800,
            cache_ttl_secs: 120,
            input_prefix: "github:pleme-io/".to_owned(),
        }
    }
}

impl NixFlakeDriftConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            drift_style: StyleSpec::default(),
            fresh_style: StyleSpec::default(),
            nix_command: String::new(),
            git_command: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
            input_prefix: String::new(),
        }
    }
}
