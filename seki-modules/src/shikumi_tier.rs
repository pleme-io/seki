//! `shikumi_tier` segment — surfaces active `<APP>_TIER` env vars.
//!
//! Pleme-io-native. Scans the shikumi catalogue (mado, tatara,
//! kenshi, …) and renders any `<APP>_TIER` that's set in the
//! current process environment. Tells the operator at a glance
//! which tiers are active in the current shell session.
//!
//! ## Theme
//!
//! Nord-aurora yellow `#EBCB8B` by default — tiers are an "alert
//! that override is in effect" signal, not a primary status.
//!
//! ## Probe budget
//!
//! Env-var read only; bypasses `scan_timeout_ms`.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::shikumi_tier::ShikumiTierConfig,
    segment::StyledFragment,
};

pub struct ShikumiTierModule {
    cfg: ShikumiTierConfig,
}

impl ShikumiTierModule {
    pub fn new(cfg: ShikumiTierConfig) -> Self {
        Self { cfg }
    }
}

impl Module for ShikumiTierModule {
    fn name(&self) -> &'static str {
        "shikumi_tier"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let hits = scan_env(&self.cfg.apps, env_lookup);
        if hits.is_empty() {
            return Ok(None);
        }
        let rendered: Vec<String> = hits
            .into_iter()
            .map(|(app, tier)| {
                seki_core::format::render(&self.cfg.format, |__n| match __n {
                    "app" => Some(app.to_owned()),
                    "tier" => Some(tier.to_owned()),
                    _ => None,
                })
            })
            .collect();
        let text = rendered.join(&self.cfg.separator);
        Ok(Some(Segment::new("shikumi_tier").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Pure-function scan: for each app in `apps`, look up its tier
/// via `lookup`. Returns the (app, tier) pairs that were set.
///
/// Pulled out for testability — call sites pass `env_lookup` in
/// production, tests pass a stub lookup.
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
    fn scan_env_skips_unset() {
        let lookup = stub_lookup(HashMap::new());
        let apps = vec!["mado".to_string()];
        assert!(scan_env(&apps, lookup).is_empty());
    }

    #[test]
    fn scan_env_translates_hyphen_to_underscore() {
        // zoekt-mcp env var is ZOEKT_MCP_TIER (hyphens become _).
        let lookup = stub_lookup(HashMap::from([("ZOEKT_MCP_TIER", "discovered")]));
        let apps = vec!["zoekt-mcp".to_string()];
        let hits = scan_env(&apps, lookup);
        assert_eq!(hits, vec![("zoekt-mcp".to_string(), "discovered".to_string())]);
    }

    #[test]
    fn render_format_app_and_tier_substitution() {
        let out = seki_core::format::render_vars("[$app:$tier]($style)", &[("app", "mado"), ("tier", "bare")]);
        assert_eq!(out, "mado:bare");
    }

    #[test]
    fn render_format_handles_plain_template() {
        let out = seki_core::format::render_vars("$app=$tier", &[("app", "tatara"), ("tier", "default")]);
        assert_eq!(out, "tatara=default");
    }
}
