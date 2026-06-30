//! Typed config for the `vigy` segment.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the local vigy daemon's reconciler count + tick rate when vigy is
//! reachable. Tells the operator at a glance how many controllers are
//! registered + how fast they're ticking — load-bearing when the
//! operator is iterating on a viggy promessa locally.
//!
//! # Theme
//!
//! Nord-frost blue `#81A1C1` — distinct from the load-bearing
//! `#88C0D0` (snowflake / nix_shell / tear), in the same frost family.
//! Vigy is fleet-control machinery; the frost-blue places it on the
//! same colour gradient as the other control surfaces (directory uses
//! the same hex).
//!
//! # Probe budget
//!
//! HTTP probe to vigy's default REST address
//! (`http://127.0.0.1:38821/reconcilers`). Bounded by
//! `probe_timeout_ms` (default 100ms — matches the canonical
//! `scan_timeout_ms` ceiling). A `cache_ttl_secs` window prevents
//! repeated probes within a shell session; stale renders annotate
//! with `(stale)`. Failures render nothing (graceful absence).

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the vigy prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VigyConfig {
    pub enabled: bool,
    /// Hostname for the vigy probe. Defaults to `127.0.0.1`.
    pub host: String,
    /// Port for the vigy probe. Defaults to `38821` (vigy's default
    /// REST listener — see `vigy --help`).
    pub port: u16,
    /// HTTP path for the reconcilers endpoint. Defaults to
    /// `/reconcilers`.
    pub path: String,
    /// Format string. Substitutions:
    /// - `$count` — registered reconciler count
    /// - `$hz` — tick rate (formatted as integer when fractional is
    ///   zero, one decimal otherwise)
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Nord-frost blue `#81A1C1`.
    pub style: StyleSpec,
    /// Per-probe timeout in milliseconds. Default 100 — matches the
    /// canonical seki `scan_timeout_ms`.
    pub probe_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous probe result.
    pub cache_ttl_secs: u64,
}

impl Default for VigyConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: false,
            host: "127.0.0.1".to_owned(),
            port: 38_821,
            path: "/reconcilers".to_owned(),
            format: "[vigy: $count @ $hz Hz]($style)".to_owned(),
            style: StyleSpec::new(NordPalette::bold(&nord.frost_steel)),
            probe_timeout_ms: 100,
            cache_ttl_secs: 30,
        }
    }
}

impl VigyConfig {
    /// Zero-opinion: nothing probed, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 0,
            path: String::new(),
            format: String::new(),
            style: StyleSpec::default(),
            probe_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
