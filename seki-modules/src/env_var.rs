//! `env_var.<NAME>` segment family — renders one fragment per
//! enabled [`env_var::EnvVarEntry`] in the config. Reads each
//! variable directly from the process env; missing values fall
//! back to the entry's `default`.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::env_var::{EnvVarConfig, EnvVarEntry},
    segment::StyledFragment,
};

pub struct EnvVarModule {
    cfg: EnvVarConfig,
}

impl EnvVarModule {
    pub fn new(cfg: EnvVarConfig) -> Self {
        Self { cfg }
    }
}

impl Module for EnvVarModule {
    fn name(&self) -> &'static str {
        "env_var"
    }

    fn enabled(&self) -> bool {
        self.cfg.entries.values().any(|e| e.enabled)
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let mut segment = Segment::new("env_var");
        for (key, entry) in &self.cfg.entries {
            if !entry.enabled {
                continue;
            }
            let var = entry.variable.as_deref().unwrap_or(key);
            let value = std::env::var(var).ok().unwrap_or_else(|| entry.default.clone());
            if value.is_empty() {
                continue;
            }
            let text = seki_core::format::render_one(&entry.format, "env_value", &value);
            if text.is_empty() {
                continue;
            }
            segment = segment.push(StyledFragment::new(text, entry.style.resolve()));
        }
        if segment.is_empty() {
            Ok(None)
        } else {
            Ok(Some(segment))
        }
    }
}

/// Helper: construct an `EnvVarEntry` for an inline declaration in
/// the blzsh-parity config. Reduces noise at the call site.
pub fn entry(variable: &str, format: &str, style: &str) -> EnvVarEntry {
    EnvVarEntry {
        enabled: true,
        variable: Some(variable.to_owned()),
        default: String::new(),
        style: seki_core::style::StyleSpec::new(style),
        format: format.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use seki_core::format::render_one;

    #[test]
    fn renders_workspace_bracket_format() {
        let out = render_one("[\\[$env_value\\]]($style) ", "env_value", "pleme-io");
        assert_eq!(out, "[pleme-io] ");
    }

    #[test]
    fn renders_tear_session_format() {
        let out = render_one("[~ $env_value]($style) ", "env_value", "main");
        assert_eq!(out, "~ main ");
    }
}
