//! `kasou_vm` segment — surfaces running kasou VM count in the prompt.
//!
//! Pleme-io-native (Tier 5). Spawns `kasou list --format=json` with a
//! hard timeout, parses the JSON array of VM records, and counts
//! entries whose `state` field equals `"running"`. The output is a
//! single Nord-themed segment of the shape `[kasou: N vm]`.
//!
//! ## Theme
//!
//! - active (`count >= 1`): Nord-frost cyan `#88C0D0`
//! - idle (`count == 0`): snowstorm dim white `#D8DEE9`
//!
//! ## Probe budget
//!
//! Subprocess with a hard `command_timeout_ms` timeout enforced via
//! a thread + `mpsc::recv_timeout` (mirrors `tend.rs` — no tokio in
//! the prompt hot path). A 60s in-process cache (see [`KasouVmConfig`])
//! prevents repeated invocations within one shell session; stale
//! renders annotate with `(stale)`.
//!
//! ## Graceful absence
//!
//! Any of: missing binary, non-zero exit, non-UTF8 output, unparseable
//! JSON, or timeout → the segment renders nothing (returns `Ok(None)`).
//! The kasou daemon may not be present on a given host; the prompt
//! must never block, panic, or surface a noisy error.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::kasou_vm::KasouVmConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::process::{Command, Stdio};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

/// Cached probe result.
#[derive(Debug, Clone)]
struct CachedCount {
    count: u32,
    captured_at: Instant,
}

pub struct KasouVmModule {
    cfg: KasouVmConfig,
    cache: Mutex<Option<CachedCount>>,
}

impl KasouVmModule {
    pub fn new(cfg: KasouVmConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for KasouVmModule {
    fn name(&self) -> &'static str {
        "kasou_vm"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // 1. Cache fast-path.
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| g.clone());
        if let Some(entry) = &cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.count, false)));
            }
        }

        // 2. Probe.
        match probe_kasou(&self.cfg.command, self.cfg.command_timeout_ms) {
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
                    // Gracefully absent — kasou missing / timed out /
                    // unparseable output. Prompt renders nothing.
                    Ok(None)
                }
            }
        }
    }
}

fn build_segment(cfg: &KasouVmConfig, count: u32, stale: bool) -> Segment {
    let status = format_status(count, stale);
    let text = seki_core::format::render(&cfg.format, |__n| match __n {
        "count" => Some(count.to_string()),
        "status" => Some(status.to_owned()),
        _ => None,
    });
    let style = pick_style(cfg, count);
    Segment::new("kasou_vm").push(StyledFragment::new(text, style))
}

/// Pick `active_style` when count >= 1, `idle_style` when count == 0.
pub fn pick_style(cfg: &KasouVmConfig, count: u32) -> Style {
    let spec: &StyleSpec = if count >= 1 {
        &cfg.active_style
    } else {
        &cfg.idle_style
    };
    spec.resolve()
}

/// Build the `kasou: N vm` label. Stale renders append ` (stale)`.
pub fn format_status(count: u32, stale: bool) -> String {
    let mut s = String::from("kasou: ");
    s.push_str(&count.to_string());
    s.push_str(" vm");
    if stale {
        s.push_str(" (stale)");
    }
    s
}

/// Spawn `kasou list --format=json` with a hard timeout. Returns the
/// count of `running` VMs, or `None` if anything goes wrong.
fn probe_kasou(command: &str, timeout_ms: u64) -> Option<u32> {
    let cmd = command.to_owned();
    let (tx, rx) = mpsc::channel::<Option<u32>>();
    std::thread::spawn(move || {
        let result = run_kasou_list(&cmd);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

/// Run `kasou list --format=json` synchronously. Returns the running
/// VM count or `None` on any failure (spawn / non-zero exit /
/// non-UTF8 / unparseable JSON).
fn run_kasou_list(command: &str) -> Option<u32> {
    let output = Command::new(command)
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
    Some(parse_running_count(&text))
}

/// Pure-string parser: count `state: running` entries in the JSON
/// output of `kasou list --format=json`. We use a minimal scan rather
/// than a full serde_json dependency because the shape is open-ended
/// (any number of fields per VM record) and we only care about the
/// `state` discriminant.
///
/// Format contract (verified informally — kasou ships VM records as
/// JSON objects with a `state` field):
///
/// ```text
/// [
///   { "name": "rio", "state": "running", ... },
///   { "name": "zek", "state": "stopped", ... }
/// ]
/// ```
///
/// Returns 0 on any unparseable input — the segment renders an idle
/// indicator rather than vanishing. Returning `None` from `probe_kasou`
/// is reserved for the harder failures (spawn error / non-zero exit).
pub fn parse_running_count(text: &str) -> u32 {
    let mut count: u32 = 0;
    // Walk the text looking for `"state"\s*:\s*"running"` token pairs.
    // This handles whitespace + quote variations without dragging in
    // a JSON parser.
    let bytes = text.as_bytes();
    let needle = b"\"state\"";
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let mut j = i + needle.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] != b':' {
                i += 1;
                continue;
            }
            j += 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] != b'"' {
                i += 1;
                continue;
            }
            j += 1; // skip opening quote
            let value_start = j;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            let value = &bytes[value_start..j];
            if value == b"running" {
                count = count.saturating_add(1);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_running_one_stopped() {
        let text = r#"[
  {"name":"rio","state":"running"},
  {"name":"zek","state":"stopped"},
  {"name":"plo","state":"running"}
]"#;
        assert_eq!(parse_running_count(text), 2);
    }

    #[test]
    fn parses_empty_array_as_zero() {
        assert_eq!(parse_running_count("[]"), 0);
    }

    #[test]
    fn parses_all_stopped_as_zero() {
        let text = r#"[{"state":"stopped"},{"state":"paused"}]"#;
        assert_eq!(parse_running_count(text), 0);
    }

    #[test]
    fn tolerates_whitespace_around_colon() {
        let text = r#"[{"state"   :    "running"}]"#;
        assert_eq!(parse_running_count(text), 1);
    }

    #[test]
    fn ignores_state_substrings() {
        // A field literally named `lifecycle_state` shouldn't trigger.
        // We anchor on the exact `"state"` quoted-key token.
        let text = r#"[{"lifecycle_state":"running","state":"stopped"}]"#;
        assert_eq!(parse_running_count(text), 0);
    }

    #[test]
    fn format_status_idle() {
        assert_eq!(format_status(0, false), "kasou: 0 vm");
    }

    #[test]
    fn format_status_active() {
        assert_eq!(format_status(2, false), "kasou: 2 vm");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(1, true), "kasou: 1 vm (stale)");
    }

    #[test]
    fn render_format_default_template() {
        let out = seki_core::format::render_one("[$status]($style)", "status", "kasou: 3 vm");
        assert_eq!(out, "kasou: 3 vm");
    }

    #[test]
    fn render_format_count_substitution() {
        let out = seki_core::format::render("[$status — $count]($style)", |__n| match __n {
            "count" => Some(5u32.to_string()),
            "status" => Some("kasou: 5 vm".to_owned()),
            _ => None,
        });
        assert_eq!(out, "kasou: 5 vm — 5");
    }

    #[test]
    fn pick_style_active_vs_idle() {
        let cfg = KasouVmConfig::default();
        assert_eq!(pick_style(&cfg, 0), cfg.idle_style.resolve());
        assert_eq!(pick_style(&cfg, 1), cfg.active_style.resolve());
        assert_eq!(pick_style(&cfg, 9), cfg.active_style.resolve());
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = KasouVmConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.command, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = KasouVmConfig::default();
        assert_eq!(cfg.active_style.as_str(), "bold #88C0D0");
        assert_eq!(cfg.idle_style.as_str(), "dimmed #D8DEE9");
    }

    #[test]
    fn tier5_default_is_disabled() {
        // Tier 5: probe cost > 0; operator opts in.
        let cfg = KasouVmConfig::default();
        assert!(!cfg.enabled);
    }

    #[test]
    fn missing_command_renders_nothing() {
        let cfg = KasouVmConfig {
            enabled: true,
            command: "kasou-nonexistent-binary-zzz".to_owned(),
            ..KasouVmConfig::default()
        };
        let module = KasouVmModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
