//! `tend` segment — surfaces tend workspace status in the prompt.
//!
//! Pleme-io-native. Spawns `tend status`, counts non-clean repos
//! across every workspace, and emits a tiered Nord-themed segment
//! (green / yellow / red) bounded by `command_timeout_ms`.
//!
//! ## Theme
//!
//! - green `#A3BE8C` — count == 0 (clean)
//! - yellow `#EBCB8B` — 1..=5 (light)
//! - red `#BF616A` — >= 6 (heavy)
//!
//! ## Probe budget
//!
//! Subprocess with a hard timeout (`command_timeout_ms`). If tend is
//! missing, errors, or times out, the segment renders nothing —
//! never blocks the prompt. A 60s in-process cache (per `TendConfig`)
//! prevents repeated invocations within a single shell session.
//!
//! ## Deviation from brief
//!
//! Brief specifies `tend status --format=json`. tend 0.1.0 does not
//! accept `--format`; we parse the stable plain-text output instead.
//! See `parse_tend_status_text` for the format contract. Migrating
//! to JSON is a one-line swap once tend grows the flag.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::tend::TendConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

/// Cached probe result — populated on the first successful render,
/// returned (with a `(stale)` annotation) when the next render falls
/// inside the cache TTL window.
#[derive(Debug, Clone)]
struct CachedCount {
    count: u32,
    captured_at: Instant,
}

pub struct TendModule {
    cfg: TendConfig,
    cache: Mutex<Option<CachedCount>>,
}

impl TendModule {
    pub fn new(cfg: TendConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for TendModule {
    fn name(&self) -> &'static str {
        "tend"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // 1. Cache fast-path: hit if last probe was within TTL.
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| g.clone());
        if let Some(entry) = &cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.count, false)));
            }
        }

        // 2. Cache miss / expired: probe tend with a hard timeout.
        let probe = probe_tend(&self.cfg.command, self.cfg.command_timeout_ms);
        match probe {
            Some(count) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedCount {
                        count,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, count, false)))
            }
            None => {
                // Probe failed. If we have a stale cached value,
                // surface it; otherwise the segment renders nothing
                // (tend missing / first-render failure).
                if let Some(entry) = cached {
                    Ok(Some(build_segment(&self.cfg, entry.count, true)))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

/// Build the typed [`Segment`] from a count + a tier-derived style.
fn build_segment(cfg: &TendConfig, count: u32, stale: bool) -> Segment {
    let status_label = format_status(count, stale);
    let text = render_format(&cfg.format, count, &status_label);
    let style = pick_style(cfg, count);
    Segment::new("tend").push(StyledFragment::new(text, style))
}

/// Pick the tier-derived style from `count`.
pub fn pick_style(cfg: &TendConfig, count: u32) -> Style {
    let spec: &StyleSpec = if count == 0 {
        &cfg.clean_style
    } else if count >= cfg.heavy_threshold {
        &cfg.heavy_style
    } else {
        &cfg.light_style
    };
    spec.resolve()
}

/// `tend: clean` when count == 0; `tend: N dirty` otherwise. Stale
/// renders get a ` (stale)` suffix so operators know the count is
/// from cache.
pub fn format_status(count: u32, stale: bool) -> String {
    let mut out = if count == 0 {
        "tend: clean".to_owned()
    } else {
        let mut s = String::from("tend: ");
        s.push_str(&count.to_string());
        s.push_str(" dirty");
        s
    };
    if stale {
        out.push_str(" (stale)");
    }
    out
}

/// Render the format string. Substitutions: `$count`, `$status`.
/// Starship-style `[…]($style)` markup is stripped (the renderer
/// applies `style` directly). Mirrors `shikumi_tier::render_format`.
pub fn render_format(fmt: &str, count: u32, status: &str) -> String {
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
                "count" => out.push_str(&count.to_string()),
                "status" => out.push_str(status),
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

/// Spawn `tend status` with a hard timeout. Returns the number of
/// non-clean repos, or `None` if tend is missing / failed /
/// timed out.
///
/// The renderer is sync — we use a thread + `recv_timeout` rather
/// than tokio to keep the dependency tree minimal.
fn probe_tend(command: &str, timeout_ms: u64) -> Option<u32> {
    let cmd = command.to_owned();
    let (tx, rx) = mpsc::channel::<Option<u32>>();
    std::thread::spawn(move || {
        let result = run_tend_status(&cmd);
        // Receiver may have dropped on timeout — ignore send errors.
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

/// Run `tend status` synchronously, capturing stdout. Returns the
/// non-clean count or `None` on any failure (spawn error, non-zero
/// exit, non-UTF8 output).
fn run_tend_status(command: &str) -> Option<u32> {
    let output = Command::new(command)
        .arg("status")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(parse_tend_status_text(&text))
}

/// Parse the count of non-clean repos from `tend status` plain-text
/// output. Format contract (verified against tend 0.1.0):
///
/// ```text
/// workspace: pleme-io
///
///   [ok] foo                   clean
///   [!!] bar                   dirty
///   [--] baz                   missing
///   [??] qux                   unknown
/// ```
///
/// We count any data line whose terminal column is anything other
/// than `clean`. Header / blank lines are skipped. The same shape
/// is used by every workspace in the output.
pub fn parse_tend_status_text(text: &str) -> u32 {
    let mut count: u32 = 0;
    for line in text.lines() {
        let trimmed = line.trim_start();
        // Data lines start with the typed status tag in brackets.
        if !trimmed.starts_with('[') {
            continue;
        }
        // Last whitespace-separated token == state word.
        let last = match trimmed.split_whitespace().last() {
            Some(s) => s,
            None => continue,
        };
        if last != "clean" {
            count = count.saturating_add(1);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_workspace_dirty_count() {
        let text = "workspace: pleme-io\n\n  [ok] a clean\n  [!!] b dirty\n  [--] c missing\n";
        assert_eq!(parse_tend_status_text(text), 2);
    }

    #[test]
    fn parses_all_clean_as_zero() {
        let text = "workspace: pleme-io\n\n  [ok] a clean\n  [ok] b clean\n";
        assert_eq!(parse_tend_status_text(text), 0);
    }

    #[test]
    fn parses_multi_workspace() {
        let text = "\
workspace: a

  [ok] r1 clean
  [!!] r2 dirty

workspace: b

  [??] r3 unknown
  [ok] r4 clean
";
        assert_eq!(parse_tend_status_text(text), 2);
    }

    #[test]
    fn skips_non_data_lines() {
        let text = "\nworkspace: x\nrandom prose\n  not a tag line\n  [ok] r clean\n";
        assert_eq!(parse_tend_status_text(text), 0);
    }

    #[test]
    fn format_status_clean() {
        assert_eq!(format_status(0, false), "tend: clean");
    }

    #[test]
    fn format_status_dirty_pluralised() {
        assert_eq!(format_status(3, false), "tend: 3 dirty");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(2, true), "tend: 2 dirty (stale)");
    }

    #[test]
    fn render_format_count_substitution() {
        let out = render_format("[$status: $count]($style)", 7, "tend: 7 dirty");
        assert_eq!(out, "tend: 7 dirty: 7");
    }

    #[test]
    fn render_format_default_template() {
        let out = render_format("[$status]($style)", 0, "tend: clean");
        assert_eq!(out, "tend: clean");
    }

    #[test]
    fn pick_style_tiers() {
        let cfg = TendConfig::default();
        assert_eq!(pick_style(&cfg, 0), cfg.clean_style.resolve());
        assert_eq!(pick_style(&cfg, 3), cfg.light_style.resolve());
        assert_eq!(pick_style(&cfg, 9), cfg.heavy_style.resolve());
    }

    #[test]
    fn pick_style_threshold_boundary() {
        let cfg = TendConfig::default();
        // heavy_threshold = 6; 5 → light, 6 → heavy.
        assert_eq!(pick_style(&cfg, 5), cfg.light_style.resolve());
        assert_eq!(pick_style(&cfg, 6), cfg.heavy_style.resolve());
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = TendConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.command, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = TendConfig::default();
        assert_eq!(cfg.clean_style.as_str(), "bold #A3BE8C");
        assert_eq!(cfg.light_style.as_str(), "bold #EBCB8B");
        assert_eq!(cfg.heavy_style.as_str(), "bold #BF616A");
    }

    #[test]
    fn missing_command_renders_nothing() {
        // Spawning a non-existent binary should fail → segment is None.
        let cfg = TendConfig {
            command: "tend-nonexistent-binary-zzz".to_owned(),
            ..TendConfig::default()
        };
        let module = TendModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
