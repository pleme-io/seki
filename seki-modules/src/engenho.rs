//! `engenho` segment — surfaces engenho K8s runtime readiness.
//!
//! Pleme-io-native (Tier 5). Issues a hand-rolled HTTP/1.0 GET against
//! `<addr><path>` (default `127.0.0.1:6443/readyz`) with a hard
//! `scan_timeout_ms` ceiling. Renders one of two Nord-aurora
//! tristate-style outcomes:
//!
//! - `[engenho: ready]` (green `#A3BE8C`) — 200 response
//! - `[engenho: degraded]` (red `#BF616A`) — non-200 response
//! - (segment absent)            — connect failed / timeout / unreachable
//!
//! ## Theme
//!
//! Mirrors the existing `tend` heavy/light/clean tiering vocabulary:
//! green = ok, red = visible failure, absence = "not on this host".
//!
//! ## Probe budget
//!
//! Hand-rolled TCP `\r\n`-terminated HTTP/1.0 GET — no `tokio`, no
//! `ureq` (per brief). The thread + `mpsc::recv_timeout` pattern from
//! `tend.rs` enforces the wall-clock budget regardless of TcpStream
//! socket timeouts.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::engenho::EngenhoConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

/// Outcome of one HTTP probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyState {
    Ready,
    Degraded,
}

/// Cached probe result.
#[derive(Debug, Clone)]
struct CachedState {
    state: ReadyState,
    captured_at: Instant,
}

pub struct EngenhoModule {
    cfg: EngenhoConfig,
    cache: Mutex<Option<CachedState>>,
}

impl EngenhoModule {
    pub fn new(cfg: EngenhoConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for EngenhoModule {
    fn name(&self) -> &'static str {
        "engenho"
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
                return Ok(Some(build_segment(&self.cfg, entry.state, false)));
            }
        }

        // 2. Resolve addr (env override → cfg.addr).
        let addr = resolve_addr(&self.cfg);
        if addr.is_empty() {
            return Ok(None);
        }

        // 3. Probe.
        match probe_engenho(&addr, &self.cfg.path, self.cfg.scan_timeout_ms) {
            Some(state) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedState {
                        state,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, state, false)))
            }
            None => {
                if let Some(entry) = cached {
                    Ok(Some(build_segment(&self.cfg, entry.state, true)))
                } else {
                    // Gracefully absent — engenho unreachable.
                    Ok(None)
                }
            }
        }
    }
}

/// Resolve the effective `host:port` for the probe. The env-var
/// override wins when set + non-empty.
pub fn resolve_addr(cfg: &EngenhoConfig) -> String {
    if !cfg.addr_env_var.is_empty() {
        if let Ok(v) = std::env::var(&cfg.addr_env_var) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    cfg.addr.clone()
}

fn build_segment(cfg: &EngenhoConfig, state: ReadyState, stale: bool) -> Segment {
    let status = format_status(state, stale);
    let state_word = match state {
        ReadyState::Ready => "ready",
        ReadyState::Degraded => "degraded",
    };
    let text = seki_core::format::render(&cfg.format, |__n| match __n {
        "status" => Some(status.to_owned()),
        "state" => Some(state_word.to_owned()),
        _ => None,
    });
    let style = pick_style(cfg, state);
    Segment::new("engenho").push(StyledFragment::new(text, style))
}

/// Pick `ready_style` or `degraded_style`.
pub fn pick_style(cfg: &EngenhoConfig, state: ReadyState) -> Style {
    let spec: &StyleSpec = match state {
        ReadyState::Ready => &cfg.ready_style,
        ReadyState::Degraded => &cfg.degraded_style,
    };
    spec.resolve()
}

/// `engenho: ready` / `engenho: degraded`. Stale renders append
/// ` (stale)`.
pub fn format_status(state: ReadyState, stale: bool) -> String {
    let mut s = String::from("engenho: ");
    s.push_str(match state {
        ReadyState::Ready => "ready",
        ReadyState::Degraded => "degraded",
    });
    if stale {
        s.push_str(" (stale)");
    }
    s
}

/// Probe the engenho apiserver with a hard wall-clock budget.
///
/// Returns:
/// - `Some(Ready)` — HTTP 200
/// - `Some(Degraded)` — connected + responded but non-200
/// - `None` — connect failure, timeout, malformed response
///
/// The TcpStream gets a per-IO timeout equal to the wall budget so
/// blocking reads/writes also bail; the thread + recv_timeout is the
/// ultimate ceiling (catches DNS hangs etc.).
fn probe_engenho(addr: &str, path: &str, timeout_ms: u64) -> Option<ReadyState> {
    let addr = addr.to_owned();
    let path = path.to_owned();
    let (tx, rx) = mpsc::channel::<Option<ReadyState>>();
    std::thread::spawn(move || {
        let result = do_probe(&addr, &path, timeout_ms);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

fn do_probe(addr: &str, path: &str, timeout_ms: u64) -> Option<ReadyState> {
    let to = Duration::from_millis(timeout_ms);
    // Resolve + connect with the same wall budget.
    let socket_addr = addr.to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&socket_addr, to).ok()?;
    stream.set_read_timeout(Some(to)).ok()?;
    stream.set_write_timeout(Some(to)).ok()?;
    // HTTP/1.0 + Connection: close so the server hangs up after one
    // response (no chunked encoding to parse).
    let host = addr_host(addr);
    let request = build_request(&host, path);
    stream.write_all(request.as_bytes()).ok()?;
    let mut buf = Vec::with_capacity(256);
    // Read a small bounded prefix — we only need the status line.
    let mut chunk = [0u8; 256];
    let n = stream.read(&mut chunk).ok()?;
    buf.extend_from_slice(&chunk[..n]);
    let text = std::str::from_utf8(&buf).ok()?;
    parse_status(text).map(|code| {
        if code == 200 {
            ReadyState::Ready
        } else {
            ReadyState::Degraded
        }
    })
}

/// Build the HTTP/1.0 GET request line. Typed builder over the
/// otherwise-magical string — keeps the wire format reviewable.
pub fn build_request(host: &str, path: &str) -> String {
    let path = if path.is_empty() { "/" } else { path };
    let mut out = String::with_capacity(64 + host.len() + path.len());
    out.push_str("GET ");
    out.push_str(path);
    out.push_str(" HTTP/1.0\r\nHost: ");
    out.push_str(host);
    out.push_str("\r\nConnection: close\r\n\r\n");
    out
}

/// Extract the bare host (sans port) for the Host header. Falls back
/// to the full string if no port is present.
pub fn addr_host(addr: &str) -> String {
    match addr.rfind(':') {
        Some(idx) => addr[..idx].to_owned(),
        None => addr.to_owned(),
    }
}

/// Parse the first line of an HTTP/1.x response: `HTTP/1.0 200 OK`.
/// Returns the status code or `None` if the line doesn't fit shape.
pub fn parse_status(text: &str) -> Option<u16> {
    let first = text.lines().next()?;
    let mut parts = first.split_whitespace();
    let version = parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }
    let code = parts.next()?;
    code.parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_request_shape() {
        let req = build_request("127.0.0.1", "/readyz");
        assert!(req.starts_with("GET /readyz HTTP/1.0\r\n"));
        assert!(req.contains("Host: 127.0.0.1\r\n"));
        assert!(req.contains("Connection: close\r\n"));
        assert!(req.ends_with("\r\n\r\n"));
    }

    #[test]
    fn build_request_empty_path_becomes_slash() {
        let req = build_request("h", "");
        assert!(req.starts_with("GET / HTTP/1.0\r\n"));
    }

    #[test]
    fn addr_host_strips_port() {
        assert_eq!(addr_host("127.0.0.1:6443"), "127.0.0.1");
        assert_eq!(addr_host("engenho.local:8080"), "engenho.local");
    }

    #[test]
    fn addr_host_without_port() {
        assert_eq!(addr_host("engenho.local"), "engenho.local");
    }

    #[test]
    fn parse_status_200_ok() {
        assert_eq!(parse_status("HTTP/1.0 200 OK\r\nfoo"), Some(200));
    }

    #[test]
    fn parse_status_503() {
        assert_eq!(parse_status("HTTP/1.1 503 Service Unavailable\r\n"), Some(503));
    }

    #[test]
    fn parse_status_garbage() {
        assert_eq!(parse_status("not a status line"), None);
        assert_eq!(parse_status(""), None);
    }

    #[test]
    fn format_status_ready() {
        assert_eq!(format_status(ReadyState::Ready, false), "engenho: ready");
    }

    #[test]
    fn format_status_degraded() {
        assert_eq!(
            format_status(ReadyState::Degraded, false),
            "engenho: degraded"
        );
    }

    #[test]
    fn format_status_stale() {
        assert_eq!(
            format_status(ReadyState::Ready, true),
            "engenho: ready (stale)"
        );
    }

    #[test]
    fn render_format_default_template() {
        let out = seki_core::format::render("[$status]($style)", |n| match n {
            "status" => Some("engenho: ready".to_owned()),
            _ => None,
        });
        assert_eq!(out, "engenho: ready");
    }

    #[test]
    fn render_format_state_substitution() {
        let out = seki_core::format::render("$state", |n| match n {
            "state" => Some("degraded".to_owned()),
            _ => None,
        });
        assert_eq!(out, "degraded");
    }

    #[test]
    fn pick_style_ready_vs_degraded() {
        let cfg = EngenhoConfig::default();
        assert_eq!(
            pick_style(&cfg, ReadyState::Ready),
            cfg.ready_style.resolve()
        );
        assert_eq!(
            pick_style(&cfg, ReadyState::Degraded),
            cfg.degraded_style.resolve()
        );
    }

    #[test]
    fn resolve_addr_uses_env_override() {
        // Use a unique env var name to avoid clobbering anything real.
        let key = "SEKI_TEST_ENGENHO_ADDR_OVERRIDE";
        // SAFETY: writing to process env in a test — single-threaded
        // by virtue of the unique key.
        unsafe {
            std::env::set_var(key, "192.0.2.1:6443");
        }
        let cfg = EngenhoConfig {
            addr_env_var: key.to_owned(),
            addr: "127.0.0.1:6443".to_owned(),
            ..EngenhoConfig::default()
        };
        assert_eq!(resolve_addr(&cfg), "192.0.2.1:6443");
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn resolve_addr_falls_back_when_env_unset() {
        let cfg = EngenhoConfig {
            addr_env_var: "SEKI_TEST_ENGENHO_DEFINITELY_UNSET".to_owned(),
            addr: "127.0.0.1:6443".to_owned(),
            ..EngenhoConfig::default()
        };
        assert_eq!(resolve_addr(&cfg), "127.0.0.1:6443");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = EngenhoConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.addr, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = EngenhoConfig::default();
        assert_eq!(cfg.ready_style.as_str(), "bold #A3BE8C");
        assert_eq!(cfg.degraded_style.as_str(), "bold #BF616A");
    }

    #[test]
    fn tier5_default_is_disabled() {
        assert!(!EngenhoConfig::default().enabled);
    }

    #[test]
    fn unreachable_addr_renders_nothing() {
        // 192.0.2.0/24 is TEST-NET-1 (RFC 5737) — guaranteed not
        // routable. Probe should fail inside the wall budget.
        let cfg = EngenhoConfig {
            enabled: true,
            addr: "192.0.2.1:6443".to_owned(),
            addr_env_var: String::new(),
            scan_timeout_ms: 150,
            ..EngenhoConfig::default()
        };
        let module = EngenhoModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
