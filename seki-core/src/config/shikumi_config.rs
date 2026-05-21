//! Typed config for the `shikumi_config` segment.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Sister
//! segment to [`super::shikumi_tier`]: where `shikumi_tier` reports
//! that an `<APP>_TIER` env var is *set*, this segment probes
//! `<app> config-show <tier>` to confirm tier resolution *actually
//! works*. The two surfaces differ — an env var can be set without
//! the binary having shipped the matching tier (typo / stale env /
//! missing binary), so the validated signal is its own segment.
//!
//! # Theme
//!
//! Nord-aurora green `#A3BE8C` — the validated-tier signal mirrors
//! `tend`'s clean state: "the operator's setup is converged." Renders
//! nothing when the binary is missing or the probe fails (no red
//! alarm — the unvalidated state is the absence of the segment, the
//! green badge is the affirmative presence).
//!
//! # Probe budget
//!
//! Subprocesses out to `<app> config-show <tier>` per resolved app +
//! tier from the environment. Bounded by `command_timeout_ms` (per
//! probe). A 60s in-process cache (per [`ShikumiConfigConfig`])
//! prevents repeated invocations within a shell session; stale
//! renders annotate with `(stale)`.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the shikumi_config prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShikumiConfigConfig {
    pub enabled: bool,
    /// Apps to probe. Each entry's `<APP>_TIER` env var is read; if
    /// set, the matching `<app> config-show <tier>` is invoked and
    /// the segment renders when the probe succeeds. Default = the
    /// same shikumi catalogue used by `shikumi_tier` (matches the
    /// audit dated 2026-05-20).
    pub apps: Vec<String>,
    /// Format string. Substitutions:
    /// - `$app` — lowercase app key
    /// - `$tier` — resolved tier (bare/default/discovered/custom)
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Nord-aurora green
    /// `#A3BE8C` (validated-tier badge).
    pub style: StyleSpec,
    /// Separator joining multiple validated `(app, tier)` pairs.
    pub separator: String,
    /// Per-probe timeout in milliseconds. Defaults to 200 to leave
    /// headroom for several probes inside one 1s `scan_timeout_ms`.
    pub command_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous probe result. Stale renders annotate with
    /// `(stale)`.
    pub cache_ttl_secs: u64,
}

impl Default for ShikumiConfigConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            apps: super::shikumi_tier::default_apps(),
            format: "[$app:$tier]($style)".to_owned(),
            style: StyleSpec::new("bold #A3BE8C"),
            separator: " ".to_owned(),
            command_timeout_ms: 200,
            cache_ttl_secs: 60,
        }
    }
}

impl ShikumiConfigConfig {
    /// Zero-opinion: nothing probed, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            apps: Vec::new(),
            format: String::new(),
            style: StyleSpec::default(),
            separator: String::new(),
            command_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
