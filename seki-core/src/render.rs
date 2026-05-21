//! Prompt rendering driver.
//!
//! Walks the typed `prompt_order` from a [`SekiConfig`], invokes each
//! [`Module`] via the [`ModuleRegistry`], filters out empty segments,
//! and concatenates the result. Each fragment is passed through
//! [`crate::style::apply`] which respects `ctx.enable_colors`.

use crate::{
    RenderContext, SekiConfig, SekiResult,
    module::ModuleRegistry,
    segment::Segment,
    style::apply,
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

    // Trailing prompt symbol from cfg.character.format — typed,
    // not stringly built. We embed a single `$ ` (or `# ` for
    // root). Anyone wanting a starshipy `❯` overrides via the
    // typed config in seki-modules.
    let symbol_style = cfg.character.style.resolve();
    let symbol_text = if ctx.last_exit_code == 0 {
        cfg.character.success_symbol.clone()
    } else {
        cfg.character.error_symbol.clone()
    };
    if !symbol_text.is_empty() {
        raw.push_str(&apply(&symbol_text, &symbol_style, ctx.enable_colors));
    }

    Ok(RenderedPrompt { segments, raw })
}
