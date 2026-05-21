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
    /// Skip command-timeout enforcement for this segment.
    pub ignore_timeout: bool,
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
