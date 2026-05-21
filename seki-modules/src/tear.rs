//! `tear` segment — surfaces current tear session + pane identity.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Reads
//! `TEAR_SESSION_NAME` + `TEAR_PANE_ID` from the process env and
//! renders a compact `[~ <session>] [pane <id6>]` where `<id6>` is
//! the first `pane_id_len` chars of `TEAR_PANE_ID`. Tells the
//! operator at a glance which tear session+pane they're driving.
//!
//! ## Theme
//!
//! Nord-frost cyan `#88C0D0` — pleme-io's load-bearing frost colour.
//!
//! ## Probe budget
//!
//! Env-var read only — bypasses `scan_timeout_ms`. No subprocess,
//! no network, no filesystem.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::tear::TearConfig,
    segment::StyledFragment,
};

pub struct TearModule {
    cfg: TearConfig,
}

impl TearModule {
    pub fn new(cfg: TearConfig) -> Self {
        Self { cfg }
    }
}

impl Module for TearModule {
    fn name(&self) -> &'static str {
        "tear"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // Both env vars must be set — outside a tear pane the
        // segment renders nothing.
        let session = match env_lookup("TEAR_SESSION_NAME") {
            Some(s) => s,
            None => return Ok(None),
        };
        let pane = match env_lookup("TEAR_PANE_ID") {
            Some(p) => p,
            None => return Ok(None),
        };
        let pane_short = truncate(&pane, self.cfg.pane_id_len);
        let text = render_format(&self.cfg.format, &session, &pane_short);
        Ok(Some(
            Segment::new("tear").push(StyledFragment::new(text, self.cfg.style.resolve())),
        ))
    }
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Truncate a string to at most `n` chars (NOT bytes — operator-safe
/// against multibyte pane IDs even though TEAR_PANE_ID is always
/// ASCII-hex in practice).
pub fn truncate(s: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    s.chars().take(n).collect()
}

/// Render the format string. Substitutions: `$session`, `$pane`.
/// Mirrors `shikumi_tier::render_format` field-for-field.
pub fn render_format(fmt: &str, session: &str, pane: &str) -> String {
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
                "session" => out.push_str(session),
                "pane" => out.push_str(pane),
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
    fn truncate_caps_at_n() {
        assert_eq!(truncate("abcdefgh", 6), "abcdef");
    }

    #[test]
    fn truncate_handles_short_input() {
        assert_eq!(truncate("abc", 6), "abc");
    }

    #[test]
    fn truncate_zero_returns_empty() {
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn render_format_session_and_pane() {
        let out = render_format("[~ $session] [pane $pane]($style)", "demo", "abc123");
        assert_eq!(out, "~ demo pane abc123");
    }

    #[test]
    fn render_format_plain_template() {
        let out = render_format("$session/$pane", "s", "p");
        assert_eq!(out, "s/p");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = TearConfig::bare();
        assert!(!cfg.enabled);
        assert_eq!(cfg.pane_id_len, 0);
    }

    #[test]
    fn default_uses_nord_frost_palette() {
        let cfg = TearConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #88C0D0");
        assert_eq!(cfg.pane_id_len, 6);
        assert!(cfg.enabled);
    }

    #[test]
    fn render_renders_nothing_without_env_vars() {
        // SAFETY: best-effort: clear both env vars before this test.
        // Test runs in parallel with others; we use distinct vars
        // (TEAR_*) which are extremely unlikely to be set in CI.
        // SAFETY: env_remove_var/set_var are unsafe in edition 2024
        // but this test is single-threaded WRT these vars.
        unsafe {
            std::env::remove_var("TEAR_SESSION_NAME");
            std::env::remove_var("TEAR_PANE_ID");
        }
        let module = TearModule::new(TearConfig {
            enabled: true,
            ..TearConfig::default()
        });
        let ctx = RenderContext::from_env().with_colors(false);
        assert!(module.render(&ctx).unwrap().is_none());
    }
}
