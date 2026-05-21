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
        let text = render_format(&self.cfg.format, theme);
        Ok(Some(Segment::new("ishou_theme").push(StyledFragment::new(
            text,
            self.cfg.style.resolve(),
        ))))
    }
}

/// Render the format string. Substitutions: `$theme`. Starship-style
/// `[…]($style)` markup stripped (renderer applies `style` directly).
/// Mirrors `shikumi_tier::render_format`.
pub fn render_format(fmt: &str, theme: &str) -> String {
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
                "theme" => out.push_str(theme),
                _ => {} // drop $style and unknowns
            }
        } else if c == '[' || c == ']' {
            // strip starship markup
        } else if c == '(' {
            // skip parenthesised style spec
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
    fn render_format_theme_substitution() {
        let out = render_format("[ishou: $theme]($style)", "PlemeDark");
        assert_eq!(out, "ishou: PlemeDark");
    }

    #[test]
    fn render_format_plain_template() {
        let out = render_format("$theme", "Bare");
        assert_eq!(out, "Bare");
    }

    #[test]
    fn renders_segment_when_enabled() {
        let module = IshouThemeModule::new(IshouThemeConfig::default());
        let ctx = RenderContext::from_env().with_colors(false);
        let seg = module.render(&ctx).unwrap().expect("segment");
        assert_eq!(seg.module, "ishou_theme");
        // The prescribed FleetDefaults variant is PlemeDark — proves
        // the ishou-tokens dep wires through end-to-end at the
        // module-render layer.
        assert_eq!(seg.fragments[0].text, "ishou: PlemeDark");
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
