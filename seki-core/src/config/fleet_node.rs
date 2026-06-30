//! Typed config for the `fleet_node` segment.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the current node's identity + cluster role as recorded in
//! `~/.config/kindling/node.yaml` (the canonical kindling node
//! manifest). Tells the operator at a glance which fleet node + which
//! cluster role they're driving — load-bearing when SSHing across
//! the fleet to a host whose hostname doesn't encode the role.
//!
//! # Theme
//!
//! Nord-snow `#D8DEE9` dimmed — quiet, persistent context (not a
//! status that changes session-to-session, more like a passport
//! stamp). Per the catalog doc, fleet-wide truth uses the snowflake;
//! this segment composes one-line of dimmed snow text as the
//! understated companion.
//!
//! # Probe budget
//!
//! Filesystem read only (`~/.config/kindling/node.yaml`). Bypasses
//! `scan_timeout_ms`. The read is cheap (one stat + one parse); we
//! cache the parsed shape across renders so re-parsing is amortised.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the fleet_node prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetNodeConfig {
    pub enabled: bool,
    /// Path to the kindling node manifest, relative to `$HOME`.
    /// Defaults to `.config/kindling/node.yaml` per the canonical
    /// blackmatter-kubernetes layout.
    pub manifest_path: String,
    /// Format string. Substitutions:
    /// - `$node` — `node_name` field
    /// - `$cluster` — `cluster` field
    /// - `$role` — `role` field
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Nord-snow dimmed.
    pub style: StyleSpec,
}

impl Default for FleetNodeConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: false,
            manifest_path: ".config/kindling/node.yaml".to_owned(),
            format: "[$node/$cluster]($style)".to_owned(),
            style: StyleSpec::new(NordPalette::dimmed(&nord.snow_dim)),
        }
    }
}

impl FleetNodeConfig {
    /// Zero-opinion: nothing read, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            manifest_path: String::new(),
            format: String::new(),
            style: StyleSpec::default(),
        }
    }
}
