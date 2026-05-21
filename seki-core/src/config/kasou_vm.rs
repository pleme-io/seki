//! Typed config for the `kasou_vm` segment.
//!
//! Pleme-io-native (Tier 5 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the count of *running* kasou VMs in the operator's prompt. Kasou is
//! the pleme-io wrapper around Apple's Virtualization framework; a
//! `running` VM means a backend Linux node is consuming host RAM and
//! CPU right now. Operators routinely want to know "is my k3s VM up?"
//! without `kasou list`-ing every prompt.
//!
//! # Theme
//!
//! - Nord-frost cyan `#88C0D0` when `count >= 1` (active fleet)
//! - Snowstorm dim white `#D8DEE9` when `count == 0` (kasou reachable
//!   but no VMs running — distinguish from absent kasou which renders
//!   nothing at all)
//!
//! # Probe budget
//!
//! Subprocess (`kasou list --format=json`) bounded by
//! `command_timeout_ms`. Renders nothing when kasou is missing from
//! PATH / returns non-zero / produces unparseable JSON / times out —
//! the segment never blocks the prompt. An in-process cache
//! ([`Self::cache_ttl_secs`]) prevents repeated invocations across
//! renders in one shell session.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the kasou_vm prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KasouVmConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$count` — number of running VMs
    /// - `$status` — computed label: `kasou: N vm` (always pluralised
    ///   without an `s` — matches existing pleme-io prompt vocabulary)
    /// Starship-style `[…]($style)` markup is stripped.
    pub format: String,
    /// Style applied when at least one VM is running. Nord-frost cyan.
    pub active_style: StyleSpec,
    /// Style applied when kasou is reachable but no VMs run.
    /// Snowstorm dim white.
    pub idle_style: StyleSpec,
    /// Path to the `kasou` binary. Falls back to `kasou` (PATH lookup).
    pub command: String,
    /// Subprocess timeout in milliseconds.
    pub command_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous count instead of re-spawning kasou. Stale
    /// renders annotate the segment with `(stale)`.
    pub cache_ttl_secs: u64,
}

impl Default for KasouVmConfig {
    fn default() -> Self {
        Self {
            // Tier 5 default: disabled (probe cost > 0; operator
            // opts in via SEKI_TIER or per-segment toggle).
            enabled: false,
            format: "[$status]($style)".to_owned(),
            active_style: StyleSpec::new("bold #88C0D0"),
            idle_style: StyleSpec::new("dimmed #D8DEE9"),
            command: "kasou".to_owned(),
            command_timeout_ms: 500,
            cache_ttl_secs: 60,
        }
    }
}

impl KasouVmConfig {
    /// Zero-opinion: nothing probed, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            active_style: StyleSpec::default(),
            idle_style: StyleSpec::default(),
            command: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
