//! `shigoto` segment — surfaces active shigoto job-DAG state.
//!
//! Pleme-io-native (Tier 3). Probes the shigoto daemon via HTTP at
//! `SHIGOTO_ADDR` (default `http://127.0.0.1:38830`), parses the
//! snapshot JSON, and emits a typed segment with the count of running
//! + pending jobs.
//!
//! ## Theme
//!
//! - Nord-aurora orange `#D08770` — `running + pending > 0` (active)
//! - Nord-aurora green `#A3BE8C` — both zero (scheduler idle)
//!
//! ## Probe budget
//!
//! Pure-Rust TCP + minimal HTTP/1.1 client, hard-bounded by
//! `command_timeout_ms` via `mpsc::recv_timeout`. No async runtime,
//! no `ureq`. Daemon absence / refused / timeout / parse error all
//! collapse to "render nothing".

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::shigoto::ShigotoConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct CachedSnapshot {
    running: u32,
    pending: u32,
    captured_at: Instant,
}

pub struct ShigotoModule {
    cfg: ShigotoConfig,
    cache: Mutex<Option<CachedSnapshot>>,
}

impl ShigotoModule {
    pub fn new(cfg: ShigotoConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for ShigotoModule {
    fn name(&self) -> &'static str {
        "shigoto"
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
                return Ok(Some(build_segment(
                    &self.cfg,
                    entry.running,
                    entry.pending,
                    false,
                )));
            }
        }
        let Some(url) = resolve_addr(&self.cfg.addr, env_lookup) else {
            return Ok(None);
        };
        let probe_path = build_probe_url(&url, &self.cfg.snapshot_path);
        let probe = probe_shigoto(&probe_path, self.cfg.command_timeout_ms);
        match probe {
            Some((running, pending)) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedSnapshot {
                        running,
                        pending,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, running, pending, false)))
            }
            None => {
                if let Some(entry) = cached {
                    Ok(Some(build_segment(
                        &self.cfg,
                        entry.running,
                        entry.pending,
                        true,
                    )))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

pub fn resolve_addr<F>(addr: &str, lookup: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if addr.is_empty() {
        return None;
    }
    if addr == "$env" {
        return Some(lookup("SHIGOTO_ADDR").unwrap_or_else(|| "http://127.0.0.1:38830".to_owned()));
    }
    Some(addr.to_owned())
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

pub fn build_probe_url(base: &str, path: &str) -> String {
    let base_trimmed = base.trim_end_matches('/');
    if path.starts_with('/') {
        let mut s = String::with_capacity(base_trimmed.len() + path.len());
        s.push_str(base_trimmed);
        s.push_str(path);
        s
    } else {
        let mut s = String::with_capacity(base_trimmed.len() + 1 + path.len());
        s.push_str(base_trimmed);
        s.push('/');
        s.push_str(path);
        s
    }
}

fn build_segment(cfg: &ShigotoConfig, running: u32, pending: u32, stale: bool) -> Segment {
    let status_label = format_status(running, pending, stale);
    let text = render_format(&cfg.format, running, pending, &status_label);
    let style = pick_style(cfg, running, pending);
    Segment::new("shigoto").push(StyledFragment::new(text, style))
}

pub fn pick_style(cfg: &ShigotoConfig, running: u32, pending: u32) -> Style {
    let spec: &StyleSpec = if running == 0 && pending == 0 {
        &cfg.idle_style
    } else {
        &cfg.active_style
    };
    spec.resolve()
}

pub fn format_status(running: u32, pending: u32, stale: bool) -> String {
    let mut out = if running == 0 && pending == 0 {
        "shigoto: idle".to_owned()
    } else {
        let mut s = String::from("shigoto: ");
        s.push_str(&running.to_string());
        s.push_str(" running, ");
        s.push_str(&pending.to_string());
        s.push_str(" pending");
        s
    };
    if stale {
        out.push_str(" (stale)");
    }
    out
}

pub fn render_format(fmt: &str, running: u32, pending: u32, status: &str) -> String {
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
                "running" => out.push_str(&running.to_string()),
                "pending" => out.push_str(&pending.to_string()),
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

fn probe_shigoto(url: &str, timeout_ms: u64) -> Option<(u32, u32)> {
    let url_owned = url.to_owned();
    let (tx, rx) = mpsc::channel::<Option<(u32, u32)>>();
    std::thread::spawn(move || {
        let result = run_http_probe(&url_owned, Duration::from_millis(timeout_ms));
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

fn run_http_probe(url: &str, timeout: Duration) -> Option<(u32, u32)> {
    let (host, port, path) = parse_url(url)?;
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect_timeout(&addr.parse().ok()?, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut buf = Vec::with_capacity(2048);
    stream.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let body = split_response_body(&text)?;
    parse_snapshot_counts(body)
}

pub fn parse_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rfind(':') {
        Some(i) => {
            let p: u16 = authority[i + 1..].parse().ok()?;
            (authority[..i].to_owned(), p)
        }
        None => (authority.to_owned(), 80),
    };
    Some((host, port, path.to_owned()))
}

pub fn split_response_body(response: &str) -> Option<&str> {
    response.find("\r\n\r\n").map(|i| &response[i + 4..])
}

pub fn parse_snapshot_counts(body: &str) -> Option<(u32, u32)> {
    let running = extract_u32_field(body, "running")
        .or_else(|| Some(count_phase(body, "Running") as u32))?;
    let pending = extract_u32_field(body, "pending").or_else(|| {
        let q = count_phase(body, "Queued") as u32;
        let r = count_phase(body, "Ready") as u32;
        Some(q + r)
    })?;
    Some((running, pending))
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

fn count_phase(body: &str, value: &str) -> usize {
    let needle_compact = format!("\"phase\":\"{value}\"");
    let needle_spaced = format!("\"phase\": \"{value}\"");
    body.matches(&needle_compact).count() + body.matches(&needle_spaced).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn stub_lookup(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).map(|s| (*s).to_owned())
    }

    #[test]
    fn resolve_addr_env_marker_uses_lookup() {
        let lookup = stub_lookup(HashMap::from([("SHIGOTO_ADDR", "http://1.2.3.4:9999")]));
        assert_eq!(
            resolve_addr("$env", lookup),
            Some("http://1.2.3.4:9999".to_owned())
        );
    }

    #[test]
    fn resolve_addr_empty_returns_none() {
        let lookup = stub_lookup(HashMap::new());
        assert_eq!(resolve_addr("", lookup), None);
    }

    #[test]
    fn resolve_addr_env_marker_default_fallback() {
        let lookup = stub_lookup(HashMap::new());
        assert_eq!(
            resolve_addr("$env", lookup),
            Some("http://127.0.0.1:38830".to_owned())
        );
    }

    #[test]
    fn build_probe_url_handles_trailing_slash() {
        assert_eq!(
            build_probe_url("http://x:1/", "/v1/snapshot"),
            "http://x:1/v1/snapshot"
        );
    }

    #[test]
    fn parse_url_explicit_port() {
        let (h, p, path) = parse_url("http://127.0.0.1:38830/v1/snapshot").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 38830);
        assert_eq!(path, "/v1/snapshot");
    }

    #[test]
    fn parse_url_rejects_non_http() {
        assert!(parse_url("https://x/").is_none());
    }

    #[test]
    fn parse_snapshot_counts_explicit_fields() {
        let body = r#"{"running": 2, "pending": 5}"#;
        assert_eq!(parse_snapshot_counts(body), Some((2, 5)));
    }

    #[test]
    fn parse_snapshot_counts_falls_back_to_phase_scan() {
        let body = r#"{"jobs":[
            {"phase":"Running"},{"phase":"Running"},
            {"phase":"Queued"},{"phase":"Ready"}
        ]}"#;
        assert_eq!(parse_snapshot_counts(body), Some((2, 2)));
    }

    #[test]
    fn format_status_idle() {
        assert_eq!(format_status(0, 0, false), "shigoto: idle");
    }

    #[test]
    fn format_status_active() {
        assert_eq!(format_status(2, 3, false), "shigoto: 2 running, 3 pending");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(
            format_status(1, 0, true),
            "shigoto: 1 running, 0 pending (stale)"
        );
    }

    #[test]
    fn render_format_default_template() {
        let out = render_format("[$status]($style)", 1, 2, "shigoto: 1 running, 2 pending");
        assert_eq!(out, "shigoto: 1 running, 2 pending");
    }

    #[test]
    fn pick_style_active_when_running() {
        let cfg = ShigotoConfig::default();
        assert_eq!(pick_style(&cfg, 1, 0), cfg.active_style.resolve());
    }

    #[test]
    fn pick_style_idle_when_both_zero() {
        let cfg = ShigotoConfig::default();
        assert_eq!(pick_style(&cfg, 0, 0), cfg.idle_style.resolve());
    }

    #[test]
    fn split_response_body_basic() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"running\":0}";
        assert_eq!(split_response_body(resp), Some("{\"running\":0}"));
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = ShigotoConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.addr, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = ShigotoConfig::default();
        assert_eq!(cfg.active_style.as_str(), "bold #D08770");
        assert_eq!(cfg.idle_style.as_str(), "bold #A3BE8C");
    }

    #[test]
    fn default_disabled_per_tier3_policy() {
        assert!(!ShigotoConfig::default().enabled);
    }

    #[test]
    fn empty_addr_renders_nothing() {
        let cfg = ShigotoConfig {
            enabled: true,
            addr: String::new(),
            ..ShigotoConfig::default()
        };
        let module = ShigotoModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
