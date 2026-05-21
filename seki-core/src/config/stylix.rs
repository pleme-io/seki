//! Typed config for the `stylix` segment.
//!
//! Pleme-io-native (Tier 4 — substrate-themed): surfaces the active
//! stylix base16 scheme name in the prompt. Stylix is the upstream
//! NixOS theming framework many pleme-io operators chain through; the
//! prompt-side echo confirms which palette the host is rendering
//! against right now.
//!
//! # Data sources (in precedence order)
//!
//! 1. `~/.config/stylix.json` — typed shape `{"base16_scheme": "…"}`
//! 2. `STYLIX_BASE16_SCHEME` env var
//!
//! Either present → segment renders. Neither → segment is silently
//! absent (operators on non-stylix hosts see nothing, never an
//! error).
//!
//! # Theme
//!
//! Nord-frost cyan `#88C0D0` — the load-bearing pleme-io frost colour
//! the rest of the substrate uses for "design system / theme" facts.
//! Matches the snowflake glyph and `caixa` accent so the operator
//! sees a coherent palette.
//!
//! # Probe budget
//!
//! Filesystem read + env-var lookup only — no subprocess. Both
//! checks are well under any reasonable `scan_timeout_ms`.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StylixConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$name` — resolved base16 scheme name (e.g. `nord`,
    ///   `gruvbox-dark-medium`)
    ///
    /// Starship-style `[…]($style)` markup is stripped; the
    /// renderer applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text. Defaults to Nord-frost
    /// cyan `#88C0D0`.
    pub style: StyleSpec,
    /// Path to the stylix config file, relative to the operator's
    /// `$HOME`. Default `.config/stylix.json`.
    pub config_path: String,
    /// Env var to fall back on when the config file is absent.
    /// Default `STYLIX_BASE16_SCHEME`.
    pub env_var: String,
}

impl Default for StylixConfig {
    fn default() -> Self {
        Self {
            // Tier 4 default = operator opt-in. Stylix is optional
            // on the fleet — flipping enabled=true is a one-line
            // operator decision in their seki.yaml.
            enabled: false,
            format: "[stylix: $name]($style)".to_owned(),
            style: StyleSpec::new("bold #88C0D0"),
            config_path: ".config/stylix.json".to_owned(),
            env_var: "STYLIX_BASE16_SCHEME".to_owned(),
        }
    }
}

impl StylixConfig {
    /// Zero-opinion: nothing scanned, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
            config_path: String::new(),
            env_var: String::new(),
        }
    }
}

/// Typed shape of `~/.config/stylix.json`. Only the `base16_scheme`
/// field is load-bearing; the rest of the document is ignored (stylix
/// emits a much larger object — we only care about the scheme name).
///
/// Tolerant of unknown fields: deserializing the partial shape is
/// fine because `serde_json` doesn't reject extras by default.
#[derive(Debug, Clone, Deserialize)]
pub struct StylixManifest {
    pub base16_scheme: String,
}

/// Read + parse the stylix manifest at `path`. Returns `None` for
/// missing files, unreadable files, or files whose JSON doesn't
/// contain a `base16_scheme` field.
///
/// Lives in seki-core because parsing pulls `serde_json` (already a
/// seki-core dep) — seki-modules stays a pure-data crate.
pub fn read_manifest_scheme(path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    let parsed: StylixManifest = serde_json::from_str(&body).ok()?;
    Some(parsed.base16_scheme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-stylix-cfg-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn default_is_disabled() {
        // Tier 4 default = operator opt-in.
        assert!(!StylixConfig::default().enabled);
    }

    #[test]
    fn default_uses_nord_frost_cyan() {
        assert_eq!(StylixConfig::default().style.as_str(), "bold #88C0D0");
    }

    #[test]
    fn bare_is_empty() {
        let cfg = StylixConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.config_path, "");
        assert_eq!(cfg.env_var, "");
    }

    #[test]
    fn read_manifest_scheme_parses_present_field() {
        let dir = tmp_dir("manifest");
        let path = dir.join("stylix.json");
        fs::write(&path, r#"{"base16_scheme":"nord","unused":"x"}"#).unwrap();
        assert_eq!(read_manifest_scheme(&path).as_deref(), Some("nord"));
    }

    #[test]
    fn read_manifest_scheme_returns_none_for_missing() {
        let dir = tmp_dir("missing");
        let path = dir.join("absent.json");
        assert!(read_manifest_scheme(&path).is_none());
    }

    #[test]
    fn read_manifest_scheme_returns_none_for_malformed() {
        let dir = tmp_dir("malformed");
        let path = dir.join("stylix.json");
        fs::write(&path, "{not json").unwrap();
        assert!(read_manifest_scheme(&path).is_none());
    }

    #[test]
    fn read_manifest_scheme_returns_none_for_missing_field() {
        let dir = tmp_dir("no-field");
        let path = dir.join("stylix.json");
        fs::write(&path, r#"{"other":"x"}"#).unwrap();
        assert!(read_manifest_scheme(&path).is_none());
    }
}
