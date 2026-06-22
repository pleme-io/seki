//! `custom.<NAME>` segment family — runs an opaque command and
//! renders its stdout.
//!
//! NO SHELL beyond the operator-authored command string itself.
//! Per the brief's hard rule, seki does NOT build a shell expression
//! around the command — we invoke it directly via `Command::new`
//! with the configured command as `sh -c "<cmd>"` only as a final
//! step. The `when` predicate is treated the same way.
//!
//! Output is bounded by the top-level `command_timeout_ms`; we
//! don't enforce that inside the module (the M2 renderer hasn't
//! taken on subprocess supervision yet).

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::custom::{CustomConfig, CustomEntry},
    segment::StyledFragment,
};
use std::process::Command;

pub struct CustomModule {
    cfg: CustomConfig,
}

impl CustomModule {
    pub fn new(cfg: CustomConfig) -> Self {
        Self { cfg }
    }
}

impl Module for CustomModule {
    fn name(&self) -> &'static str {
        "custom"
    }

    fn enabled(&self) -> bool {
        self.cfg.entries.values().any(|e| e.enabled)
    }

    fn render(&self, _ctx: &RenderContext) -> SekiResult<Option<Segment>> {
        let mut segment = Segment::new("custom");
        for (_name, entry) in &self.cfg.entries {
            if !entry.enabled {
                continue;
            }
            if !satisfies_when(&entry.when) {
                continue;
            }
            let Some(output) = run_command(&entry.command) else {
                continue;
            };
            if output.is_empty() {
                continue;
            }
            let text = seki_core::format::render_one(&entry.format, "output", &output);
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

fn satisfies_when(when: &Option<String>) -> bool {
    let Some(cmd) = when else {
        return true;
    };
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_command(cmd: &str) -> Option<String> {
    let output = Command::new("sh").arg("-c").arg(cmd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Some(s)
}

/// Helper for declarative entry construction in the blzsh-parity
/// config — same shape as the inline TOML.
pub fn entry(description: &str, command: &str, when: Option<&str>, format: &str, style: &str) -> CustomEntry {
    CustomEntry {
        enabled: true,
        description: description.to_owned(),
        command: command.to_owned(),
        when: when.map(str::to_owned),
        style: seki_core::style::StyleSpec::new(style),
        format: format.to_owned(),
        ignore_timeout: false,
    }
}

#[cfg(test)]
mod tests {
    use seki_core::format::render_one;

    #[test]
    fn renders_tear_pane_format() {
        assert_eq!(render_one("[· $output]($style) ", "output", "abcdef"), "· abcdef ");
    }
}
