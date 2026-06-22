//! `hostname` segment — reads the system hostname, optionally
//! truncating at `trim_at`. Honours `ssh_only` by checking
//! `SSH_CONNECTION`.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::hostname::HostnameConfig,
    segment::StyledFragment,
};
use std::ffi::OsStr;

pub struct HostnameModule {
    cfg: HostnameConfig,
}

impl HostnameModule {
    pub fn new(cfg: HostnameConfig) -> Self {
        Self { cfg }
    }
}

impl Module for HostnameModule {
    fn name(&self) -> &'static str {
        "hostname"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        if self.cfg.ssh_only && std::env::var_os("SSH_CONNECTION").is_none() {
            return Ok(None);
        }
        let Some(host) = read_hostname() else {
            return Ok(None);
        };
        let trimmed = trim_at(&host, &self.cfg.trim_at);
        let text = seki_core::format::render_one(&self.cfg.format, "hostname", &trimmed);
        Ok(Some(Segment::new("hostname").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

pub fn trim_at(host: &str, sep: &str) -> String {
    if sep.is_empty() {
        return host.to_owned();
    }
    match host.find(sep) {
        Some(idx) => host[..idx].to_owned(),
        None => host.to_owned(),
    }
}

pub fn read_hostname() -> Option<String> {
    // /etc/hostname on linux; HOSTNAME on most shells; fallback
    // to libc gethostname via std::env::var. We deliberately don't
    // shell out (NO SHELL).
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.is_empty() {
            return Some(h);
        }
    }
    let bytes = match hostname_libc() {
        Some(b) => b,
        None => return std::fs::read_to_string("/etc/hostname")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
    };
    Some(bytes)
}

#[cfg(unix)]
fn hostname_libc() -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes a NUL-terminated string into the
    // buffer; we then locate the NUL and decode as UTF-8.
    let ret = unsafe {
        libc_gethostname(buf.as_mut_ptr().cast::<i8>(), buf.len())
    };
    if ret != 0 {
        return None;
    }
    let nul = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    let s = OsStr::from_bytes(&buf[..nul]).to_string_lossy().into_owned();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(not(unix))]
fn hostname_libc() -> Option<String> {
    None
}

#[cfg(unix)]
unsafe extern "C" {
    #[link_name = "gethostname"]
    fn libc_gethostname(name: *mut i8, len: usize) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use seki_core::format::render_one;

    #[test]
    fn trim_at_dot() {
        assert_eq!(trim_at("foo.example.com", "."), "foo");
        assert_eq!(trim_at("foo", "."), "foo");
        assert_eq!(trim_at("foo.example.com", ""), "foo.example.com");
    }

    #[test]
    fn render_format_strips_markup() {
        let out = render_one("[$hostname](dimmed $style) · ", "hostname", "rio");
        assert_eq!(out, "rio · ");
    }

    #[test]
    fn render_format_handles_plain_hostname() {
        let out = render_one("$hostname", "hostname", "rio");
        assert_eq!(out, "rio");
    }
}
