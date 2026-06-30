//! Typed config for the `kindling_posture` segment.
//!
//! Pleme-io-native (Tier 3): surfaces kindling node posture from
//! `~/.config/kindling/posture.json`.
//!
//! # Theme
//!
//! - Nord-aurora green `#A3BE8C` — `ready`
//! - Nord-aurora yellow `#EBCB8B` — `provisioned`
//! - Nord-aurora orange `#D08770` — otherwise
//!
//! # Probe budget
//!
//! Filesystem read only. Gracefully absent on any failure.

use crate::palette::NordPalette;
use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindlingPostureConfig {
    pub enabled: bool,
    pub format: String,
    pub ready_style: StyleSpec,
    pub provisioned_style: StyleSpec,
    pub other_style: StyleSpec,
    /// Path to posture JSON. Relative paths resolve against `$HOME`.
    /// Empty disables the probe.
    pub posture_path: String,
}

impl Default for KindlingPostureConfig {
    fn default() -> Self {
        let nord = NordPalette::pleme();
        Self {
            enabled: false,
            format: "[$status]($style)".to_owned(),
            ready_style: StyleSpec::new(NordPalette::bold(&nord.aurora_green)),
            provisioned_style: StyleSpec::new(NordPalette::bold(&nord.aurora_yellow)),
            other_style: StyleSpec::new(NordPalette::bold(&nord.aurora_orange)),
            posture_path: ".config/kindling/posture.json".to_owned(),
        }
    }
}

impl KindlingPostureConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            ready_style: StyleSpec::default(),
            provisioned_style: StyleSpec::default(),
            other_style: StyleSpec::default(),
            posture_path: String::new(),
        }
    }
}
