//! Typed config for the `shikumi_tier` segment.
//!
//! Pleme-io-native: shows the active `<APP>_TIER` for any shikumi
//! consumer in the operator's environment. Tells the operator at
//! a glance which tier the current shell session is running under.
//!
//! Example: an operator running `KENSHI_TIER=bare frostmourne` sees
//! `[kenshi:bare]` in their prompt, instantly confirming the tier
//! override is in effect.
//!
//! # Why this lives in seki (not blzsh)
//!
//! Every shikumi consumer (the 18 Rust pleme-io apps) reads an
//! `<APP>_TIER` env var. Surfacing that in the prompt is the
//! standard operator visibility move — but with upstream starship
//! it required hand-writing a custom command per app. With seki's
//! typed-segment model, it's one module that scans every known
//! shikumi-app env var and emits a compact summary.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Tier surface visible in the prompt. Strings the renderer
/// interpolates via `$tier` / `$app`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShikumiTierConfig {
    pub enabled: bool,
    /// Apps to scan. Each entry's `<APP>_TIER` env var is checked
    /// and surfaced when set. Default = the full pleme-io shikumi
    /// catalogue (matches the audit at
    /// `~/.claude/projects/.../memory/project_shikumi_adoption_audit.md`).
    pub apps: Vec<String>,
    /// Format string. Substitutions: `$app` (lowercase app key),
    /// `$tier` (resolved tier — bare/default/discovered/custom).
    /// Style markup `[…](…)` is stripped (the renderer applies
    /// `style` directly).
    pub format: String,
    /// Style applied to the rendered text.
    pub style: StyleSpec,
    /// When multiple `<APP>_TIER` env vars are set simultaneously,
    /// separate them with this string. Default: `" "`.
    pub separator: String,
}

impl Default for ShikumiTierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            apps: default_apps(),
            format: "[$app:$tier]($style)".to_owned(),
            style: StyleSpec::new("dimmed yellow"),
            separator: " ".to_owned(),
        }
    }
}

impl ShikumiTierConfig {
    /// Zero-opinion: nothing scanned, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            apps: Vec::new(),
            format: String::new(),
            style: StyleSpec::default(),
            separator: String::new(),
        }
    }
}

/// The canonical pleme-io shikumi app catalogue — every Rust app
/// that implements `shikumi::TieredConfig` per the audit dated
/// 2026-05-20. Reading their `<APP>_TIER` env vars is the way
/// operators verify "which tier am I running" at a glance.
///
/// Adding a new shikumi app to this list is the only edit needed
/// to surface its tier in every operator's prompt.
pub fn default_apps() -> Vec<String> {
    [
        "mado",
        "tatara",
        "zoekt-mcp",
        "kindling",
        "ayatsuri",
        "kenshi",
        "taimen",
        "tear",
        "escriba",
        "namimado",
        "kurage",
        "hikki",
        "hibiki",
        "hikyaku",
        "fumi",
        "kekkai",
        "tend",
        "kikai",
        "hashfix",
        "seki",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}
