//! Typed config for the `custom.<NAME>` segment family.
//!
//! starship lets the operator declare arbitrary `[custom.FOO]` sections
//! that run a shell command and render its stdout. seki declares where
//! `$output` comes from with a typed [`CustomSource`] instead — and only
//! one of its variants costs a process.
//!
//! ── ★ THE SPAWN IS DECLARED, NOT IMPLIED ────────────────────────────────
//! This surface used to be an untyped `command: String` plus an optional
//! `when: String`, both handed to `sh -c`. Every entry therefore paid for
//! a shell — two of them, when a `when` guard was present — whatever it
//! was actually doing.
//!
//! The fleet's ONE default entry was doing a substring of an environment
//! variable: `command = 'printf "%s" "${TEAR_PANE_ID:0:6}"'` guarded by
//! `when = '[ -n "$TEAR_PANE_ID" ]'`. Measured on ryn (2026-08-07,
//! `hyperfine -N --warmup 3 -m 15`): `seki module custom` 24.7 ms against
//! a 4.0 ms seki process floor, i.e. ~20.7 ms of pure `sh` startup —
//! macOS `/bin/sh` is bash 3.2 and `/bin/sh -c ':'` alone measures
//! 10.6 ms, twice. That was ~74% of the entire prompt.
//!
//! Two things follow, and the type encodes both:
//!
//! 1. **Reading an env var is not a reason to start a shell.**
//!    [`CustomSource::EnvSlice`] and [`CustomSource::EnvVar`] resolve in
//!    the renderer's own process.
//! 2. **A spawn should be visible in the declaration.** It is now spelled
//!    [`CustomSource::Shell`] — the escape hatch an operator reaches for
//!    deliberately, never the shape they land in by default.
//!
//! ── ★ AND THE SHELL FORM WAS NOT PORTABLE ───────────────────────────────
//! `${VAR:0:6}` is a bash/zsh substring expansion with no POSIX
//! equivalent. It worked only because macOS `/bin/sh` happens to be
//! bash 3.2; under dash — `/bin/sh` on Debian/Ubuntu and most NixOS
//! containers — it is a hard `Bad substitution` on stderr with a
//! non-zero exit, so the segment renders nothing and complains while
//! doing it. [`CustomSource::EnvSlice`] has no shell to be incompatible
//! with.
//!
//! ── ★ THE BUDGET LIVES WITH THE THING THAT NEEDS BOUNDING ───────────────
//! `ignore_timeout` + `command_timeout_ms` are fields of [`ShellSource`],
//! not of [`CustomEntry`]. A process-free source does not carry an unused
//! timeout — it carries *no* timeout, because there is no child to bound
//! and therefore nothing for the field to mean. The budget machinery
//! itself is unchanged; see [`ShellSource::budget`].

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// Where a `custom.<NAME>` segment's `$output` comes from.
///
/// Internally tagged on `kind`, so what `seki config-show` prints — and
/// what a `custom`-tier overlay would be written as — is an ordinary
/// mapping:
///
/// ```yaml
/// source:
///   kind: env_slice
///   var: TEAR_PANE_ID
///   chars: 6
/// ```
///
/// Deliberately NOT serde's default external tagging, which serializes
/// to a YAML `!env_slice` type tag — correct, round-trippable, and
/// alien to anyone reading it. [`crate::style::Color`] is externally
/// tagged because its variants are tuples and have no choice; this one
/// does.
///
/// Tier-honest: `seki config-show custom --path <file>` currently
/// ignores the file entirely (measured 2026-08-07 — even a nonexistent
/// path prints the default tier and exits 0), so no operator YAML
/// reaches a running prompt yet. That is a gap in the shikumi CLI
/// wiring, not a reason to pick the less readable encoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CustomSource {
    /// A fixed-width prefix of an environment variable, sliced by
    /// CHARACTERS. Costs no process.
    ///
    /// This is the shape the fleet's `tear_pane` tag actually needed:
    /// a machine-assigned pane id truncated to something that fits a
    /// prompt.
    EnvSlice {
        var: String,
        /// How many CHARACTERS of `var` to render. See [`take_chars`]
        /// for why the distinction from bytes is load-bearing.
        chars: usize,
    },
    /// A whole environment variable's value, rendered when non-empty.
    /// Costs no process.
    EnvVar { var: String },
    /// Run a command and render its stdout.
    ///
    /// The escape hatch — the ONLY variant that can cost a spawn, so the
    /// cost is visible in the declaration rather than implied by the
    /// segment family. Everything an unbounded child needs (the budget,
    /// the `when` predicate) lives inside [`ShellSource`], where it
    /// applies.
    Shell(ShellSource),
}

impl CustomSource {
    /// Does rendering this source cost a process?
    ///
    /// The forcing function for "the fleet default prompt spawns
    /// nothing" — a test can assert it over a whole config rather than
    /// re-reading each entry's command string.
    #[must_use]
    pub fn spawns(&self) -> bool {
        matches!(self, Self::Shell(_))
    }

    /// The value of a source that reads the environment and nothing else.
    ///
    /// The lookup is injected rather than read from the process, so the
    /// whole matrix is unit-testable without mutating the test binary's
    /// environment (which edition 2024 makes `unsafe`, and which is
    /// racy across parallel tests regardless).
    ///
    /// [`CustomSource::Shell`] answers `None` here: its value is a child
    /// process's stdout, and this function does not spawn. The renderer
    /// matches on the variant and takes the bounded path there.
    ///
    /// Note it does NOT trim. `env_var`'s segment doesn't either, and an
    /// environment value silently losing its edges in one segment but
    /// not the other is exactly the kind of invisible transform that
    /// makes two prompt segments disagree about the same variable.
    #[must_use]
    pub fn from_env(&self, lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
        match self {
            Self::EnvSlice { var, chars } => Some(take_chars(&lookup(var)?, *chars)),
            Self::EnvVar { var } => lookup(var),
            Self::Shell(_) => None,
        }
    }
}

/// The first `chars` CHARACTERS of `value` — never the first `chars`
/// BYTES.
///
/// ── ★ WHY THIS IS NOT `&value[..chars]` ─────────────────────────────────
/// Byte slicing panics when the index lands inside a multi-byte UTF-8
/// sequence, and the input here is an operator-controlled environment
/// value. A prompt renderer that panics takes the shell's prompt with
/// it, so a non-ASCII variable must not be able to reach a byte
/// boundary at all.
///
/// It is also what makes the replacement faithful: bash's `${VAR:0:n}`
/// is itself character-based in a UTF-8 locale. Verified on ryn against
/// the pre-change binary — `TEAR_PANE_ID=日本語テスト` renders all six
/// characters under the retired `sh -c 'printf "%s" "${VAR:0:6}"'` form,
/// which is what this produces too.
#[must_use]
pub fn take_chars(value: &str, chars: usize) -> String {
    value.chars().take(chars).collect()
}

/// A command run to produce `$output`, and the deadline it runs under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellSource {
    /// Command run to produce `$output`. Opaque to seki.
    pub command: String,
    /// Optional predicate command — segment is silent when this exits
    /// non-zero. Mirrors starship's `when`. Costs a SECOND spawn when
    /// present.
    pub when: Option<String>,
    /// ── ★ RELAX THE BUDGET; NEVER REMOVE IT ────────────────────────────
    /// Was documented as "skip command-timeout enforcement", and there was
    /// no enforcement to skip — `custom.rs` ran `.output()` and `.status()`
    /// unbounded while its module docs credited a "top-level
    /// `command_timeout_ms`" for the bound. (That top-level field does
    /// exist, at [`crate::SekiConfig::command_timeout_ms`] — the earlier
    /// correction overstated it as absent. It is worse than absent: it is
    /// declared and read by nothing.) A configured command touching a
    /// stale mount or a down endpoint hung EVERY shell prompt.
    ///
    /// It now selects [`ShellSource::IGNORE_TIMEOUT_MS`] instead of
    /// [`ShellSource::command_timeout_ms`]. Deliberately not "unbounded":
    /// a prompt has no honest use for a command that may never return, so
    /// no configuration is allowed to express one. The generous ceiling is
    /// the escape hatch; infinity is not on the menu.
    pub ignore_timeout: bool,
    /// Wall-clock budget for both `command` and the `when` predicate.
    /// Per-entry, matching the convention every other probing segment
    /// already follows (`arnes_cache`, `kasou_vm`, `tend`, …).
    pub command_timeout_ms: u64,
}

impl ShellSource {
    /// Budget applied when `ignore_timeout` is set. Two seconds is long
    /// enough that an operator who asked for slack gets it, and short
    /// enough that a wedged command is a stutter rather than a dead shell.
    pub const IGNORE_TIMEOUT_MS: u64 = 2_000;

    /// The budget this command actually runs under.
    ///
    /// Total, and reachable only from the variant that spawns — there is
    /// no `Option` here and no budget on a source that has no child.
    #[must_use]
    pub fn budget(&self) -> Duration {
        Duration::from_millis(if self.ignore_timeout {
            Self::IGNORE_TIMEOUT_MS
        } else {
            self.command_timeout_ms
        })
    }
}

impl Default for ShellSource {
    fn default() -> Self {
        Self {
            command: String::new(),
            when: None,
            ignore_timeout: false,
            command_timeout_ms: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomEntry {
    pub enabled: bool,
    pub description: String,
    /// Where `$output` comes from — and, for [`CustomSource::Shell`],
    /// the whole timeout budget.
    pub source: CustomSource,
    pub style: StyleSpec,
    /// Format string. Substitution: `$output`.
    pub format: String,
}

impl Default for CustomEntry {
    fn default() -> Self {
        Self {
            enabled: true,
            description: String::new(),
            // A default entry is a placeholder either way (there is no
            // useful default command and no useful default variable), so
            // the default is the one that costs nothing to render.
            source: CustomSource::EnvVar { var: String::new() },
            style: StyleSpec::new("green bold"),
            format: "[$symbol($output )]($style)".to_owned(),
        }
    }
}

impl CustomEntry {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            description: String::new(),
            source: CustomSource::EnvVar { var: String::new() },
            style: StyleSpec::default(),
            format: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomConfig {
    pub entries: BTreeMap<String, CustomEntry>,
}

impl CustomConfig {
    pub fn bare() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in process environment holding exactly one variable —
    /// injected rather than set, so no test mutates the shared process
    /// env (`unsafe` in edition 2024, and racy under the default
    /// parallel test harness regardless).
    fn env_holding(value: &str) -> impl Fn(&str) -> Option<String> {
        let value = value.to_owned();
        move |key: &str| (key == "PANE").then(|| value.clone())
    }

    /// An environment in which nothing is set.
    fn empty_env(_key: &str) -> Option<String> {
        None
    }

    /// The defect a byte slice would have: `&"日本語テスト"[..6]` is a
    /// panic (index 6 is mid-character), and a panicking prompt renderer
    /// takes the shell's prompt down with it.
    #[test]
    fn env_slice_counts_characters_not_bytes() {
        let src = CustomSource::EnvSlice {
            var: "PANE".to_owned(),
            chars: 6,
        };
        // 18 bytes, 6 characters — a byte slice at 6 lands mid-sequence.
        assert_eq!("日本語テスト".len(), 18);
        assert_eq!(
            src.from_env(env_holding("日本語テストの余分")),
            Some("日本語テスト".to_owned()),
        );
    }

    /// Bare-ASCII control for the test above: without it, a `take_chars`
    /// that returned the whole string would pass the multi-byte case.
    #[test]
    fn env_slice_truncates_an_ascii_value() {
        let src = CustomSource::EnvSlice {
            var: "PANE".to_owned(),
            chars: 6,
        };
        assert_eq!(
            src.from_env(env_holding("42032e03aa3e69d8")),
            Some("42032e".to_owned()),
        );
    }

    #[test]
    fn env_slice_shorter_than_the_width_renders_whole() {
        assert_eq!(take_chars("ab", 6), "ab");
        assert_eq!(take_chars("", 6), "");
        assert_eq!(take_chars("abc", 0), "");
    }

    #[test]
    fn env_var_renders_the_whole_value() {
        let src = CustomSource::EnvVar {
            var: "PANE".to_owned(),
        };
        assert_eq!(
            src.from_env(env_holding("42032e03aa3e69d8")),
            Some("42032e03aa3e69d8".to_owned()),
        );
    }

    /// An absent variable is silence, and so is an empty one — the
    /// renderer drops an empty `$output`. This is what replaces the
    /// retired `when = '[ -n "$TEAR_PANE_ID" ]'` guard, at zero spawns
    /// instead of one.
    #[test]
    fn an_absent_or_empty_variable_yields_nothing_to_render() {
        for src in [
            CustomSource::EnvVar {
                var: "PANE".to_owned(),
            },
            CustomSource::EnvSlice {
                var: "PANE".to_owned(),
                chars: 6,
            },
        ] {
            assert_eq!(src.from_env(empty_env), None, "absent: {src:?}");
            assert_eq!(
                src.from_env(env_holding("")).unwrap_or_default(),
                "",
                "empty: {src:?}",
            );
        }
    }

    /// `from_env` never spawns, so the one variant whose value IS a
    /// child process has no answer to give here.
    #[test]
    fn a_shell_source_has_no_env_value() {
        let src = CustomSource::Shell(ShellSource {
            command: "printf hi".to_owned(),
            ..ShellSource::default()
        });
        assert_eq!(src.from_env(env_holding("x")), None);
    }

    #[test]
    fn only_the_shell_variant_spawns() {
        assert!(CustomSource::Shell(ShellSource::default()).spawns());
        assert!(
            !CustomSource::EnvVar {
                var: "X".to_owned()
            }
            .spawns()
        );
        assert!(
            !CustomSource::EnvSlice {
                var: "X".to_owned(),
                chars: 6
            }
            .spawns()
        );
    }

    /// The default entry must not be the spawning one — the shell is the
    /// escape hatch, including for whoever reaches for `..Default()`.
    #[test]
    fn the_default_entry_costs_no_process() {
        assert!(!CustomEntry::default().source.spawns());
        assert!(!CustomEntry::bare().source.spawns());
    }

    #[test]
    fn ignore_timeout_relaxes_the_budget_but_never_removes_it() {
        let strict = ShellSource {
            command_timeout_ms: 500,
            ignore_timeout: false,
            ..ShellSource::default()
        };
        assert_eq!(strict.budget(), Duration::from_millis(500));

        let relaxed = ShellSource {
            ignore_timeout: true,
            ..strict
        };
        assert_eq!(
            relaxed.budget(),
            Duration::from_millis(ShellSource::IGNORE_TIMEOUT_MS),
        );
        assert!(
            relaxed.budget() > Duration::ZERO,
            "`ignore_timeout` must never mean unbounded",
        );
    }

    /// Every variant survives the YAML round-trip the whole config goes
    /// through (`seki config-show`, the exported examples, the
    /// grid-alignment gate). An enum shape that serializes but does not
    /// deserialize would break config loading with all tests still green.
    #[test]
    fn every_source_variant_round_trips_through_yaml() {
        for src in [
            CustomSource::EnvSlice {
                var: "TEAR_PANE_ID".to_owned(),
                chars: 6,
            },
            CustomSource::EnvVar {
                var: "WORKSPACE".to_owned(),
            },
            CustomSource::Shell(ShellSource {
                command: "printf hi".to_owned(),
                when: Some("test -n \"$X\"".to_owned()),
                ignore_timeout: true,
                command_timeout_ms: 500,
            }),
        ] {
            let entry = CustomEntry {
                source: src.clone(),
                ..CustomEntry::default()
            };
            let yaml = serde_yaml::to_string(&entry).expect("serializes");
            let back: CustomEntry = serde_yaml::from_str(&yaml).expect("deserializes");
            assert_eq!(back, entry, "round trip failed for {src:?}:\n{yaml}");
        }
    }
}
