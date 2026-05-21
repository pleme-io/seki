//! `blackmatter` segment — counts enabled blackmatter components on
//! the operator's host.
//!
//! Pleme-io-native (Tier 4 — substrate-themed). Two-tier lookup:
//!
//! 1. Read `~/.config/blackmatter/enabled-components.json` and count
//!    the `components` array.
//! 2. Heuristic fallback — scan `$XDG_CONFIG_HOME` (default
//!    `~/.config`) for known blackmatter HM module directories
//!    (`mado/`, `tatara/`, …) and count those.
//!
//! Either lookup → render. Both empty → segment absent.
//!
//! ## Theme
//!
//! Nord-aurora green `#A3BE8C` — "ok / clean / instrumented". Matches
//! the `tend` clean state convention.
//!
//! ## Probe budget
//!
//! Filesystem reads only — no subprocess, no env-var I/O for the
//! actual count (we do read `XDG_CONFIG_HOME` for the fallback root,
//! which is a process-local env read with no syscall fanout). JSON
//! parsing delegated to
//! `seki_core::config::blackmatter::read_manifest_count` (which owns
//! the typed `BlackmatterManifest` shape + `serde_json` dep — keeps
//! seki-modules pure-data).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::blackmatter::{BlackmatterConfig, read_manifest_count},
    segment::StyledFragment,
};
use std::path::{Path, PathBuf};

pub struct BlackmatterModule {
    cfg: BlackmatterConfig,
}

impl BlackmatterModule {
    pub fn new(cfg: BlackmatterConfig) -> Self {
        Self { cfg }
    }
}

impl Module for BlackmatterModule {
    fn name(&self) -> &'static str {
        "blackmatter"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let xdg = xdg_config_home(ctx.home.as_deref(), env_lookup);
        let Some(count) = resolve_count(
            ctx.home.as_deref(),
            &self.cfg.manifest_path,
            xdg.as_deref(),
            &self.cfg.known_components,
        ) else {
            return Ok(None);
        };
        let text = render_format(&self.cfg.format, count);
        Ok(Some(Segment::new("blackmatter").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Resolve `$XDG_CONFIG_HOME` with the standard fallback:
/// `$XDG_CONFIG_HOME` env var → `<home>/.config` → `None`.
///
/// Exposed for testability; the production call uses [`env_lookup`].
pub fn xdg_config_home<F>(home: Option<&Path>, lookup: F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(v) = lookup("XDG_CONFIG_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    home.map(|h| h.join(".config"))
}

/// Pure resolver — exposed for testability. Order of precedence:
///
/// 1. JSON manifest at `<home>/<manifest_path>` (counted via
///    [`read_manifest_count`]).
/// 2. Filesystem-heuristic — scan `xdg_config_home` for any directory
///    whose name matches an entry in `known_components`.
///
/// Returns `None` when neither lookup produces a count (including
/// the case where both yield `0`, since "blackmatter not present" is
/// the typed absent signal, not "0 components enabled").
pub fn resolve_count(
    home: Option<&Path>,
    manifest_path: &str,
    xdg_config_home: Option<&Path>,
    known_components: &[String],
) -> Option<usize> {
    if let Some(home) = home {
        let path = home.join(manifest_path);
        if let Some(n) = read_manifest_count(&path) {
            // Manifest present and parsed — trust it even at 0.
            return Some(n);
        }
    }
    let xdg = xdg_config_home?;
    let count = count_known_dirs(xdg.to_path_buf(), known_components);
    if count == 0 {
        None
    } else {
        Some(count)
    }
}

/// Count directories under `root` whose name matches an entry in
/// `known`. Quiet on read errors (a missing or unreadable root → 0,
/// not an error — the segment treats this as "blackmatter not
/// installed").
pub fn count_known_dirs(root: PathBuf, known: &[String]) -> usize {
    let entries = match std::fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(_) => return 0,
    };
    let mut hits = 0usize;
    for entry in entries.flatten() {
        let Ok(ty) = entry.file_type() else { continue };
        if !ty.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        if known.iter().any(|k| k == s) {
            hits += 1;
        }
    }
    hits
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Render the format string. Substitutions: `$count`. Starship-style
/// `[…]($style)` markup stripped. Mirrors `shikumi_tier::render_format`.
pub fn render_format(fmt: &str, count: usize) -> String {
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
                "count" => out.push_str(&count.to_string()),
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
        p.push(format!("seki-bm-mod-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn stub_lookup(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).map(|s| (*s).to_owned())
    }

    fn known() -> Vec<String> {
        ["mado", "tatara", "kindling"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    #[test]
    fn manifest_overrides_heuristic() {
        let dir = tmp_dir("manifest-wins");
        let bm_dir = dir.join(".config").join("blackmatter");
        fs::create_dir_all(&bm_dir).unwrap();
        fs::write(
            bm_dir.join("enabled-components.json"),
            r#"{"components":["a","b","c","d","e"]}"#,
        )
        .unwrap();
        // Heuristic would find 1 dir; manifest says 5 → manifest wins.
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(cfg_dir.join("mado")).unwrap();
        let count = resolve_count(
            Some(&dir),
            ".config/blackmatter/enabled-components.json",
            Some(&cfg_dir),
            &known(),
        );
        assert_eq!(count, Some(5));
    }

    #[test]
    fn heuristic_used_when_manifest_absent() {
        let dir = tmp_dir("heuristic");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(cfg_dir.join("mado")).unwrap();
        fs::create_dir_all(cfg_dir.join("tatara")).unwrap();
        // Unrelated dir doesn't count.
        fs::create_dir_all(cfg_dir.join("randomthing")).unwrap();
        let count = resolve_count(
            Some(&dir),
            ".config/blackmatter/enabled-components.json",
            Some(&cfg_dir),
            &known(),
        );
        assert_eq!(count, Some(2));
    }

    #[test]
    fn neither_present_returns_none() {
        let dir = tmp_dir("neither");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        let count = resolve_count(
            Some(&dir),
            ".config/blackmatter/enabled-components.json",
            Some(&cfg_dir),
            &known(),
        );
        assert!(count.is_none());
    }

    #[test]
    fn manifest_with_zero_components_is_load_bearing() {
        // An explicit empty manifest ≠ "blackmatter not installed".
        // We trust the manifest even at 0.
        let dir = tmp_dir("zero-manifest");
        let bm_dir = dir.join(".config").join("blackmatter");
        fs::create_dir_all(&bm_dir).unwrap();
        fs::write(
            bm_dir.join("enabled-components.json"),
            r#"{"components":[]}"#,
        )
        .unwrap();
        let cfg_dir = dir.join(".config");
        let count = resolve_count(
            Some(&dir),
            ".config/blackmatter/enabled-components.json",
            Some(&cfg_dir),
            &known(),
        );
        assert_eq!(count, Some(0));
    }

    #[test]
    fn xdg_env_overrides_home_default() {
        let lookup = stub_lookup(HashMap::from([("XDG_CONFIG_HOME", "/some/xdg/path")]));
        let r = xdg_config_home(Some(Path::new("/home/u")), lookup);
        assert_eq!(r, Some(PathBuf::from("/some/xdg/path")));
    }

    #[test]
    fn xdg_falls_back_to_home_config() {
        let lookup = stub_lookup(HashMap::new());
        let r = xdg_config_home(Some(Path::new("/home/u")), lookup);
        assert_eq!(r, Some(PathBuf::from("/home/u/.config")));
    }

    #[test]
    fn render_format_count_substitution() {
        assert_eq!(render_format("[bm: $count]($style)", 7), "bm: 7");
    }

    #[test]
    fn renders_segment_for_present_manifest() {
        let dir = tmp_dir("render");
        let bm_dir = dir.join(".config").join("blackmatter");
        fs::create_dir_all(&bm_dir).unwrap();
        fs::write(
            bm_dir.join("enabled-components.json"),
            r#"{"components":["mado","tatara","kindling"]}"#,
        )
        .unwrap();
        let mut cfg = BlackmatterConfig::default();
        cfg.enabled = true;
        let module = BlackmatterModule::new(cfg);
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "blackmatter");
        assert_eq!(seg.fragments[0].text, "bm: 3");
    }

    #[test]
    fn renders_nothing_on_non_blackmatter_host() {
        let dir = tmp_dir("absent");
        let cfg_dir = dir.join(".config");
        fs::create_dir_all(&cfg_dir).unwrap();
        let mut cfg = BlackmatterConfig::default();
        cfg.enabled = true;
        let module = BlackmatterModule::new(cfg);
        let mut ctx = RenderContext::from_env().with_colors(false);
        ctx.home = Some(dir);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
