//! Typed config for the `engenho` segment.
//!
//! Pleme-io-native (Tier 5 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the readiness of the local engenho Kubernetes runtime. Engenho is
//! pleme-io's typed Rust-native K8s control plane — when it's running
//! on the host (typically via `engenho serve` or the launchd service),
//! operators want a single-glance signal that the apiserver is
//! reachable + serving `/readyz` 200s before they reach for `kubectl`.
//!
//! # Theme
//!
//! - Nord-aurora green `#A3BE8C` when `/readyz` returns 200 (ready)
//! - Nord-aurora red `#BF616A` when the apiserver is reachable but
//!   returns non-200 (degraded — still a signal worth seeing)
//!
//! Renders nothing when the apiserver is absent / unreachable —
//! distinguishes "engenho not on this host" (segment absent) from
//! "engenho on this host but degraded" (red badge).
//!
//! # Probe budget
//!
//! Hand-rolled TCP HTTP/1.0 GET against `http://127.0.0.1:6443/readyz`
//! (or `ENGENHO_API_ADDR` env override). Bounded by
//! `scan_timeout_ms`. NO tokio (sync hot path). An in-process cache
//! ([`Self::cache_ttl_secs`]) prevents repeated probes across renders
//! in one shell session.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the engenho prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngenhoConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$status` — `engenho: ready` or `engenho: degraded`
    /// - `$state` — bare state word (`ready`/`degraded`)
    /// Starship-style `[…]($style)` markup is stripped.
    pub format: String,
    /// Style applied when `/readyz` returns 200. Nord-aurora green.
    pub ready_style: StyleSpec,
    /// Style applied when the apiserver returns non-200.
    /// Nord-aurora red.
    pub degraded_style: StyleSpec,
    /// Default apiserver bind address (host:port). Engenho's
    /// canonical port is the same 6443 k3s/k8s uses. Empty string
    /// disables the probe.
    pub addr: String,
    /// HTTP path to probe — engenho mirrors the K8s `/readyz`
    /// convention.
    pub path: String,
    /// Env var consulted for an override of [`Self::addr`]. When the
    /// var is set and non-empty its value replaces `addr` for the
    /// duration of the probe. Default: `ENGENHO_API_ADDR`.
    pub addr_env_var: String,
    /// Per-probe timeout in milliseconds (covers TCP connect + HTTP
    /// round-trip). Matches `scan_timeout_ms` ceiling per the
    /// pleme-io segment doc.
    pub scan_timeout_ms: u64,
    /// In-process cache TTL in seconds; renders within this window
    /// reuse the previous probe result. Stale renders annotate with
    /// `(stale)`.
    pub cache_ttl_secs: u64,
}

impl Default for EngenhoConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            // Tier 5 default: disabled (probe cost > 0; operator
            // opts in via SEKI_TIER or per-segment toggle).
            enabled: false,
            format: "[$status]($style)".to_owned(),
            ready_style: StyleSpec::new(NordPalette::bold(&nord.aurora_green)),
            degraded_style: StyleSpec::new(NordPalette::bold(&nord.aurora_red)),
            addr: "127.0.0.1:6443".to_owned(),
            path: "/readyz".to_owned(),
            addr_env_var: "ENGENHO_API_ADDR".to_owned(),
            scan_timeout_ms: 100,
            cache_ttl_secs: 30,
        }
    }
}

impl EngenhoConfig {
    /// Zero-opinion: nothing probed, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            ready_style: StyleSpec::default(),
            degraded_style: StyleSpec::default(),
            addr: String::new(),
            path: String::new(),
            addr_env_var: String::new(),
            scan_timeout_ms: 0,
            cache_ttl_secs: 0,
        }
    }
}
