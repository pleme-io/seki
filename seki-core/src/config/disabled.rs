//! Typed `DisabledModuleConfig` — a degenerate config used for every
//! starship module the operator wants to be reachable in `config-show`
//! but is currently turned off.
//!
//! Why a shared type instead of one struct per disabled module? Per
//! the prime directive (macros everywhere / duplication is a bug):
//! every `[X]\ndisabled = true` row in the reference TOML carries the
//! same one-bit shape, so we model the shape once and reuse it. When
//! an operator wants to enable a module, the migration is "promote
//! `DisabledModuleConfig` → typed-per-module struct" — a separate
//! concrete struct is justified only when there's >1 typed knob to
//! configure.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisabledModuleConfig {
    pub enabled: bool,
}

impl Default for DisabledModuleConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl DisabledModuleConfig {
    pub fn bare() -> Self {
        Self { enabled: false }
    }

    /// Force enable — used by the rare situation where an operator
    /// wants to flip a disabled module on without adding a typed
    /// struct (M2 escape hatch; M3+ a typed struct is preferred).
    pub fn on() -> Self {
        Self { enabled: true }
    }
}
