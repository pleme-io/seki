//! Typed config for the `mado_session` segment.
//!
//! Pleme-io-native (Tier 3): surfaces the live mado session count +
//! current average fps via the mado MCP Unix socket.
//!
//! # Theme
//!
//! Nord-frost blue `#81A1C1`.
//!
//! # Probe budget
//!
//! Unix socket connect + minimal MCP query, hard-bounded by
//! `command_timeout_ms`. Gracefully absent on any failure.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MadoSessionConfig {
    pub enabled: bool,
    pub format: String,
    pub style: StyleSpec,
    /// Socket path. `"$env"` reads `MADO_SOCKET` (default
    /// `<home>/.local/share/mado/mado.sock`). Empty disables.
    pub socket_path: String,
    pub command_timeout_ms: u64,
    pub cache_ttl_secs: u64,
}

impl Default for MadoSessionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: "[$status]($style)".to_owned(),
            style: StyleSpec::new("bold #81A1C1"),
            socket_path: "$env".to_owned(),
            command_timeout_ms: 200,
            cache_ttl_secs: 5,
        }
    }
}

impl MadoSessionConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
            socket_path: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
