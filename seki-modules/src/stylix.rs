//! `stylix` segment — surfaces the active stylix base16 scheme name.
//!
//! Pleme-io-native (Tier 4 — substrate-themed). Reads
//! `~/.config/stylix.json` if present; otherwise falls back to the
//! `STYLIX_BASE16_SCHEME` env var. Either present → renders the
//! scheme name in Nord-frost cyan.
//!
//! ## Theme
//!
//! Nord-frost cyan `#88C0D0` by default — the "design system /
//! theme" accent the rest of the substrate uses.
//!
//! ## Probe budget
//!
//! Filesystem read + env-var lookup only — no subprocess. Bounded
//! well under any reasonable `scan_timeout_ms`. JSON parsing is
//! delegated to `seki_core::config::stylix::read_manifest_scheme`
//! (which owns the typed `StylixManifest` shape + `serde_json`
//! dep — keeps seki-modules pure-data).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::stylix::{StylixConfig, read_manifest_scheme},
    segment::StyledFragment,
};
use std::path::{Path, PathBuf};

pub struct StylixModule {
    cfg: StylixConfig,
}

impl StylixModule {
    pub fn new(cfg: StylixConfig) -> Self {
        Self { cfg }
    }
}

impl Module for StylixModule {
    fn name(&self) -> &'static str {
        "stylix"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(scheme) = resolve_scheme(
            ctx.home.as_deref(),
            &self.cfg.config_path,
            &self.cfg.env_var,
            env_lookup,
        ) else {
            return Ok(None);
        };
        let text = render_format(&self.cfg.format, &scheme);
        Ok(Some(Segment::new("stylix").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Pure resolver — exposed for testability. Order of precedence:
///
/// 1. JSON manifest at `<home>/<config_path>` (only consulted if
///    `home` is `Some` AND the file is readable AND its
///    `base16_scheme` field is non-empty).
/// 2. Env-var `env_var` (via the supplied lookup; production passes
///    [`env_lookup`], tests pass stubs).
///
/// Returns `None` when neither source produces a non-empty name.
pub fn resolve_scheme<F>(
    home: Option<&Path>,
    config_path: &str,
    env_var: &str,
    lookup: F,
) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(home) = home {
        let path: PathBuf = home.join(config_path);
        if let Some(name) = read_manifest_scheme(&path) {
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    let v = lookup(env_var)?;
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Render the format string. Substitutions: `$name`. Starship-style
/// `[…]($style)` markup stripped. Mirrors `shikumi_tier::render_format`.
pub fn render_format(fmt: &str, name: &str) -> String {
    let mut out = String::with_capacity(fmt.len());
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut id = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    id.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            match id.as_str() {
                "name" => out.push_str(name),
                _ => {}
            }
        } else if c == '[' || c == ']' {
        } else if c == '(' {
            let mut depth = 1;
            for n in chars.by_ref() {
                if n == '(' {
                    depth += 1;
                } else if n == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
        } else if c == '\\' {
            if let Some(&n) = chars.peek() {
                out.push(n);
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-stylix-mod-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn stub_lookup(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).map(|s| (*s).to_owned())
    }

    #[test]
    fn manifest_takes_precedence_over_env() {
        let dir = tmp_dir("manifest-wins");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(
            cfg_dir.join("stylix.json"),
            r#"{"base16_scheme":"nord","unused":"x"}"#,
        )
        .unwrap();
        let lookup = stub_lookup(HashMap::from([("STYLIX_BASE16_SCHEME", "gruvbox-dark-medium")]));
        let resolved = resolve_scheme(
            Some(&dir),
            ".config/stylix.json",
            "STYLIX_BASE16_SCHEME",
            lookup,
        );
        assert_eq!(resolved.as_deref(), Some("nord"));
    }

    #[test]
    fn env_var_used_when_manifest_absent() {
        let dir = tmp_dir("env-fallback");
        let lookup = stub_lookup(HashMap::from([("STYLIX_BASE16_SCHEME", "tomorrow-night")]));
        let resolved = resolve_scheme(
            Some(&dir),
            ".config/stylix.json",
            "STYLIX_BASE16_SCHEME",
            lookup,
        );
        assert_eq!(resolved.as_deref(), Some("tomorrow-night"));
    }

    #[test]
    fn neither_present_returns_none() {
        let dir = tmp_dir("neither");
        let lookup = stub_lookup(HashMap::new());
        let resolved = resolve_scheme(
            Some(&dir),
            ".config/stylix.json",
            "STYLIX_BASE16_SCHEME",
            lookup,
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn missing_home_falls_back_to_env() {
        let lookup = stub_lookup(HashMap::from([("STYLIX_BASE16_SCHEME", "nord")]));
        let resolved = resolve_scheme(None, ".config/stylix.json", "STYLIX_BASE16_SCHEME", lookup);
        assert_eq!(resolved.as_deref(), Some("nord"));
    }

    #[test]
    fn malformed_json_falls_back_to_env() {
        let dir = tmp_dir("malformed");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(cfg_dir.join("stylix.json"), "{not json").unwrap();
        let lookup = stub_lookup(HashMap::from([("STYLIX_BASE16_SCHEME", "tokyonight")]));
        let resolved = resolve_scheme(
            Some(&dir),
            ".config/stylix.json",
            "STYLIX_BASE16_SCHEME",
            lookup,
        );
        assert_eq!(resolved.as_deref(), Some("tokyonight"));
    }

    #[test]
    fn render_format_name_substitution() {
        let out = render_format("[stylix: $name]($style)", "nord");
        assert_eq!(out, "stylix: nord");
    }

    #[test]
    fn renders_segment_for_present_manifest() {
        let dir = tmp_dir("render");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(cfg_dir.join("stylix.json"), r#"{"base16_scheme":"nord"}"#).unwrap();
        let mut cfg = StylixConfig::default();
        cfg.enabled = true;
        let module = StylixModule::new(cfg);
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "stylix");
        assert_eq!(seg.fragments[0].text, "stylix: nord");
    }

    #[test]
    fn renders_nothing_when_neither_source_present() {
        let dir = tmp_dir("absent");
        let mut cfg = StylixConfig::default();
        cfg.enabled = true;
        cfg.env_var = "SEKI_TEST_STYLIX_DEFINITELY_UNSET".to_owned();
        let module = StylixModule::new(cfg);
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
