//! Typed config for the `env_var.<NAME>` segment family.
//!
//! starship lets the operator declare arbitrary `[env_var.FOO]`
//! sections; each renders the env var of that name. We model the
//! same shape but keyed by variable name.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Per-variable config — exactly mirrors starship's
/// `[env_var.NAME]` shape (variable + default + style + format).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVarEntry {
    pub enabled: bool,
    /// The env variable name to read; if `None`, falls back to the
    /// table key.
    pub variable: Option<String>,
    /// Value to use when the env var is unset.
    pub default: String,
    pub style: StyleSpec,
    /// Format string. Substitution: `$env_value`.
    pub format: String,
}

impl Default for EnvVarEntry {
    fn default() -> Self {
        Self {
            enabled: true,
            variable: None,
            default: String::new(),
            style: StyleSpec::new("black bold dimmed"),
            format: "with [$env_value]($style) ".to_owned(),
        }
    }
}

impl EnvVarEntry {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            variable: None,
            default: String::new(),
            style: StyleSpec::default(),
            format: String::new(),
        }
    }
}

/// All `[env_var.*]` sections, indexed by the key (e.g. `"WORKSPACE"`).
/// BTreeMap so render order is deterministic.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EnvVarConfig {
    pub entries: BTreeMap<String, EnvVarEntry>,
}

impl EnvVarConfig {
    pub fn bare() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}
