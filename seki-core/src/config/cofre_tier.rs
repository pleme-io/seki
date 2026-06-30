//! Typed config for the `cofre_tier` segment.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Surfaces
//! the cofre secret backend the operator is currently bound to
//! (akeyless / sops / mock). Tells the operator at a glance which
//! materialization tier they're driving — load-bearing when
//! `cofre.yaml` flips the backend without a process restart.
//!
//! # Theme
//!
//! Nord-aurora yellow `#EBCB8B` — the canonical "tier in effect"
//! colour, mirroring the `shikumi_tier` default. Secret backend is a
//! tier of trust; yellow signals "this is a context the operator
//! should be aware of."
//!
//! # Probe budget
//!
//! Filesystem read only (`~/.config/cofre/cofre.yaml`). Bypasses
//! `scan_timeout_ms`.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the cofre_tier prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CofreTierConfig {
    pub enabled: bool,
    /// Path to the cofre manifest, relative to `$HOME`. Defaults to
    /// `.config/cofre/cofre.yaml` per the cofre repo's canonical
    /// layout.
    pub manifest_path: String,
    /// Format string. Substitutions:
    /// - `$backend` — `backend` field (akeyless / sops / mock)
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Nord-aurora yellow
    /// `#EBCB8B`.
    pub style: StyleSpec,
}

impl Default for CofreTierConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: false,
            manifest_path: ".config/cofre/cofre.yaml".to_owned(),
            format: "[cofre: $backend]($style)".to_owned(),
            style: StyleSpec::new(NordPalette::bold(&nord.aurora_yellow)),
        }
    }
}

impl CofreTierConfig {
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
