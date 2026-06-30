//! Typed config for the `tend` segment.
//!
//! Pleme-io-native: surfaces the operator's tend workspace status
//! (clean / N dirty) in the prompt. Tells the operator at a glance
//! whether their workspaces are converged, drifting, or actively
//! dirty before they kick off a job.
//!
//! # Theme
//!
//! - Nord-aurora green `#A3BE8C` when `count == 0` (clean)
//! - Nord-aurora yellow `#EBCB8B` when `count` is in `1..=5` (light)
//! - Nord-aurora red `#BF616A` when `count >= 6` (heavy)
//!
//! Matches the pleme-io Nord palette + the convention used in
//! `git_status` (yellow = ahead/dirty, red = error).
//!
//! # Probe budget
//!
//! Subprocesses out to `tend status` and parses the plain-text
//! output (one repo per line, terminal column = state). Bounded by
//! `command_timeout_ms`. A 60s in-process cache prevents repeated
//! invocations within a single shell session; stale renders emit
//! `(stale)` next to the count.
//!
//! ## Brief vs reality
//!
//! The original brief specified `tend status --format=json`, but the
//! current tend CLI doesn't accept a `--format` flag (verified
//! against `tend 0.1.0` on 2026-05-21). The text format is stable
//! and trivially parseable: a `[XX]` tag at column 3 + the state
//! word at the end of each line. Migrating to JSON once tend grows
//! a `--format=json` flag is a one-line swap inside the module.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TendConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$count` — number of non-clean repos
    /// - `$status` — computed label: `tend: clean` when count == 0,
    ///   `tend: N dirty` otherwise.
    pub format: String,
    /// Style applied when every repo is clean. Nord-aurora green.
    pub clean_style: StyleSpec,
    /// Style applied when `count` is between 1 and `heavy_threshold - 1`.
    /// Nord-aurora yellow.
    pub light_style: StyleSpec,
    /// Style applied when `count >= heavy_threshold`. Nord-aurora red.
    pub heavy_style: StyleSpec,
    /// Inclusive lower bound for `heavy_style`. Default: 6.
    pub heavy_threshold: u32,
    /// Path to the `tend` binary. Falls back to `tend` (PATH lookup).
    pub command: String,
    /// Subprocess timeout in milliseconds. Falls back on the top-level
    /// `command_timeout_ms` when zero — defaults to 500 to leave a
    /// safety margin under the canonical 100ms scan budget.
    pub command_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous count instead of re-spawning tend. Stale
    /// renders annotate the segment with `(stale)`.
    pub cache_ttl_secs: u64,
}

impl Default for TendConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: true,
            format: "[$status]($style)".to_owned(),
            clean_style: StyleSpec::new(NordPalette::bold(&nord.aurora_green)),
            light_style: StyleSpec::new(NordPalette::bold(&nord.aurora_yellow)),
            heavy_style: StyleSpec::new(NordPalette::bold(&nord.aurora_red)),
            heavy_threshold: 6,
            command: "tend".to_owned(),
            command_timeout_ms: 500,
            cache_ttl_secs: 60,
        }
    }
}

impl TendConfig {
    /// Zero-opinion: nothing scanned, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            clean_style: StyleSpec::default(),
            light_style: StyleSpec::default(),
            heavy_style: StyleSpec::default(),
            heavy_threshold: 0,
            command: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
