//! `vigy` segment — surfaces local vigy reconciler count + tick rate.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). HTTP
//! probe to vigy's default REST endpoint
//! (`http://127.0.0.1:38821/reconcilers`). Tells the operator at a
//! glance how many controllers are registered + at what rate they're
//! ticking.
//!
//! ## Theme
//!
//! Nord-frost blue `#81A1C1` — control-surface frost gradient.
//!
//! ## Probe budget
//!
//! Hard-bounded HTTP GET against the configured host:port via a
//! minimal `std::net::TcpStream` + raw HTTP/1.1 framing — no `ureq`,
//! no `tokio`, no `reqwest` (probe must add zero new deps per the
//! brief). Failures + timeouts render nothing (graceful absence).
//! A `cache_ttl_secs` window prevents repeated probes inside one
//! shell session.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::vigy::VigyConfig,
    segment::StyledFragment,
};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CachedSnapshot {
    count: u32,
    tick_hz: f64,
    captured_at: Instant,
}

pub struct VigyModule {
    cfg: VigyConfig,
    cache: Mutex<Option<CachedSnapshot>>,
}

impl VigyModule {
    pub fn new(cfg: VigyConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for VigyModule {
    fn name(&self) -> &'static str {
        "vigy"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| g.clone());
        if let Some(entry) = &cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.count, entry.tick_hz)));
            }
        }
        // Cache miss — probe.
        match probe_vigy(&self.cfg.host, self.cfg.port, &self.cfg.path, self.cfg.probe_timeout_ms)
        {
            Some((count, tick_hz)) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedSnapshot {
                        count,
                        tick_hz,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, count, tick_hz)))
            }
            None => Ok(None),
        }
    }
}

fn build_segment(cfg: &VigyConfig, count: u32, tick_hz: f64) -> Segment {
    let hz = format_hz(tick_hz);
    let text = render_format(&cfg.format, count, &hz);
    Segment::new("vigy").push(StyledFragment::new(text, cfg.style.resolve()))
}

/// Format hz as integer when fractional is zero, one-decimal otherwise.
pub fn format_hz(hz: f64) -> String {
    if (hz - hz.round()).abs() < f64::EPSILON {
        // Use Display impl on integer to avoid format!() of payload.
        (hz.round() as i64).to_string()
    } else {
        // One-decimal precision via integer math (no format!()).
        let scaled = (hz * 10.0).round() as i64;
        let int_part = scaled / 10;
        let frac_part = (scaled % 10).abs();
        let mut s = int_part.to_string();
        s.push('.');
        s.push_str(&frac_part.to_string());
        s
    }
}

/// Probe `http://<host>:<port><path>` with a hard timeout. Returns
/// `Some((count, tick_hz))` on success, `None` on any failure.
///
/// Speaks raw HTTP/1.1 over a `TcpStream` to avoid pulling
/// `ureq`/`reqwest`/`tokio` into the dep tree. The vigy snapshot is
/// small (<2 KiB), so we read once into a Vec and parse.
fn probe_vigy(host: &str, port: u16, path: &str, timeout_ms: u64) -> Option<(u32, f64)> {
    let timeout = Duration::from_millis(timeout_ms);
    let addr = (host, port).to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, timeout).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    // Minimal HTTP/1.1 request — built from typed components, not
    // a format!() of HTTP syntax.
    let mut req = Vec::with_capacity(64 + path.len() + host.len());
    req.extend_from_slice(b"GET ");
    req.extend_from_slice(path.as_bytes());
    req.extend_from_slice(b" HTTP/1.1\r\nHost: ");
    req.extend_from_slice(host.as_bytes());
    req.extend_from_slice(b"\r\nConnection: close\r\nAccept: application/json\r\n\r\n");
    stream.write_all(&req).ok()?;
    let mut buf = Vec::with_capacity(2048);
    stream.read_to_end(&mut buf).ok()?;
    let response = std::str::from_utf8(&buf).ok()?;
    let body = http_body(response)?;
    parse_vigy_snapshot(body)
}

/// Strip the HTTP status + headers from an HTTP/1.1 response and
/// return the body slice.
pub fn http_body(response: &str) -> Option<&str> {
    let (head, body) = response.split_once("\r\n\r\n")?;
    // Status line must start with `HTTP/1.1 2xx` to be considered ok.
    let status_line = head.lines().next()?;
    let code_token = status_line.split_whitespace().nth(1)?;
    let code: u16 = code_token.parse().ok()?;
    if !(200..300).contains(&code) {
        return None;
    }
    Some(body)
}

/// Parse the vigy snapshot JSON body. Expected shape:
///
/// ```json
/// {"count": 12, "tick_hz": 4.0}
/// ```
///
/// (Vigy's actual `/reconcilers` response may be richer; we extract
/// the two fields we render and ignore everything else — matches the
/// segment doc contract.)
pub fn parse_vigy_snapshot(body: &str) -> Option<(u32, f64)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let count = value.get("count")?.as_u64()? as u32;
    let tick_hz = value.get("tick_hz")?.as_f64()?;
    Some((count, tick_hz))
}

/// Render the format string. Substitutions: `$count`, `$hz`.
/// Mirrors `shikumi_tier::render_format` field-for-field.
pub fn render_format(fmt: &str, count: u32, hz: &str) -> String {
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
                "hz" => out.push_str(hz),
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

    #[test]
    fn format_hz_integer() {
        assert_eq!(format_hz(4.0), "4");
    }

    #[test]
    fn format_hz_one_decimal() {
        assert_eq!(format_hz(2.5), "2.5");
    }

    #[test]
    fn parse_vigy_snapshot_extracts_fields() {
        let body = r#"{"count": 12, "tick_hz": 4.0}"#;
        let parsed = parse_vigy_snapshot(body).unwrap();
        assert_eq!(parsed.0, 12);
        assert!((parsed.1 - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_vigy_snapshot_rejects_garbage() {
        assert!(parse_vigy_snapshot("not json").is_none());
        assert!(parse_vigy_snapshot(r#"{"wrong": "shape"}"#).is_none());
    }

    #[test]
    fn http_body_extracts_after_blank_line() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        assert_eq!(http_body(resp), Some("hello"));
    }

    #[test]
    fn http_body_rejects_non_2xx() {
        let resp = "HTTP/1.1 404 Not Found\r\n\r\nnope";
        assert_eq!(http_body(resp), None);
    }

    #[test]
    fn render_format_substitutes_count_and_hz() {
        let out = render_format("[vigy: $count @ $hz Hz]($style)", 7, "4");
        assert_eq!(out, "vigy: 7 @ 4 Hz");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = VigyConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.host, "");
        assert_eq!(cfg.port, 0);
    }

    #[test]
    fn default_uses_nord_frost_blue() {
        let cfg = VigyConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #81A1C1");
        assert_eq!(cfg.port, 38_821);
        assert_eq!(cfg.path, "/reconcilers");
    }

    #[test]
    fn probe_unreachable_returns_none() {
        // Connect to a port that's almost certainly closed; must
        // return None within the timeout.
        let result = probe_vigy("127.0.0.1", 1, "/reconcilers", 100);
        assert!(result.is_none());
    }
}
