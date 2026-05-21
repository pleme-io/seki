//! [`Module`] — the trait every prompt segment implements.
//!
//! A module is a pure function from ([`RenderContext`], typed config)
//! to an optional [`Segment`]. Modules are registered by name in a
//! [`ModuleRegistry`]; the [`crate::render::render_prompt`] driver
//! walks the configured prompt order and calls each module in turn.
//!
//! seki-modules ships a [`Module`] impl per segment; seki-core
//! provides the bare trait + the registry. Per the prime directive,
//! per-module typed configuration travels via the
//! [`Module::Config`] associated type so each impl owns its config
//! shape without back-references into [`crate::SekiConfig`].

use crate::{RenderContext, Segment, SekiResult};
use std::collections::HashMap;
use std::sync::Arc;

pub trait Module: Send + Sync {
    /// Stable canonical name — `directory`, `git_branch`, … . Used
    /// to look the module up in the registry + as the value of
    /// `prompt_order` entries.
    fn name(&self) -> &'static str;

    /// Render this module against the current context. Returning
    /// `None` (or a [`Segment`] with zero fragments) means "I have
    /// nothing to show right now" — the renderer filters such
    /// segments out, matching starship's per-module `disabled`
    /// semantics.
    fn render(&self, ctx: &RenderContext) -> SekiResult<Option<Segment>>;

    /// Whether this module is enabled in the current config.
    /// Disabled modules are skipped entirely (no render call).
    fn enabled(&self) -> bool {
        true
    }
}

/// Registry of typed modules, keyed by [`Module::name`]. The
/// canonical registry lives in seki-modules
/// (`seki_modules::default_registry`); seki-core owns the bare
/// type so that seki-shikumi can construct registries from a
/// [`crate::SekiConfig`] without depending on seki-modules.
#[derive(Default, Clone)]
pub struct ModuleRegistry {
    modules: HashMap<String, Arc<dyn Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<M: Module + 'static>(&mut self, module: M) {
        self.modules
            .insert(module.name().to_owned(), Arc::new(module));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Module>> {
        self.modules.get(name).cloned()
    }

    pub fn names(&self) -> Vec<&str> {
        self.modules.keys().map(String::as_str).collect()
    }
}
