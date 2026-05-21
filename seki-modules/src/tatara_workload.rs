//! `tatara_workload` segment — surfaces running tatara allocations.
//!
//! Pleme-io-native (Tier 3). Spawns `tatara node list --format=json`,
//! parses the running-allocation count, and emits a Nord-frost-cyan
//! segment.
//!
//! ## Theme
//!
//! Nord-frost cyan `#88C0D0`.
//!
//! ## Probe budget
//!
//! Subprocess with a hard timeout (`command_timeout_ms`). 30s cache.
//! Gracefully absent on failure.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::tatara_workload::TataraWorkloadConfig,
    segment::StyledFragment,
};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct CachedCount {
    count: u32,
    captured_at: Instant,
}

pub struct TataraWorkloadModule {
    cfg: TataraWorkloadConfig,
    cache: Mutex<Option<CachedCount>>,
}

impl TataraWorkloadModule {
    pub fn new(cfg: TataraWorkloadConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for TataraWorkloadModule {
    fn name(&self) -> &'static str {
        "tatara_workload"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| *g);
        if let Some(entry) = cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.count, false)));
            }
        }
        let probe = probe_tatara(&self.cfg.command, self.cfg.command_timeout_ms);
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
                if let Some(entry) = cached {
                    Ok(Some(build_segment(&self.cfg, entry.count, true)))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

fn build_segment(cfg: &TataraWorkloadConfig, count: u32, stale: bool) -> Segment {
    let status_label = format_status(count, stale);
    let text = render_format(&cfg.format, count, &status_label);
    let style = cfg.style.resolve();
    Segment::new("tatara_workload").push(StyledFragment::new(text, style))
}

pub fn format_status(count: u32, stale: bool) -> String {
    let mut s = String::from("tatara: ");
    s.push_str(&count.to_string());
    s.push_str(" alloc");
    if stale {
        s.push_str(" (stale)");
    }
    s
}

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

fn probe_tatara(command: &str, timeout_ms: u64) -> Option<u32> {
    let cmd = command.to_owned();
    let (tx, rx) = mpsc::channel::<Option<u32>>();
    std::thread::spawn(move || {
        let result = run_tatara_node_list(&cmd);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

fn run_tatara_node_list(command: &str) -> Option<u32> {
    let output = Command::new(command)
        .arg("node")
        .arg("list")
        .arg("--format=json")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(parse_tatara_json(&text))
}

/// Count running allocations from JSON output. Tolerantly checks
/// `"allocations_running"` / `"running"` integer fields, then falls
/// back to counting `"status":"Running"`.
pub fn parse_tatara_json(body: &str) -> u32 {
    if let Some(n) = extract_u32_field(body, "allocations_running") {
        return n;
    }
    if let Some(n) = extract_u32_field(body, "running") {
        return n;
    }
    body.matches("\"status\":\"Running\"").count() as u32
        + body.matches("\"status\": \"Running\"").count() as u32
}

fn extract_u32_field(body: &str, name: &str) -> Option<u32> {
    let key = format!("\"{name}\"");
    let idx = body.find(&key)?;
    let after = &body[idx + key.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?;
    let after = after.trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    if end == 0 {
        return None;
    }
    after[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tatara_json_explicit_running_count() {
        let body = r#"{"allocations_running": 7}"#;
        assert_eq!(parse_tatara_json(body), 7);
    }

    #[test]
    fn parse_tatara_json_falls_back_to_status_scan() {
        let body = r#"{"allocs":[
            {"status":"Running"},{"status":"Running"},
            {"status":"Pending"},{"status":"Running"}
        ]}"#;
        assert_eq!(parse_tatara_json(body), 3);
    }

    #[test]
    fn parse_tatara_json_empty_returns_zero() {
        let body = r#"{"allocs":[]}"#;
        assert_eq!(parse_tatara_json(body), 0);
    }

    #[test]
    fn parse_tatara_json_alternate_running_key() {
        let body = r#"{"running": 4}"#;
        assert_eq!(parse_tatara_json(body), 4);
    }

    #[test]
    fn format_status_zero() {
        assert_eq!(format_status(0, false), "tatara: 0 alloc");
    }

    #[test]
    fn format_status_with_count() {
        assert_eq!(format_status(5, false), "tatara: 5 alloc");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(2, true), "tatara: 2 alloc (stale)");
    }

    #[test]
    fn render_format_default_template() {
        let out = render_format("[$status]($style)", 3, "tatara: 3 alloc");
        assert_eq!(out, "tatara: 3 alloc");
    }

    #[test]
    fn render_format_count_substitution() {
        let out = render_format("alloc=$count", 9, "_");
        assert_eq!(out, "alloc=9");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = TataraWorkloadConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.command, "");
    }

    #[test]
    fn default_uses_nord_frost_palette() {
        let cfg = TataraWorkloadConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #88C0D0");
    }

    #[test]
    fn default_disabled_per_tier3_policy() {
        assert!(!TataraWorkloadConfig::default().enabled);
    }

    #[test]
    fn missing_binary_renders_nothing() {
        let cfg = TataraWorkloadConfig {
            enabled: true,
            command: "tatara-nonexistent-binary-zzz".to_owned(),
            ..TataraWorkloadConfig::default()
        };
        let module = TataraWorkloadModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
