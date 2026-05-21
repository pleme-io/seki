//! Typed config for the `blackmatter` segment.
//!
//! Pleme-io-native (Tier 4 — substrate-themed): counts the number of
//! enabled blackmatter components on this host and surfaces it in the
//! prompt. Blackmatter is the pleme-io fleet's home-manager module
//! aggregator; the count tells the operator at a glance how
//! "instrumented" their shell session is with fleet primitives.
//!
//! # Data sources (in precedence order)
//!
//! 1. `~/.config/blackmatter/enabled-components.json` — typed shape
//!    `{ "components": ["mado", "tatara", "kindling", …] }`. Authored
//!    by the blackmatter HM module at activation time.
//! 2. Heuristic fallback — scan `$XDG_CONFIG_HOME` (default
//!    `~/.config`) for known blackmatter HM module directories
//!    (`mado/`, `tatara/`, `kindling/`, …) and count those.
//!
//! Either present → segment renders. Neither → segment is silently
//! absent (operators on non-blackmatter hosts see nothing).
//!
//! # Theme
//!
//! Nord-aurora green `#A3BE8C` — the "ok / clean / instrumented"
//! signal. Matches the `tend` clean state + `git_status` clean
//! convention; semantics here are "the fleet substrate is loaded and
//! ready".
//!
//! # Probe budget
//!
//! Filesystem reads only — no subprocess, no network. The JSON
//! parse is a tiny serde call; the heuristic fallback is a single
//! `read_dir` over `$XDG_CONFIG_HOME`. Both bounded well under any
//! `scan_timeout_ms`.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlackmatterConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$count` — number of enabled blackmatter components
    ///
    /// Starship-style `[…]($style)` markup is stripped; the
    /// renderer applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Defaults to Nord-aurora
    /// green `#A3BE8C`.
    pub style: StyleSpec,
    /// Path to the blackmatter enabled-components manifest, relative
    /// to `$HOME`. Default `.config/blackmatter/enabled-components.json`.
    pub manifest_path: String,
    /// Known blackmatter HM module names for the heuristic
    /// fallback — directories of these names under `$XDG_CONFIG_HOME`
    /// count as "enabled" when the JSON manifest is absent. Keep in
    /// sync with the blackmatter sub-repo registry.
    pub known_components: Vec<String>,
}

impl Default for BlackmatterConfig {
    fn default() -> Self {
        Self {
            // Tier 4 default = operator opt-in. Blackmatter is the
            // load-bearing pleme-io HM aggregator but the count is
            // an "fleet health" cue, not a critical-path signal.
            enabled: false,
            format: "[bm: $count]($style)".to_owned(),
            style: StyleSpec::new("bold #A3BE8C"),
            manifest_path: ".config/blackmatter/enabled-components.json".to_owned(),
            known_components: default_known_components(),
        }
    }
}

impl BlackmatterConfig {
    /// Zero-opinion: nothing scanned, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
            manifest_path: String::new(),
            known_components: Vec::new(),
        }
    }
}

/// Known blackmatter component HM module directory names — used by
/// the heuristic fallback when the JSON manifest is missing. Mirrors
/// the blackmatter sub-repo registry (mado, tatara, kindling,
/// frostmourne, ayatsuri, …).
///
/// Adding a new blackmatter HM module to this list is the only edit
/// needed to surface its presence in every operator's prompt count.
pub fn default_known_components() -> Vec<String> {
    [
        "mado",
        "tatara",
        "kindling",
        "frostmourne",
        "ayatsuri",
        "tear",
        "frost",
        "seki",
        "ghostty",
        "tend",
        "kikai",
        "kurage",
        "kenshi",
        "taimen",
        "zoekt-mcp",
        "codesearch",
        "curupira",
        "namimado",
        "hibiki",
        "hikki",
        "hikyaku",
        "escriba",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// Typed shape of `~/.config/blackmatter/enabled-components.json` —
/// the canonical manifest written by the blackmatter HM module at
/// activation time. Lives in seki-core because the parse pulls
/// `serde_json` (already a seki-core dep) — seki-modules stays
/// pure-data.
#[derive(Debug, Clone, Deserialize)]
pub struct BlackmatterManifest {
    pub components: Vec<String>,
}

/// Read + parse the manifest at `path`. Returns the explicit
/// component count on success, or `None` if the file is missing,
/// unreadable, or malformed (the module impl falls back to the
/// filesystem heuristic in that case).
pub fn read_manifest_count(path: &Path) -> Option<usize> {
    let body = std::fs::read_to_string(path).ok()?;
    let parsed: BlackmatterManifest = serde_json::from_str(&body).ok()?;
    Some(parsed.components.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-bm-cfg-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn default_is_disabled() {
        assert!(!BlackmatterConfig::default().enabled);
    }

    #[test]
    fn default_uses_nord_aurora_green() {
        assert_eq!(
            BlackmatterConfig::default().style.as_str(),
            "bold #A3BE8C"
        );
    }

    #[test]
    fn default_known_components_includes_mado() {
        let known = default_known_components();
        assert!(known.iter().any(|s| s == "mado"));
        assert!(known.iter().any(|s| s == "tatara"));
        assert!(known.iter().any(|s| s == "kindling"));
    }

    #[test]
    fn bare_is_empty() {
        let cfg = BlackmatterConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.manifest_path, "");
        assert!(cfg.known_components.is_empty());
    }

    #[test]
    fn read_manifest_count_returns_count() {
        let dir = tmp_dir("present");
        let path = dir.join("enabled-components.json");
        fs::write(
            &path,
            r#"{"components":["mado","tatara","kindling","frostmourne"]}"#,
        )
        .unwrap();
        assert_eq!(read_manifest_count(&path), Some(4));
    }

    #[test]
    fn read_manifest_count_returns_zero_for_empty_list() {
        let dir = tmp_dir("empty");
        let path = dir.join("enabled-components.json");
        fs::write(&path, r#"{"components":[]}"#).unwrap();
        assert_eq!(read_manifest_count(&path), Some(0));
    }

    #[test]
    fn read_manifest_count_returns_none_for_missing() {
        let dir = tmp_dir("missing");
        let path = dir.join("nope.json");
        assert!(read_manifest_count(&path).is_none());
    }

    #[test]
    fn read_manifest_count_returns_none_for_malformed() {
        let dir = tmp_dir("malformed");
        let path = dir.join("manifest.json");
        fs::write(&path, "{not json").unwrap();
        assert!(read_manifest_count(&path).is_none());
    }
}
