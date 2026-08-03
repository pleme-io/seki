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
    Module, RenderContext, Segment, SekiResult, config::tatara_workload::TataraWorkloadConfig,
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
    let text = seki_core::format::render(&cfg.format, |__n| match __n {
        "count" => Some(count.to_string()),
        "status" => Some(status_label.to_owned()),
        _ => None,
    });
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
    parse_tatara_json(&text)
}

/// Count running allocations from JSON output. Tolerantly checks
/// `"allocations_running"` / `"running"` integer fields, then falls
/// back to counting `"status":"Running"`.
/// ── ★ NO ALLOCATION DATA IS NO READING, NOT ZERO ───────────────────────
/// Returned a bare `u32`, so a body carrying neither an explicit count nor
/// a single `"status"` entry — an error page, a moved schema, a binary
/// that exited 0 with a warning — tallied to 0 and rendered as a MEASURED
/// `tatara: 0`. Same shape as `tend`'s green `clean`, and as `shigoto`'s
/// `idle` before its `has_phases` guard landed.
///
/// The explicit fields are evidence in themselves; the scan needs a
/// `"status"` marker before a count over it means anything.
pub fn parse_tatara_json(body: &str) -> Option<u32> {
    if let Some(n) = extract_u32_field(body, "allocations_running") {
        return Some(n);
    }
    if let Some(n) = extract_u32_field(body, "running") {
        return Some(n);
    }
    // ── ★ THE MARKER IS THE CONTAINER, NOT THE ENTRIES ─────────────────
    // First written as `"status"`, and the repo's own
    // `parse_tatara_json_empty_returns_zero` went red: `{"allocs":[]}` is a
    // GENUINE zero — tatara answered, and the answer is "none running".
    // Keying on the entries would have turned every real idle reading into
    // "no evidence" and silenced the segment exactly when it is healthy,
    // which is a worse failure than the one being fixed.
    //
    // The container proves the document is a tatara response; the entries
    // only prove it is non-empty.
    seki_core::exec::tally_with_evidence(body, &["\"allocs\"", "\"status\""], |b| {
        b.matches("\"status\":\"Running\"").count() as u32
            + b.matches("\"status\": \"Running\"").count() as u32
    })
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
    /// ── ★ THE `stable` CLASS, IN A COUNT ───────────────────────────────
    /// A zero-exit body with nothing countable in it used to tally to 0,
    /// and 0 renders GREEN — "measured, and everything is fine" — for a
    /// document the segment never actually understood.
    ///
    /// Red run: drop the `tally_with_evidence` guard and this returns
    /// `Some(0)`, the literal bytes that render as healthy.
    #[test]
    fn a_body_with_nothing_countable_is_no_reading() {
        assert_eq!(
            parse_tatara_json(r#"<html><body>502 Bad Gateway</body></html>"#),
            None,
            "an unparseable body must not render as a measured zero"
        );
    }

    /// The negative control: a body that IS understood must still read.
    /// Without it, returning `None` unconditionally passes the case above
    /// and silences the segment permanently.
    #[test]
    fn a_body_that_is_understood_still_reads() {
        assert!(parse_tatara_json(r#"{"allocs":[{"status":"Running"}]}"#).is_some());
    }

    use super::*;

    #[test]
    fn parse_tatara_json_explicit_running_count() {
        let body = r#"{"allocations_running": 7}"#;
        assert_eq!(parse_tatara_json(body), Some(7));
    }

    #[test]
    fn parse_tatara_json_falls_back_to_status_scan() {
        let body = r#"{"allocs":[
            {"status":"Running"},{"status":"Running"},
            {"status":"Pending"},{"status":"Running"}
        ]}"#;
        assert_eq!(parse_tatara_json(body), Some(3));
    }

    #[test]
    fn parse_tatara_json_empty_returns_zero() {
        let body = r#"{"allocs":[]}"#;
        assert_eq!(parse_tatara_json(body), Some(0));
    }

    #[test]
    fn parse_tatara_json_alternate_running_key() {
        let body = r#"{"running": 4}"#;
        assert_eq!(parse_tatara_json(body), Some(4));
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
        let out = seki_core::format::render_vars(
            "[$status]($style)",
            &[("count", "3"), ("status", "tatara: 3 alloc")],
        );
        assert_eq!(out, "tatara: 3 alloc");
    }

    #[test]
    fn render_format_count_substitution() {
        let out =
            seki_core::format::render_vars("alloc=$count", &[("count", "9"), ("status", "_")]);
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
