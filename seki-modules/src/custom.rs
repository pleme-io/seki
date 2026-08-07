//! `custom.<NAME>` segment family — renders one fragment per enabled
//! [`CustomEntry`], from whichever [`CustomSource`] the entry declares.
//!
//! ── ★ EXACTLY ONE SOURCE VARIANT SPAWNS ─────────────────────────────────
//! The match below is the whole point of the typed source. Two of its
//! three arms resolve in this process; only [`CustomSource::Shell`]
//! reaches `sh -c`. Before the source was typed, EVERY entry took that
//! path — including the fleet's one default entry, whose command was
//! `printf "%s" "${TEAR_PANE_ID:0:6}"` guarded by
//! `[ -n "$TEAR_PANE_ID" ]`, i.e. two shells per prompt to slice an
//! environment variable. Measured on ryn: ~20.7 ms of the prompt's
//! 28.1 ms. See `seki_core::config::custom` for the full receipt.
//!
//! When you add an arm here, the question to answer is not "does it
//! work" but "does it spawn" — a prompt runs before every command the
//! operator types, so a segment's cost is paid at the rate the operator
//! thinks, not the rate the machine works.
//!
//! ── ★ THE SPAWNING ARM RUNS UNDER A REAL DEADLINE ───────────────────────
//! This block used to read: "Output is bounded by the top-level
//! `command_timeout_ms`; we don't enforce that inside the module (the M2
//! renderer hasn't taken on subprocess supervision yet)." Nothing
//! enforced it: `SekiConfig::command_timeout_ms` is declared and read by
//! no module at all, and `custom.rs` called `.output()` / `.status()`
//! unbounded.
//!
//! `.output()` and `.status()` block forever. This segment runs
//! OPERATOR-CONFIGURED shell, so a command touching a stale NFS mount, a
//! sleeping VPN or a dead endpoint hung every prompt on the machine.
//! Both call sites go through `seki_core::exec`, which kills and reaps
//! the child at the deadline rather than abandoning it. That machinery
//! is untouched by the source typing — it simply now applies to the only
//! arm that can need it.

use seki_core::{
    Module, RenderContext, Segment, SekiResult,
    config::custom::{CustomConfig, CustomEntry, CustomSource, ShellSource},
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
            let Some(output) = resolve(&entry.source) else {
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

/// The `$output` of one source.
///
/// The env-backed arms are a pure function of the process environment
/// and are unit-tested against an injected lookup in
/// `seki_core::config::custom`; here they are handed the real one.
fn resolve(source: &CustomSource) -> Option<String> {
    match source {
        CustomSource::Shell(shell) => run_shell(shell),
        env_backed => env_backed.from_env(|var| std::env::var(var).ok()),
    }
}

/// The escape hatch: up to two children, both under the entry's budget.
///
/// One budget covers the predicate AND the value command, so a slow
/// `when` cannot spend the value command's allowance.
fn run_shell(shell: &ShellSource) -> Option<String> {
    let budget = shell.budget();
    if !satisfies_when(shell.when.as_deref(), budget) {
        return None;
    }
    seki_core::exec::bounded_stdout(Command::new("sh").arg("-c").arg(&shell.command), budget)
}

/// The `when` predicate, under the entry's budget.
///
/// A predicate that does not finish in time is FALSE. Treating an
/// unanswered check as satisfied would render a segment the operator's
/// `when` clause exists to suppress — and it would do so precisely when
/// the system is unhealthy enough to be slow.
fn satisfies_when(when: Option<&str>, budget: std::time::Duration) -> bool {
    let Some(cmd) = when else {
        return true;
    };
    seki_core::exec::bounded_success(Command::new("sh").arg("-c").arg(cmd), budget)
}

/// Helper for declarative entry construction in the parity + theme
/// configs — same shape as the inline TOML, with the source stated.
pub fn entry(description: &str, source: CustomSource, format: &str, style: &str) -> CustomEntry {
    CustomEntry {
        enabled: true,
        description: description.to_owned(),
        source,
        style: seki_core::style::StyleSpec::new(style),
        format: format.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seki_core::format::render_one;

    #[test]
    fn renders_tear_pane_format() {
        assert_eq!(
            render_one("[· $output]($style) ", "output", "abcdef"),
            "· abcdef "
        );
    }

    /// The env-backed arms must not reach `sh`. There is no way to
    /// observe "did not spawn" from the outside, so this asserts the
    /// nearest observable proxy: `resolve` answers from an environment
    /// variable this test's own process really has (`PATH` is set for
    /// every process that can run a test binary at all), through the
    /// same code path the renderer uses.
    #[test]
    fn an_env_backed_source_resolves_without_a_shell() {
        let got = resolve(&CustomSource::EnvSlice {
            var: "PATH".to_owned(),
            chars: 1,
        });
        assert_eq!(
            got.map(|s| s.chars().count()),
            Some(1),
            "EnvSlice must render one character of $PATH",
        );
    }

    /// The negative control for the test above: an unset variable is
    /// silence, not a fabricated empty render.
    #[test]
    fn an_unset_variable_renders_nothing() {
        assert_eq!(
            resolve(&CustomSource::EnvVar {
                var: "SEKI_DEFINITELY_NOT_A_REAL_VARIABLE".to_owned(),
            }),
            None,
        );
    }

    /// The escape hatch still works, and still under a deadline.
    #[test]
    fn a_shell_source_still_runs_its_command() {
        let got = resolve(&CustomSource::Shell(ShellSource {
            command: "printf hi".to_owned(),
            when: None,
            ignore_timeout: false,
            command_timeout_ms: 5_000,
        }));
        assert_eq!(got.as_deref(), Some("hi"));
    }

    /// A false `when` suppresses the segment — the behaviour the
    /// retired `tear_pane` guard relied on, kept intact for operators
    /// who genuinely need a predicate.
    #[test]
    fn a_failing_when_suppresses_the_shell_source() {
        let got = resolve(&CustomSource::Shell(ShellSource {
            command: "printf hi".to_owned(),
            when: Some("exit 1".to_owned()),
            ignore_timeout: false,
            command_timeout_ms: 5_000,
        }));
        assert_eq!(got, None);
    }
}
