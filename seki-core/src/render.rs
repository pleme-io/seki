//! Prompt rendering driver.
//!
//! Walks the typed `prompt_order` from a [`SekiConfig`], invokes each
//! [`Module`] via the [`ModuleRegistry`], filters out empty segments,
//! and concatenates the result. Each fragment is passed through
//! [`crate::style::apply`] which respects `ctx.enable_colors`.
//!
//! The trailing prompt character honours the M2 [`CharacterConfig`]
//! shape — `success_symbol` / `error_symbol` plus the vim-mode
//! variants. Embedded starship markup like `"[❄](bold #88C0D0)"` is
//! parsed in-place so a single character config field holds both
//! glyph and style.

use crate::{
    RenderContext, SekiConfig, SekiResult,
    config::character::CharacterConfig,
    module::ModuleRegistry,
    segment::Segment,
    style::{StyleSpec, apply},
};

pub struct RenderedPrompt {
    pub segments: Vec<Segment>,
    pub raw: String,
}

/// Render the configured prompt into a single ANSI-escaped string.
///
/// Modules whose name doesn't resolve in the registry are silently
/// skipped (matches starship's tolerance of stale `prompt_order`
/// entries). The skipped-name list is returned through
/// [`RenderedPrompt::segments`] for debugging — empty segments
/// indicate a skipped module.
pub fn render_prompt(
    cfg: &SekiConfig,
    registry: &ModuleRegistry,
    ctx: &RenderContext,
) -> SekiResult<RenderedPrompt> {
    let mut segments = Vec::with_capacity(cfg.prompt_order.len());
    let mut raw = String::new();

    for name in &cfg.prompt_order {
        if name == "character" {
            // character is rendered at the end of the prompt — skip
            // here, emit after the loop.
            continue;
        }
        let Some(module) = registry.get(name) else {
            continue;
        };
        if !module.enabled() {
            continue;
        }
        match module.render(ctx)? {
            Some(segment) if !segment.is_empty() => {
                for fragment in &segment.fragments {
                    raw.push_str(&apply(&fragment.text, &fragment.style, ctx.enable_colors));
                }
                segments.push(segment);
            }
            _ => {}
        }
    }

    if cfg.character.enabled {
        let (text, style) = render_character(&cfg.character, ctx.last_exit_code);
        if !text.is_empty() {
            raw.push_str(&apply(&text, &style, ctx.enable_colors));
            // Trailing space per starship's `"$symbol "` default.
            raw.push(' ');
        }
    }

    Ok(RenderedPrompt { segments, raw })
}

/// Resolve the character segment to a (text, style) pair, parsing
/// starship-style embedded-style markup like `"[❄](bold #88C0D0)"`.
pub fn render_character(cfg: &CharacterConfig, exit_code: i32) -> (String, crate::style::Style) {
    let raw_symbol = if exit_code == 0 {
        &cfg.success_symbol
    } else {
        &cfg.error_symbol
    };
    let (text, embedded_style) = parse_embedded_style(raw_symbol);
    let style = embedded_style.unwrap_or_else(|| cfg.style.resolve());
    (text, style)
}

/// Parse starship's `"[text](style)"` grammar into (text, optional
/// style). Returns the raw input as text + `None` when there's no
/// markup.
pub fn parse_embedded_style(raw: &str) -> (String, Option<crate::style::Style>) {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') {
        return (raw.to_owned(), None);
    }
    let Some(close_bracket) = trimmed.find("](") else {
        return (raw.to_owned(), None);
    };
    let text = &trimmed[1..close_bracket];
    let after = &trimmed[close_bracket + 2..];
    let Some(close_paren) = after.find(')') else {
        return (raw.to_owned(), None);
    };
    let style_str = &after[..close_paren];
    let style = StyleSpec::new(style_str).resolve();
    (text.to_owned(), Some(style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Color;

    #[test]
    fn parses_starship_embedded_style() {
        let (text, style) = parse_embedded_style("[❄](bold #88C0D0)");
        assert_eq!(text, "❄");
        let style = style.unwrap();
        assert!(style.bold);
        assert_eq!(style.fg, Some(Color::Rgb(0x88, 0xC0, 0xD0)));
    }

    #[test]
    fn plain_symbol_has_no_embedded_style() {
        let (text, style) = parse_embedded_style("❯");
        assert_eq!(text, "❯");
        assert!(style.is_none());
    }
}
