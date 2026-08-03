//! Typed config for the `custom.<NAME>` segment family.
//!
//! starship lets the operator declare arbitrary `[custom.FOO]`
//! sections that run a shell command and render its stdout. seki
//! restricts the surface — only opaque commands (matching the blzsh
//! `[custom.tear_pane]` use case). Per the NO SHELL rule we still
//! treat the command string as opaque, but the typed configuration
//! is explicit about what's being driven.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomEntry {
    pub enabled: bool,
    pub description: String,
    /// Command run to produce `$output`. NO SHELL beyond what the
    /// operator authors — the command is opaque to seki.
    pub command: String,
    /// Optional predicate command — segment is silent when this
    /// exits non-zero. Mirrors starship's `when`.
    pub when: Option<String>,
    pub style: StyleSpec,
    /// Format string. Substitution: `$output`.
    pub format: String,
    /// ── ★ RELAX THE BUDGET; NEVER REMOVE IT ────────────────────────────
    /// Was documented as "skip command-timeout enforcement", and there was
    /// no enforcement to skip — `custom.rs` ran `.output()` and `.status()`
    /// unbounded while its module docs credited a "top-level
    /// `command_timeout_ms`" that does not exist anywhere in this config
    /// tree. A configured command touching a stale mount or a down
    /// endpoint hung EVERY shell prompt.
    ///
    /// It now selects [`CustomEntry::IGNORE_TIMEOUT_MS`] instead of
    /// [`CustomEntry::command_timeout_ms`]. Deliberately not "unbounded":
    /// a prompt has no honest use for a command that may never return, so
    /// no configuration is allowed to express one. The generous ceiling is
    /// the escape hatch; infinity is not on the menu.
    pub ignore_timeout: bool,
    /// Wall-clock budget for both `command` and the `when` predicate.
    /// Per-entry, matching the convention every other probing segment
    /// already follows (arnes_cache, kasou_vm, tend, …).
    pub command_timeout_ms: u64,
}

impl CustomEntry {
    /// Budget applied when `ignore_timeout` is set. Two seconds is long
    /// enough that an operator who asked for slack gets it, and short
    /// enough that a wedged command is a stutter rather than a dead shell.
    pub const IGNORE_TIMEOUT_MS: u64 = 2_000;

    /// The budget this entry actually runs under.
    #[must_use]
    pub fn budget(&self) -> std::time::Duration {
        std::time::Duration::from_millis(if self.ignore_timeout {
            Self::IGNORE_TIMEOUT_MS
        } else {
            self.command_timeout_ms
        })
    }
}

impl Default for CustomEntry {
    fn default() -> Self {
        Self {
            enabled: true,
            description: String::new(),
            command: String::new(),
            when: None,
            style: StyleSpec::new("green bold"),
            format: "[$symbol($output )]($style)".to_owned(),
            ignore_timeout: false,
            command_timeout_ms: 500,
        }
    }
}

impl CustomEntry {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            description: String::new(),
            command: String::new(),
            when: None,
            style: StyleSpec::default(),
            format: String::new(),
            ignore_timeout: false,
            command_timeout_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
