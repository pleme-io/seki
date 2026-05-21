//! `shikumi_config` segment — surfaces *resolved* tier per shikumi app.
//!
//! Pleme-io-native (Tier 2 per `docs/PLEME-IO-SEGMENTS.md`). Sister
//! segment to [`super::shikumi_tier`]: where `shikumi_tier` reports
//! that an `<APP>_TIER` env var is *set*, this segment probes
//! `<app> config-show <tier>` to confirm the tier resolves end-to-end
//! (the binary ships the tier, the value parses, etc.). Renders
//! nothing when the binary is missing or the probe fails — the
//! affirmative green badge is the validated-tier signal, the
//! unvalidated state is the segment's absence.
//!
//! ## Theme
//!
//! Nord-aurora green `#A3BE8C` — mirrors `tend`'s clean state.
//!
//! ## Probe budget
//!
//! Subprocesses out per active `<APP>_TIER` env var with a hard
//! per-probe timeout (`command_timeout_ms`). A `cache_ttl_secs`
//! window prevents repeated invocations across renders within a
//! shell session; stale renders annotate the segment with
//! `(stale)`. Probe failures render nothing (graceful absence).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::shikumi_config::ShikumiConfigConfig,
    segment::StyledFragment,
};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cached probe result for a single `(app, tier)` pair.
#[derive(Debug, Clone)]
struct CachedProbe {
    app: String,
    tier: String,
    captured_at: Instant,
}

pub struct ShikumiConfigModule {
    cfg: ShikumiConfigConfig,
    cache: Mutex<Vec<CachedProbe>>,
}

impl ShikumiConfigModule {
    pub fn new(cfg: ShikumiConfigConfig) -> Self {
        Self {
            cfg,
            cache: Mutex::new(Vec::new()),
        }
    }
}

impl Module for ShikumiConfigModule {
    fn name(&self) -> &'static str {
        "shikumi_config"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        // 1. Read env vars: which (app, tier) pairs are *requested*?
        let requested = scan_env(&self.cfg.apps, env_lookup);
        if requested.is_empty() {
            return Ok(None);
        }

        // 2. Bucket: cache-hit pairs vs need-probe pairs.
        let now = Instant::now();
        let ttl = Duration::from_secs(self.cfg.cache_ttl_secs);
        let cache_snapshot: Vec<CachedProbe> = self
            .cache
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default();

        let mut validated: Vec<(String, String, bool)> = Vec::new(); // (app, tier, stale)
        let mut fresh_entries: Vec<CachedProbe> = Vec::new();

        for (app, tier) in &requested {
            let cached = cache_snapshot
                .iter()
                .find(|c| c.app == *app && c.tier == *tier);
            if let Some(entry) = cached {
                if now.duration_since(entry.captured_at) < ttl {
                    validated.push((app.clone(), tier.clone(), false));
                    fresh_entries.push(entry.clone());
                    continue;
                }
            }
            // Cache miss / expired — probe.
            if probe_config_show(app, tier, self.cfg.command_timeout_ms) {
                validated.push((app.clone(), tier.clone(), false));
                fresh_entries.push(CachedProbe {
                    app: app.clone(),
                    tier: tier.clone(),
                    captured_at: now,
                });
            } else if let Some(entry) = cached {
                // Probe failed but we have stale cache — surface it
                // with a stale marker so the operator knows.
                validated.push((app.clone(), tier.clone(), true));
                fresh_entries.push(entry.clone());
            }
            // else: no cache, probe failed → drop entirely (graceful absence)
        }

        // 3. Update cache for next render.
        if let Ok(mut g) = self.cache.lock() {
            *g = fresh_entries;
        }

        if validated.is_empty() {
            return Ok(None);
        }

        let rendered: Vec<String> = validated
            .into_iter()
            .map(|(app, tier, stale)| {
                let mut s = render_format(&self.cfg.format, &app, &tier);
                if stale {
                    s.push_str(" (stale)");
                }
                s
            })
            .collect();
        let text = rendered.join(&self.cfg.separator);
        Ok(Some(
            Segment::new("shikumi_config").push(StyledFragment::new(text, self.cfg.style.resolve())),
        ))
    }
}

/// Pure-function scan: for each app, look up its `<APP>_TIER` env var
/// and return the `(app, tier)` pairs that have a non-empty value.
/// Mirrors `shikumi_tier::scan_env`.
pub fn scan_env<F>(apps: &[String], lookup: F) -> Vec<(String, String)>
where
    F: Fn(&str) -> Option<String>,
{
    apps.iter()
        .filter_map(|app| {
            let env_name = format!("{}_TIER", app.to_uppercase().replace('-', "_"));
            lookup(&env_name).map(|tier| (app.clone(), tier))
        })
        .collect()
}

fn env_lookup(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Probe `<app> config-show <tier>` with a hard timeout. Returns
/// `true` iff the subprocess exits cleanly (status 0). Spawn errors,
/// non-zero exits, and timeouts all count as failure.
///
/// Sync — uses a thread + `recv_timeout` to bound the probe without
/// pulling tokio into the dep tree (mirrors `tend::probe_tend`).
fn probe_config_show(app: &str, tier: &str, timeout_ms: u64) -> bool {
    let app_owned = app.to_owned();
    let tier_owned = tier.to_owned();
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    std::thread::spawn(move || {
        let result = run_config_show(&app_owned, &tier_owned);
        let _ = tx.send(result);
    });
    rx.recv_timeout(Duration::from_millis(timeout_ms))
        .unwrap_or(false)
}

/// Run `<app> config-show <tier>` synchronously. Returns `true` iff
/// the spawn succeeded AND the exit status was zero.
fn run_config_show(app: &str, tier: &str) -> bool {
    Command::new(app)
        .arg("config-show")
        .arg(tier)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Build the cache map view from a snapshot — useful for testing
/// (kept in case future tests need it; not used in render path).
#[cfg(test)]
fn cache_view(
    snapshot: &[CachedProbe],
) -> std::collections::HashMap<(String, String), Instant> {
    snapshot
        .iter()
        .map(|c| ((c.app.clone(), c.tier.clone()), c.captured_at))
        .collect()
}

/// Render the format string. Substitutions: `$app`, `$tier`.
/// Mirrors `shikumi_tier::render_format` field-for-field.
pub fn render_format(fmt: &str, app: &str, tier: &str) -> String {
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
                "app" => out.push_str(app),
                "tier" => out.push_str(tier),
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
    use std::collections::HashMap;

    fn stub_lookup(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).map(|s| (*s).to_owned())
    }

    #[test]
    fn scan_env_picks_up_set_vars() {
        let lookup = stub_lookup(HashMap::from([
            ("MADO_TIER", "bare"),
            ("KENSHI_TIER", "default"),
        ]));
        let apps = vec!["mado".to_string(), "kenshi".to_string(), "tatara".to_string()];
        let hits = scan_env(&apps, lookup);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0], ("mado".to_string(), "bare".to_string()));
        assert_eq!(hits[1], ("kenshi".to_string(), "default".to_string()));
    }

    #[test]
    fn scan_env_translates_hyphen_to_underscore() {
        let lookup = stub_lookup(HashMap::from([("ZOEKT_MCP_TIER", "discovered")]));
        let apps = vec!["zoekt-mcp".to_string()];
        let hits = scan_env(&apps, lookup);
        assert_eq!(hits, vec![("zoekt-mcp".to_string(), "discovered".to_string())]);
    }

    #[test]
    fn render_format_app_and_tier_substitution() {
        let out = render_format("[$app:$tier]($style)", "mado", "bare");
        assert_eq!(out, "mado:bare");
    }

    #[test]
    fn bare_config_is_disabled() {
        let cfg = ShikumiConfigConfig::bare();
        assert!(!cfg.enabled);
        assert!(cfg.apps.is_empty());
        assert_eq!(cfg.command_timeout_ms, 0);
    }

    #[test]
    fn default_uses_nord_aurora_green() {
        let cfg = ShikumiConfigConfig::default();
        assert_eq!(cfg.style.as_str(), "bold #A3BE8C");
        assert!(cfg.enabled);
    }

    #[test]
    fn probe_nonexistent_binary_returns_false() {
        // Spawning a deliberately-missing binary must not panic and
        // must return false within the timeout.
        let ok = probe_config_show("shikumi-cfg-no-such-binary-zzz", "bare", 200);
        assert!(!ok);
    }

    #[test]
    fn cache_view_aggregates() {
        // Tiny smoke test that the helper aggregates correctly —
        // future test surface for cache-hit logic.
        let now = Instant::now();
        let snap = vec![
            CachedProbe {
                app: "mado".to_owned(),
                tier: "bare".to_owned(),
                captured_at: now,
            },
            CachedProbe {
                app: "kenshi".to_owned(),
                tier: "default".to_owned(),
                captured_at: now,
            },
        ];
        let view = cache_view(&snap);
        assert_eq!(view.len(), 2);
        assert!(view.contains_key(&("mado".to_owned(), "bare".to_owned())));
    }
}
