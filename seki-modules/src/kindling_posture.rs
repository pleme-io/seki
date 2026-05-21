//! `kindling_posture` segment — surfaces local kindling node posture.
//!
//! Pleme-io-native (Tier 3). Reads `~/.config/kindling/posture.json`
//! and surfaces the `posture_level` field (bootstrap / seeded /
//! provisioned / ready).
//!
//! ## Theme
//!
//! - Nord-aurora green `#A3BE8C` — `ready`
//! - Nord-aurora yellow `#EBCB8B` — `provisioned`
//! - Nord-aurora orange `#D08770` — otherwise
//!
//! ## Probe budget
//!
//! Filesystem read only. Gracefully absent on any failure.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::kindling_posture::KindlingPostureConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::path::{Path, PathBuf};

pub struct KindlingPostureModule {
    cfg: KindlingPostureConfig,
}

impl KindlingPostureModule {
    pub fn new(cfg: KindlingPostureConfig) -> Self {
        Self { cfg }
    }
}

impl Module for KindlingPostureModule {
    fn name(&self) -> &'static str {
        "kindling_posture"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let Some(path) = resolve_posture_path(&self.cfg.posture_path, ctx.home.as_deref()) else {
            return Ok(None);
        };
        let Some(level) = read_posture_level(&path) else {
            return Ok(None);
        };
        let style = pick_style(&self.cfg, &level);
        let status_label = format_status(&level);
        let text = render_format(&self.cfg.format, &level, &status_label);
        Ok(Some(
            Segment::new("kindling_posture").push(StyledFragment::new(text, style)),
        ))
    }
}

/// Resolve the configured posture path against `$HOME` when relative.
pub fn resolve_posture_path(posture_path: &str, home: Option<&Path>) -> Option<PathBuf> {
    if posture_path.is_empty() {
        return None;
    }
    let p = Path::new(posture_path);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    let home = home?;
    Some(home.join(p))
}

pub fn read_posture_level(path: &Path) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;
    parse_posture_level(&body)
}

/// Scan JSON body for `"posture_level": "<value>"`. Returns the value
/// only if it's one of the four typed levels.
pub fn parse_posture_level(body: &str) -> Option<String> {
    let key = "\"posture_level\"";
    let idx = body.find(key)?;
    let after = &body[idx + key.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?;
    let after = after.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    let value = &after[..end];
    if RECOGNISED_LEVELS.contains(&value) {
        Some(value.to_owned())
    } else {
        None
    }
}

const RECOGNISED_LEVELS: &[&str] = &["bootstrap", "seeded", "provisioned", "ready"];

pub fn pick_style(cfg: &KindlingPostureConfig, level: &str) -> Style {
    let spec: &StyleSpec = match level {
        "ready" => &cfg.ready_style,
        "provisioned" => &cfg.provisioned_style,
        _ => &cfg.other_style,
    };
    spec.resolve()
}

pub fn format_status(level: &str) -> String {
    let mut s = String::from("kindling: ");
    s.push_str(level);
    s
}

pub fn render_format(fmt: &str, level: &str, status: &str) -> String {
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
                "level" => out.push_str(level),
                "status" => out.push_str(status),
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
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("seki-kindling-posture-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parse_posture_level_ready() {
        let body = r#"{"posture_level": "ready", "other": 1}"#;
        assert_eq!(parse_posture_level(body), Some("ready".to_owned()));
    }

    #[test]
    fn parse_posture_level_provisioned() {
        let body = r#"{"posture_level":"provisioned"}"#;
        assert_eq!(parse_posture_level(body), Some("provisioned".to_owned()));
    }

    #[test]
    fn parse_posture_level_bootstrap_and_seeded() {
        assert_eq!(
            parse_posture_level(r#"{"posture_level":"bootstrap"}"#),
            Some("bootstrap".to_owned())
        );
        assert_eq!(
            parse_posture_level(r#"{"posture_level":"seeded"}"#),
            Some("seeded".to_owned())
        );
    }

    #[test]
    fn parse_posture_level_rejects_unknown() {
        let body = r#"{"posture_level":"draconic"}"#;
        assert_eq!(parse_posture_level(body), None);
    }

    #[test]
    fn parse_posture_level_absent_returns_none() {
        let body = r#"{"other": 1}"#;
        assert_eq!(parse_posture_level(body), None);
    }

    #[test]
    fn pick_style_ready_uses_green() {
        let cfg = KindlingPostureConfig::default();
        assert_eq!(pick_style(&cfg, "ready"), cfg.ready_style.resolve());
    }

    #[test]
    fn pick_style_provisioned_uses_yellow() {
        let cfg = KindlingPostureConfig::default();
        assert_eq!(
            pick_style(&cfg, "provisioned"),
            cfg.provisioned_style.resolve()
        );
    }

    #[test]
    fn pick_style_other_uses_orange() {
        let cfg = KindlingPostureConfig::default();
        assert_eq!(pick_style(&cfg, "bootstrap"), cfg.other_style.resolve());
        assert_eq!(pick_style(&cfg, "seeded"), cfg.other_style.resolve());
    }

    #[test]
    fn resolve_posture_path_relative_uses_home() {
        let home = PathBuf::from("/u/luis");
        let r = resolve_posture_path(".config/kindling/posture.json", Some(&home));
        assert_eq!(
            r,
            Some(PathBuf::from("/u/luis/.config/kindling/posture.json"))
        );
    }

    #[test]
    fn resolve_posture_path_absolute_passthrough() {
        let r = resolve_posture_path("/etc/kindling/posture.json", None);
        assert_eq!(r, Some(PathBuf::from("/etc/kindling/posture.json")));
    }

    #[test]
    fn resolve_posture_path_empty_disables() {
        let r = resolve_posture_path("", Some(Path::new("/u/luis")));
        assert_eq!(r, None);
    }

    #[test]
    fn format_status_label() {
        assert_eq!(format_status("ready"), "kindling: ready");
    }

    #[test]
    fn render_format_level_substitution() {
        let out = render_format("[$status]($style)", "ready", "kindling: ready");
        assert_eq!(out, "kindling: ready");
    }

    #[test]
    fn renders_segment_when_posture_present() {
        let dir = tmp_dir("present");
        fs::write(
            dir.join("posture.json"),
            r#"{"posture_level": "ready", "node":"cid"}"#,
        )
        .unwrap();
        let cfg = KindlingPostureConfig {
            enabled: true,
            posture_path: dir.join("posture.json").display().to_string(),
            ..KindlingPostureConfig::default()
        };
        let module = KindlingPostureModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "kindling_posture");
        assert_eq!(seg.fragments[0].text, "kindling: ready");
    }

    #[test]
    fn renders_nothing_when_posture_absent() {
        let dir = tmp_dir("absent");
        let cfg = KindlingPostureConfig {
            enabled: true,
            posture_path: dir.join("posture.json").display().to_string(),
            ..KindlingPostureConfig::default()
        };
        let module = KindlingPostureModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = KindlingPostureConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.posture_path, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = KindlingPostureConfig::default();
        assert_eq!(cfg.ready_style.as_str(), "bold #A3BE8C");
        assert_eq!(cfg.provisioned_style.as_str(), "bold #EBCB8B");
        assert_eq!(cfg.other_style.as_str(), "bold #D08770");
    }
}
