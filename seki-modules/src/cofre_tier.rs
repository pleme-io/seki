//! `cofre_tier` segment — surfaces cofre secret backend.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Reads
//! the typed cofre manifest at `~/.config/cofre/cofre.yaml` and
//! renders the active backend (akeyless / sops / mock). Tells the
//! operator at a glance which secret materialization tier is bound.
//!
//! ## Theme
//!
//! Nord-aurora yellow `#EBCB8B` — "tier in effect" colour.
//!
//! ## Probe budget
//!
//! Filesystem read only — bypasses `scan_timeout_ms`. Same
//! path-traversal guard as `fleet_node`.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::cofre_tier::CofreTierConfig,
    segment::StyledFragment,
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Mutex;

/// Typed cofre manifest — only the `backend` field is deserialized.
#[derive(Debug, Clone, Deserialize)]
pub struct CofreManifest {
    pub backend: String,
}

pub struct CofreTierModule {
    cfg: CofreTierConfig,
    cache: Mutex<Option<CofreManifest>>,
}

impl CofreTierModule {
    pub fn new(cfg: CofreTierConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for CofreTierModule {
    fn name(&self) -> &'static str {
        "cofre_tier"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let cached = self.cache.lock().ok().and_then(|g| g.clone());
        let manifest = match cached {
            Some(m) => m,
            None => {
                let home = match ctx.home.as_ref() {
                    Some(h) => h,
                    None => return Ok(None),
                };
                let path = resolve_manifest_path(home, &self.cfg.manifest_path);
                match load_manifest(&path) {
                    Some(m) => {
                        if let Ok(mut g) = self.cache.lock() {
                            *g = Some(m.clone());
                        }
                        m
                    }
                    None => return Ok(None),
                }
            }
        };
        let text = render_format(&self.cfg.format, &manifest.backend);
        Ok(Some(
            Segment::new("cofre_tier").push(StyledFragment::new(text, self.cfg.style.resolve())),
        ))
    }
}

/// Join `$HOME` with the manifest path. Mirrors
/// `fleet_node::resolve_manifest_path` — guards against `..` and
/// passes absolute paths through.
pub fn resolve_manifest_path(home: &std::path::Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        return p;
    }
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return home.join("__rejected_traversal__");
    }
    home.join(p)
}

pub fn load_manifest(path: &std::path::Path) -> Option<CofreManifest> {
    let body = std::fs::read_to_string(path).ok()?;
    parse_manifest(&body)
}

pub fn parse_manifest(body: &str) -> Option<CofreManifest> {
    serde_yaml::from_str(body).ok()
}

/// Render the format string. Substitutions: `$backend`.
/// Mirrors `shikumi_tier::render_format` field-for-field.
pub fn render_format(fmt: &str, backend: &str) -> String {
    let mut out = String::with_capacity(fmt.len());
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut name = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    name.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            match name.as_str() {
                "backend" => out.push_str(backend),
                _ => {}
            }
        } else if c == '[' || c == ']' {
            // strip starship markup
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
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-cofre-tier-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parses_typed_manifest() {
        let body = "backend: akeyless\n";
        let m = parse_manifest(body).unwrap();
        assert_eq!(m.backend, "akeyless");
    }

    #[test]
    fn parses_extra_fields_ignored() {
        let body = "backend: sops\nbase_dir: /tmp/cofre\n";
        let m = parse_manifest(body).unwrap();
        assert_eq!(m.backend, "sops");
    }

    #[test]
    fn parses_garbage_returns_none() {
        assert!(parse_manifest("::not yaml").is_none());
    }

    #[test]
    fn render_format_substitutes_backend() {
        let out = render_format("[cofre: $backend]($style)", "akeyless");
        assert_eq!(out, "cofre: akeyless");
    }

    #[test]
    fn resolve_manifest_path_rejects_traversal() {
        let home = PathBuf::from("/home/op");
        let p = resolve_manifest_path(&home, "../etc/cofre.yaml");
        assert!(p.to_str().unwrap().contains("__rejected_traversal__"));
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = CofreTierConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.manifest_path, "");
    }

    #[test]
    fn default_uses_nord_aurora_yellow() {
        let cfg = CofreTierConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #EBCB8B");
        assert_eq!(cfg.manifest_path, ".config/cofre/cofre.yaml");
    }

    #[test]
    fn renders_segment_from_present_manifest() {
        let dir = tmp_dir("present");
        let cfg_dir = dir.join(".config").join("cofre");
        fs::create_dir_all(&cfg_dir).unwrap();
        fs::write(cfg_dir.join("cofre.yaml"), "backend: akeyless\n").unwrap();
        let module = CofreTierModule::new(CofreTierConfig {
            enabled: true,
            ..CofreTierConfig::default()
        });
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "cofre_tier");
        assert_eq!(seg.fragments[0].text, "cofre: akeyless");
    }

    #[test]
    fn renders_nothing_when_manifest_missing() {
        let dir = tmp_dir("missing");
        let module = CofreTierModule::new(CofreTierConfig {
            enabled: true,
            ..CofreTierConfig::default()
        });
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
