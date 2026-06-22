//! `arnes_cache` segment — surfaces P2P cache hit rate in the prompt.
//!
//! Pleme-io-native (Tier 5). Connects to the arnes daemon over a
//! Unix socket (`~/.local/share/arnes/arnes.sock` by default, or
//! `ARNES_SOCKET`), issues a single minimal stats request, parses
//! the response, and renders the cache hit rate as `[arnes: NN%]`.
//!
//! ## Theme
//!
//! Tristate Nord-aurora — same vocabulary as `tend`'s heavy/light/clean:
//!
//! - `>= 0.80` hit rate → green `#A3BE8C` (warm)
//! - `>= 0.50` and `< 0.80` → yellow `#EBCB8B` (lukewarm)
//! - `< 0.50` → red `#BF616A` (cold)
//!
//! ## Probe budget
//!
//! UnixStream connect + tiny HTTP-shaped request + bounded read. The
//! per-IO timeout is set on the stream; the outer thread +
//! `mpsc::recv_timeout` enforces the wall-clock budget regardless of
//! syscall blocking.
//!
//! ## Graceful absence
//!
//! Socket absent, connect refused, parse failure, timeout → returns
//! `Ok(None)`. The arnes daemon will routinely be absent on developer
//! laptops; the prompt must remain silent in that case.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::arnes_cache::ArnesCacheConfig,
    segment::StyledFragment,
    style::{Style, StyleSpec},
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

/// Cached hit-rate value (0.0..=1.0).
#[derive(Debug, Clone, Copy)]
struct CachedRate {
    rate: f32,
    captured_at: Instant,
}

pub struct ArnesCacheModule {
    cfg: ArnesCacheConfig,
    cache: Mutex<Option<CachedRate>>,
}

impl ArnesCacheModule {
    pub fn new(cfg: ArnesCacheConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for ArnesCacheModule {
    fn name(&self) -> &'static str {
        "arnes_cache"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // 1. Cache fast-path.
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached: Option<CachedRate> = match self.cache.lock() {
            Ok(g) => *g,
            Err(_) => None,
        };
        if let Some(entry) = cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(&self.cfg, entry.rate, false)));
            }
        }

        // 2. Resolve socket path.
        let Some(sock) = resolve_socket(&self.cfg, ctx.home.as_deref()) else {
            return Ok(None);
        };

        // 3. Probe.
        match probe_arnes(&sock, self.cfg.command_timeout_ms) {
            Some(rate) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedRate {
                        rate,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, rate, false)))
            }
            None => {
                if let Some(entry) = cached {
                    Ok(Some(build_segment(&self.cfg, entry.rate, true)))
                } else {
                    // Gracefully absent.
                    Ok(None)
                }
            }
        }
    }
}

/// Resolve the effective socket path. The env-var override wins;
/// otherwise the configured path is taken relative to `$HOME` if
/// not absolute. Returns `None` when the path resolves to empty.
pub fn resolve_socket(cfg: &ArnesCacheConfig, home: Option<&Path>) -> Option<PathBuf> {
    if !cfg.socket_env_var.is_empty() {
        if let Ok(v) = std::env::var(&cfg.socket_env_var) {
            if !v.is_empty() {
                return Some(PathBuf::from(v));
            }
        }
    }
    if cfg.socket_path.is_empty() {
        return None;
    }
    let p = PathBuf::from(&cfg.socket_path);
    if p.is_absolute() {
        Some(p)
    } else {
        // Anchor relative paths under $HOME when available; otherwise
        // accept the relative form (caller cwd-dependent — fine).
        Some(match home {
            Some(h) => h.join(p),
            None => p,
        })
    }
}

fn build_segment(cfg: &ArnesCacheConfig, rate: f32, stale: bool) -> Segment {
    let pct = rate_to_pct(rate);
    let status = format_status(pct, stale);
    let text = seki_core::format::render(&cfg.format, |__n| match __n {
        "pct" => Some(pct.to_string()),
        "status" => Some(status.to_owned()),
        _ => None,
    });
    let style = pick_style(cfg, rate);
    Segment::new("arnes_cache").push(StyledFragment::new(text, style))
}

/// Clamp + round a `0.0..=1.0` hit rate to a `0..=100` integer percent.
pub fn rate_to_pct(rate: f32) -> u32 {
    let clamped = rate.clamp(0.0, 1.0);
    (clamped * 100.0 + 0.5) as u32
}

/// Pick the tier-derived style. Mirrors the tend tiering vocabulary.
pub fn pick_style(cfg: &ArnesCacheConfig, rate: f32) -> Style {
    let spec: &StyleSpec = if rate >= cfg.warm_threshold {
        &cfg.warm_style
    } else if rate >= cfg.cold_threshold {
        &cfg.lukewarm_style
    } else {
        &cfg.cold_style
    };
    spec.resolve()
}

/// `arnes: NN%`. Stale renders append ` (stale)`.
pub fn format_status(pct: u32, stale: bool) -> String {
    let mut s = String::from("arnes: ");
    s.push_str(&pct.to_string());
    s.push('%');
    if stale {
        s.push_str(" (stale)");
    }
    s
}

/// Connect to the arnes Unix socket, send a minimal stats request,
/// read a bounded prefix, and parse `hit_rate` from the JSON body.
/// Returns the rate clamped to `0.0..=1.0`, or `None` on any failure.
fn probe_arnes(sock: &Path, timeout_ms: u64) -> Option<f32> {
    let sock = sock.to_path_buf();
    let (tx, rx) = mpsc::channel::<Option<f32>>();
    std::thread::spawn(move || {
        let result = do_probe(&sock, timeout_ms);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

fn do_probe(sock: &Path, timeout_ms: u64) -> Option<f32> {
    let to = Duration::from_millis(timeout_ms);
    // Cheap pre-check — avoids syscall storms on hosts without arnes.
    if !sock.exists() {
        return None;
    }
    let mut stream = UnixStream::connect(sock).ok()?;
    stream.set_read_timeout(Some(to)).ok()?;
    stream.set_write_timeout(Some(to)).ok()?;
    let request = build_request();
    stream.write_all(request.as_bytes()).ok()?;
    // Read a bounded prefix — arnes stats response is small.
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 512];
    // Two reads max — keeps us under the wall budget without
    // hanging on a slow body.
    for _ in 0..2 {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }
    let text = std::str::from_utf8(&buf).ok()?;
    parse_hit_rate(text)
}

/// Build the HTTP/1.0 stats request the arnes daemon serves. The
/// daemon implements a tiny HTTP-shaped Unix-socket surface; the
/// stats endpoint is `/v1/stats`.
pub fn build_request() -> String {
    String::from("GET /v1/stats HTTP/1.0\r\nHost: arnes\r\nConnection: close\r\n\r\n")
}

/// Extract the `hit_rate` value from an HTTP response body. Tolerant
/// of formatting variations (whitespace, body-without-headers).
/// Returns the parsed `f32` clamped to `0.0..=1.0`, or `None` on
/// missing-key / unparseable cases.
pub fn parse_hit_rate(text: &str) -> Option<f32> {
    let needle = "\"hit_rate\"";
    let idx = text.find(needle)?;
    let after = &text[idx + needle.len()..];
    let after = after.trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    // Read the numeric token — digits + optional `.`/`-`/`e`/`E`.
    let mut end = 0;
    for (i, ch) in after.char_indices() {
        let is_numeric = ch.is_ascii_digit()
            || ch == '.'
            || ch == '-'
            || ch == '+'
            || ch == 'e'
            || ch == 'E';
        if is_numeric {
            end = i + ch.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let value = &after[..end];
    let parsed: f32 = value.parse().ok()?;
    Some(parsed.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_to_pct_rounding() {
        assert_eq!(rate_to_pct(0.0), 0);
        assert_eq!(rate_to_pct(0.5), 50);
        assert_eq!(rate_to_pct(0.876), 88);
        assert_eq!(rate_to_pct(1.0), 100);
    }

    #[test]
    fn rate_to_pct_clamps_out_of_range() {
        assert_eq!(rate_to_pct(-0.5), 0);
        assert_eq!(rate_to_pct(1.5), 100);
    }

    #[test]
    fn parse_hit_rate_simple() {
        let body = r#"{"hit_rate":0.81,"miss":3}"#;
        assert!((parse_hit_rate(body).unwrap() - 0.81).abs() < 1e-4);
    }

    #[test]
    fn parse_hit_rate_with_whitespace() {
        let body = r#"{ "hit_rate"  :   0.42 }"#;
        assert!((parse_hit_rate(body).unwrap() - 0.42).abs() < 1e-4);
    }

    #[test]
    fn parse_hit_rate_missing_returns_none() {
        let body = r#"{"hits":12,"misses":3}"#;
        assert_eq!(parse_hit_rate(body), None);
    }

    #[test]
    fn parse_hit_rate_clamps() {
        // Out-of-range value still parses; clamp protects downstream
        // bucketing.
        let body = r#"{"hit_rate":1.5}"#;
        assert!((parse_hit_rate(body).unwrap() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn format_status_rounded() {
        assert_eq!(format_status(81, false), "arnes: 81%");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(42, true), "arnes: 42% (stale)");
    }

    #[test]
    fn render_format_default_template() {
        let out = seki_core::format::render("[$status]($style)", |n| match n {
            "status" => Some("arnes: 73%".to_owned()),
            _ => None,
        });
        assert_eq!(out, "arnes: 73%");
    }

    #[test]
    fn render_format_pct_substitution() {
        let out = seki_core::format::render("$pct%", |n| match n {
            "pct" => Some(50u32.to_string()),
            _ => None,
        });
        assert_eq!(out, "50%");
    }

    #[test]
    fn pick_style_warm_lukewarm_cold() {
        let cfg = ArnesCacheConfig::default();
        // warm_threshold = 0.80, cold_threshold = 0.50
        assert_eq!(pick_style(&cfg, 0.95), cfg.warm_style.resolve());
        assert_eq!(pick_style(&cfg, 0.80), cfg.warm_style.resolve());
        assert_eq!(pick_style(&cfg, 0.65), cfg.lukewarm_style.resolve());
        assert_eq!(pick_style(&cfg, 0.50), cfg.lukewarm_style.resolve());
        assert_eq!(pick_style(&cfg, 0.10), cfg.cold_style.resolve());
    }

    #[test]
    fn resolve_socket_env_override() {
        let key = "SEKI_TEST_ARNES_SOCK_OVERRIDE";
        unsafe {
            std::env::set_var(key, "/run/arnes-override.sock");
        }
        let cfg = ArnesCacheConfig {
            socket_env_var: key.to_owned(),
            ..ArnesCacheConfig::default()
        };
        assert_eq!(
            resolve_socket(&cfg, Some(Path::new("/home/u"))),
            Some(PathBuf::from("/run/arnes-override.sock"))
        );
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn resolve_socket_relative_under_home() {
        let cfg = ArnesCacheConfig {
            socket_env_var: "SEKI_TEST_ARNES_DEFINITELY_UNSET".to_owned(),
            socket_path: ".local/share/arnes/arnes.sock".to_owned(),
            ..ArnesCacheConfig::default()
        };
        let got = resolve_socket(&cfg, Some(Path::new("/home/u"))).unwrap();
        assert_eq!(got, PathBuf::from("/home/u/.local/share/arnes/arnes.sock"));
    }

    #[test]
    fn resolve_socket_absolute_path_preserved() {
        let cfg = ArnesCacheConfig {
            socket_env_var: "SEKI_TEST_ARNES_DEFINITELY_UNSET".to_owned(),
            socket_path: "/var/run/arnes.sock".to_owned(),
            ..ArnesCacheConfig::default()
        };
        let got = resolve_socket(&cfg, Some(Path::new("/home/u"))).unwrap();
        assert_eq!(got, PathBuf::from("/var/run/arnes.sock"));
    }

    #[test]
    fn build_request_shape() {
        let req = build_request();
        assert!(req.starts_with("GET /v1/stats HTTP/1.0\r\n"));
        assert!(req.contains("Connection: close\r\n"));
        assert!(req.ends_with("\r\n\r\n"));
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = ArnesCacheConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.socket_path, "");
    }

    #[test]
    fn default_uses_nord_palette() {
        let cfg = ArnesCacheConfig::default();
        assert_eq!(cfg.warm_style.as_str(), "bold #A3BE8C");
        assert_eq!(cfg.lukewarm_style.as_str(), "bold #EBCB8B");
        assert_eq!(cfg.cold_style.as_str(), "bold #BF616A");
    }

    #[test]
    fn tier5_default_is_disabled() {
        assert!(!ArnesCacheConfig::default().enabled);
    }

    #[test]
    fn missing_socket_renders_nothing() {
        let cfg = ArnesCacheConfig {
            enabled: true,
            socket_env_var: "SEKI_TEST_ARNES_DEFINITELY_UNSET".to_owned(),
            socket_path: "/tmp/seki-arnes-nonexistent-zzz.sock".to_owned(),
            ..ArnesCacheConfig::default()
        };
        let module = ArnesCacheModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
