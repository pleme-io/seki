//! `custom.<NAME>` segment family — runs an opaque command and
//! renders its stdout.
//!
//! NO SHELL beyond the operator-authored command string itself.
//! Per the brief's hard rule, seki does NOT build a shell expression
//! around the command — we invoke it directly via `Command::new`
//! with the configured command as `sh -c "<cmd>"` only as a final
//! step. The `when` predicate is treated the same way.
//!
//! ── ★ BOTH COMMANDS RUN UNDER A REAL DEADLINE ──────────────────────────
//! This block used to read: "Output is bounded by the top-level
//! `command_timeout_ms`; we don't enforce that inside the module (the M2
//! renderer hasn't taken on subprocess supervision yet)." Every clause of
//! that was true except the first — there is no top-level
//! `command_timeout_ms` anywhere in seki's config tree, so the bound was
//! credited to an authority that does not exist and nothing enforced it.
//!
//! `.output()` and `.status()` block forever. This segment runs
//! OPERATOR-CONFIGURED shell, so a command touching a stale NFS mount, a
//! sleeping VPN or a dead endpoint hung every prompt on the machine.
//! Both call sites now go through `seki_core::exec`, which kills and reaps
//! the child at the deadline rather than abandoning it.

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
            // One budget covers the predicate AND the value command, so a
            // slow `when` cannot spend the value command's allowance.
            let budget = entry.budget();
            if !satisfies_when(&entry.when, budget) {
                continue;
            }
            let Some(output) = run_command(&entry.command, budget) else {
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

/// The `when` predicate, under the entry's budget.
///
/// A predicate that does not finish in time is FALSE. Treating an
/// unanswered check as satisfied would render a segment the operator's
/// `when` clause exists to suppress — and it would do so precisely when
/// the system is unhealthy enough to be slow.
fn satisfies_when(when: &Option<String>, budget: std::time::Duration) -> bool {
    let Some(cmd) = when else {
        return true;
    };
    seki_core::exec::bounded_success(Command::new("sh").arg("-c").arg(cmd), budget)
}

/// The value command, under the entry's budget.
fn run_command(cmd: &str, budget: std::time::Duration) -> Option<String> {
    seki_core::exec::bounded_stdout(Command::new("sh").arg("-c").arg(cmd), budget)
}

/// Helper for declarative entry construction in the blzsh-parity
/// config — same shape as the inline TOML.
pub fn entry(
    description: &str,
    command: &str,
    when: Option<&str>,
    format: &str,
    style: &str,
) -> CustomEntry {
    CustomEntry {
        enabled: true,
        description: description.to_owned(),
        command: command.to_owned(),
        when: when.map(str::to_owned),
        style: seki_core::style::StyleSpec::new(style),
        format: format.to_owned(),
        ignore_timeout: false,
        command_timeout_ms: CustomEntry::default().command_timeout_ms,
    }
}

#[cfg(test)]
mod tests {
    use seki_core::format::render_one;

    #[test]
    fn renders_tear_pane_format() {
        assert_eq!(
            render_one("[· $output]($style) ", "output", "abcdef"),
            "· abcdef "
        );
    }
}
