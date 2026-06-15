//! `mado_session` segment — surfaces live mado session count + fps.
//!
//! Pleme-io-native (Tier 3). Connects to the mado MCP Unix socket at
//! `MADO_SOCKET` (default `<home>/.local/share/mado/mado.sock`), sends
//! a minimal MCP `list_sessions` query, and emits a Nord-frost segment.
//!
//! ## Theme
//!
//! Nord-frost blue `#81A1C1`.
//!
//! ## Probe budget
//!
//! Unix socket connect + send + recv, hard-bounded by
//! `command_timeout_ms`. Gracefully absent on any failure.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::mado_session::MadoSessionConfig,
    segment::StyledFragment,
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct CachedSnapshot {
    sessions: u32,
    fps: u32,
    captured_at: Instant,
}

pub struct MadoSessionModule {
    cfg: MadoSessionConfig,
    cache: Mutex<Option<CachedSnapshot>>,
}

impl MadoSessionModule {
    pub fn new(cfg: MadoSessionConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(None),
        }
    }
}

impl Module for MadoSessionModule {
    fn name(&self) -> &'static str {
        "mado_session"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cached = self.cache.lock().ok().and_then(|g| *g);
        if let Some(entry) = cached {
            if now.duration_since(entry.captured_at) < ttl {
                return Ok(Some(build_segment(
                    &self.cfg,
                    entry.sessions,
                    entry.fps,
                    false,
                )));
            }
        }
        let Some(socket) =
            resolve_socket_path(&self.cfg.socket_path, env_lookup, ctx.home.as_deref())
        else {
            return Ok(None);
        };
        if !socket.exists() {
            return Ok(None);
        }
        let probe = probe_mado(&socket, self.cfg.command_timeout_ms);
        match probe {
            Some((sessions, fps)) => {
                if let Ok(mut g) = self.cache.lock() {
                    *g = Some(CachedSnapshot {
                        sessions,
                        fps,
                        captured_at: now,
                    });
                }
                Ok(Some(build_segment(&self.cfg, sessions, fps, false)))
            }
            None => {
                if let Some(entry) = cached {
                    Ok(Some(build_segment(
                        &self.cfg,
                        entry.sessions,
                        entry.fps,
                        true,
                    )))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

/// Resolve the configured socket path. `"$env"` reads `MADO_SOCKET`
/// (falling back to `<home>/.local/share/mado/mado.sock`).
pub fn resolve_socket_path<F>(
    socket_path: &str,
    lookup: F,
    home: Option<&Path>,
) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    // Env-var name from the typed cross-tool contract — the SAME source
    // mado (the producer) exports MADO_SOCKET from.
    use ishou_tokens::FleetStateVar;
    if socket_path.is_empty() {
        return None;
    }
    if socket_path == "$env" {
        if let Some(env_path) = lookup(FleetStateVar::MadoSocket.name()) {
            return Some(PathBuf::from(env_path));
        }
        let home = home?;
        return Some(home.join(".local/share/mado/mado.sock"));
    }
    Some(PathBuf::from(socket_path))
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

fn build_segment(cfg: &MadoSessionConfig, sessions: u32, fps: u32, stale: bool) -> Segment {
    let status_label = format_status(sessions, fps, stale);
    let text = render_format(&cfg.format, sessions, fps, &status_label);
    let style = cfg.style.resolve();
    Segment::new("mado_session").push(StyledFragment::new(text, style))
}

pub fn format_status(sessions: u32, fps: u32, stale: bool) -> String {
    let mut s = String::from("mado: ");
    s.push_str(&sessions.to_string());
    s.push_str(" sess, ");
    s.push_str(&fps.to_string());
    s.push_str(" fps");
    if stale {
        s.push_str(" (stale)");
    }
    s
}

pub fn render_format(fmt: &str, sessions: u32, fps: u32, status: &str) -> String {
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
                "sessions" => out.push_str(&sessions.to_string()),
                "fps" => out.push_str(&fps.to_string()),
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

fn probe_mado(socket: &Path, timeout_ms: u64) -> Option<(u32, u32)> {
    let socket = socket.to_path_buf();
    let (tx, rx) = mpsc::channel::<Option<(u32, u32)>>();
    std::thread::spawn(move || {
        let result = run_mado_probe(&socket, Duration::from_millis(timeout_ms));
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .ok()
        .and_then(|r| r)
}

fn run_mado_probe(socket: &Path, timeout: Duration) -> Option<(u32, u32)> {
    let mut stream = UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;

    let request = "{\"jsonrpc\":\"2.0\",\"method\":\"list_sessions\",\"id\":1}\n";
    stream.write_all(request.as_bytes()).ok()?;

    let mut buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 256];
    while buf.len() < 4096 {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.contains(&b'\n') {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf);
    parse_mado_response(&text)
}

/// Parse the mado MCP response for `(sessions, fps)`. Tolerantly
/// looks for top-level integer fields; falls back to counting
/// `"id":` occurrences when summary fields are absent.
pub fn parse_mado_response(body: &str) -> Option<(u32, u32)> {
    let sessions = extract_u32_field(body, "sessions")
        .or_else(|| Some(count_substring(body, "\"id\":") as u32))?;
    let fps = extract_u32_field(body, "fps").unwrap_or(0);
    Some((sessions, fps))
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

fn count_substring(body: &str, needle: &str) -> usize {
    body.matches(needle).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn stub_lookup(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).map(|s| (*s).to_owned())
    }

    /// Forcing function: the MADO_SOCKET name this segment reads comes
    /// from the typed cross-tool contract (`ishou_tokens::FleetStateVar`)
    /// — the SAME source mado (the producer) exports it from. A rename on
    /// either side is a compile+test failure here.
    #[test]
    fn mado_socket_env_var_name_comes_from_fleet_state_contract() {
        use ishou_tokens::FleetStateVar;
        assert_eq!(FleetStateVar::MadoSocket.name(), "MADO_SOCKET");
    }

    #[test]
    fn resolve_socket_explicit_path() {
        let lookup = stub_lookup(HashMap::new());
        let r = resolve_socket_path("/tmp/x.sock", lookup, None);
        assert_eq!(r, Some(PathBuf::from("/tmp/x.sock")));
    }

    #[test]
    fn resolve_socket_env_marker_uses_lookup() {
        let lookup = stub_lookup(HashMap::from([(
            ishou_tokens::FleetStateVar::MadoSocket.name(),
            "/run/mado.sock",
        )]));
        let r = resolve_socket_path("$env", lookup, None);
        assert_eq!(r, Some(PathBuf::from("/run/mado.sock")));
    }

    #[test]
    fn resolve_socket_env_marker_default_fallback() {
        let lookup = stub_lookup(HashMap::new());
        let home = PathBuf::from("/u/luis");
        let r = resolve_socket_path("$env", lookup, Some(&home));
        assert_eq!(
            r,
            Some(PathBuf::from("/u/luis/.local/share/mado/mado.sock"))
        );
    }

    #[test]
    fn resolve_socket_empty_disables() {
        let lookup = stub_lookup(HashMap::new());
        assert_eq!(resolve_socket_path("", lookup, None), None);
    }

    #[test]
    fn resolve_socket_env_with_no_home_returns_none() {
        let lookup = stub_lookup(HashMap::new());
        assert_eq!(resolve_socket_path("$env", lookup, None), None);
    }

    #[test]
    fn parse_mado_response_explicit_fields() {
        let body = r#"{"sessions": 3, "fps": 60, "ok": true}"#;
        assert_eq!(parse_mado_response(body), Some((3, 60)));
    }

    #[test]
    fn parse_mado_response_falls_back_to_id_scan() {
        let body = r#"{"data":[{"id":1},{"id":2},{"id":3},{"id":4}]}"#;
        assert_eq!(parse_mado_response(body), Some((4, 0)));
    }

    #[test]
    fn parse_mado_response_fps_default_zero() {
        let body = r#"{"sessions": 1}"#;
        assert_eq!(parse_mado_response(body), Some((1, 0)));
    }

    #[test]
    fn format_status_no_stale() {
        assert_eq!(format_status(2, 60, false), "mado: 2 sess, 60 fps");
    }

    #[test]
    fn format_status_zero_sessions() {
        assert_eq!(format_status(0, 0, false), "mado: 0 sess, 0 fps");
    }

    #[test]
    fn format_status_stale_suffix() {
        assert_eq!(format_status(1, 30, true), "mado: 1 sess, 30 fps (stale)");
    }

    #[test]
    fn render_format_default_template() {
        let out = render_format("[$status]($style)", 2, 60, "mado: 2 sess, 60 fps");
        assert_eq!(out, "mado: 2 sess, 60 fps");
    }

    #[test]
    fn render_format_sessions_substitution() {
        let out = render_format("sess=$sessions fps=$fps", 4, 120, "_");
        assert_eq!(out, "sess=4 fps=120");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = MadoSessionConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.socket_path, "");
    }

    #[test]
    fn default_uses_nord_frost_palette() {
        let cfg = MadoSessionConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #81A1C1");
    }

    #[test]
    fn default_disabled_per_tier3_policy() {
        assert!(!MadoSessionConfig::default().enabled);
    }

    #[test]
    fn missing_socket_renders_nothing() {
        let cfg = MadoSessionConfig {
            enabled: true,
            socket_path: "/tmp/seki-mado-nonexistent-zzz.sock".to_owned(),
            ..MadoSessionConfig::default()
        };
        let module = MadoSessionModule::new(cfg);
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
