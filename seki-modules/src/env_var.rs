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
            let text = render_format(&entry.format, &value);
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

/// Render `[\[$env_value\]]($style) `-style format strings,
/// stripping starship markup and substituting `$env_value`.
pub fn render_format(fmt: &str, value: &str) -> String {
    let mut out = String::with_capacity(fmt.len() + value.len());
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&n) = chars.peek() {
                out.push(n);
                chars.next();
            }
        } else if c == '$' {
            let mut name = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || n == '_' {
                    name.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            if name == "env_value" {
                out.push_str(value);
            }
        } else if c == '[' || c == ']' {
            continue;
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
        } else {
            out.push(c);
        }
    }
    out
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

    #[test]
    fn renders_workspace_bracket_format() {
        let out = render_format("[\\[$env_value\\]]($style) ", "pleme-io");
        assert_eq!(out, "[pleme-io] ");
    }

    #[test]
    fn renders_tear_session_format() {
        let out = render_format("[~ $env_value]($style) ", "main");
        assert_eq!(out, "~ main ");
    }
}
