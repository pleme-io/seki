//! `ishou_theme` segment — surfaces the fleet-prescribed FleetTheme
//! variant in the prompt.
//!
//! Pleme-io-native (Tier 4 — substrate-themed). Pulls
//! `ishou_tokens::FleetDefaults::prescribed().theme` at compile time
//! via [`seki_core::config::ishou_theme::prescribed_theme_name`] and
//! renders the variant name (`Bare` / `PlemeDark`) inside a typed
//! segment.
//!
//! ## Theme
//!
//! Nord-snowstorm dim `#D8DEE9` by default — the "fact about the
//! design system" accent. Distinguished from aurora-yellow override
//! signals (`shikumi_tier`) and frost-blue typed-repo identity
//! signals (`caixa`).
//!
//! ## Probe budget
//!
//! Compile-time constant — bypasses every scan budget. The render
//! call is pure (no I/O, no env reads, no syscalls).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::ishou_theme::{IshouThemeConfig, prescribed_theme_name},
    segment::StyledFragment,
};

pub struct IshouThemeModule {
    cfg: IshouThemeConfig,
}

impl IshouThemeModule {
    pub fn new(cfg: IshouThemeConfig) -> Self {
        Self { cfg }
    }
}

impl Module for IshouThemeModule {
    fn name(&self) -> &'static str {
        "ishou_theme"
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let theme = prescribed_theme_name();
        let text = seki_core::format::render_one(&self.cfg.format, "theme", theme);
        Ok(Some(Segment::new("ishou_theme").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_format_theme_substitution() {
        let out = seki_core::format::render_one("[ishou: $theme]($style)", "theme", "PlemeDark");
        assert_eq!(out, "ishou: PlemeDark");
    }

    #[test]
    fn render_format_plain_template() {
        let out = seki_core::format::render_one("$theme", "theme", "Bare");
        assert_eq!(out, "Bare");
    }

    #[test]
    fn renders_segment_when_enabled() {
        let module = IshouThemeModule::new(IshouThemeConfig::default());
        let ctx = RenderContext::from_env().with_colors(false);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "ishou_theme");
        // The prescribed FleetDefaults variant is Vellum —
        // proves the ishou-tokens dep wires through end-to-end at the
        // module-render layer.
        assert_eq!(seg.fragments[0].text, "ishou: Vellum");
    }

    #[test]
    fn bare_module_reports_disabled() {
        let module = IshouThemeModule::new(IshouThemeConfig::bare());
        assert!(!module.enabled());
    }

    #[test]
    fn default_module_reports_enabled() {
        // Tier 4 default: enabled = true (cheap, no probe).
        let module = IshouThemeModule::new(IshouThemeConfig::default());
        assert!(module.enabled());
    }
}
