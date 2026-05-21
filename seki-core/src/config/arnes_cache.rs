//! Typed config for the `arnes_cache` segment.
//!
//! Pleme-io-native (Tier 5 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the P2P content-cache hit rate of the arnes daemon when it's
//! running on the host. Arnes is pleme-io's content-addressed P2P
//! cache — operators want to see at a glance whether the cache is
//! warm, lukewarm, or cold before kicking off a fan-out workload.
//!
//! # Theme
//!
//! Tristate Nord-aurora — same vocabulary as `tend` (green clean /
//! yellow light / red heavy):
//!
//! - `hit_rate >= 0.80` → green `#A3BE8C` (warm cache)
//! - `0.50 <= hit_rate < 0.80` → yellow `#EBCB8B` (lukewarm)
//! - `hit_rate < 0.50` → red `#BF616A` (cold)
//!
//! # Probe budget
//!
//! Unix-socket request to `~/.local/share/arnes/arnes.sock` (or
//! `ARNES_SOCKET` env override). Bounded by `command_timeout_ms`.
//! Renders nothing on any failure (socket absent / connect refused /
//! parse failure). The same thread + `recv_timeout` pattern from
//! `tend`/`engenho` enforces the wall budget.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the arnes_cache prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArnesCacheConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$pct` — hit rate rendered as a 0-100 integer percent
    /// - `$status` — computed label: `arnes: NN%`
    /// Starship-style `[…]($style)` markup is stripped.
    pub format: String,
    /// Style applied when hit rate >= `warm_threshold`. Nord green.
    pub warm_style: StyleSpec,
    /// Style applied when `cold_threshold <= rate < warm_threshold`.
    /// Nord yellow.
    pub lukewarm_style: StyleSpec,
    /// Style applied when hit rate < `cold_threshold`. Nord red.
    pub cold_style: StyleSpec,
    /// Inclusive lower bound for warm (e.g. 0.80 = 80%).
    pub warm_threshold: f32,
    /// Inclusive lower bound for lukewarm (e.g. 0.50 = 50%).
    pub cold_threshold: f32,
    /// Default Unix socket path (relative to `$HOME` if not absolute).
    /// Empty string disables the probe entirely.
    pub socket_path: String,
    /// Env var consulted for an override of [`Self::socket_path`].
    /// When set + non-empty replaces the path for one probe.
    pub socket_env_var: String,
    /// Subprocess timeout in milliseconds.
    pub command_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous hit-rate value. Stale renders annotate
    /// with `(stale)`.
    pub cache_ttl_secs: u64,
}

impl Default for ArnesCacheConfig {
    fn default() -> Self {
        Self {
            // Tier 5 default: disabled (probe cost > 0; operator
            // opts in via SEKI_TIER or per-segment toggle).
            enabled: false,
            format: "[$status]($style)".to_owned(),
            warm_style: StyleSpec::new("bold #A3BE8C"),
            lukewarm_style: StyleSpec::new("bold #EBCB8B"),
            cold_style: StyleSpec::new("bold #BF616A"),
            warm_threshold: 0.80,
            cold_threshold: 0.50,
            // Relative to $HOME — the module resolves the full path
            // at probe time (so the typed default stays portable).
            socket_path: ".local/share/arnes/arnes.sock".to_owned(),
            socket_env_var: "ARNES_SOCKET".to_owned(),
            command_timeout_ms: 200,
            cache_ttl_secs: 30,
        }
    }
}

impl ArnesCacheConfig {
    /// Zero-opinion: nothing probed, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            warm_style: StyleSpec::default(),
            lukewarm_style: StyleSpec::default(),
            cold_style: StyleSpec::default(),
            warm_threshold: 0.0,
            cold_threshold: 0.0,
            socket_path: String::new(),
            socket_env_var: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
