//! Typed `LangModuleConfig` — shared config shape for every starship
//! language module (`rust`, `golang`, `python`, `nodejs`, `ruby`,
//! `lua`, `java`, `kotlin`, `swift`, `zig`, `elixir`, `erlang`,
//! `dart`, `nim`, `ocaml`, `perl`, `php`, `haskell`, `c`, `cmake`,
//! `elm`, …).
//!
//! Per the prime directive: every language module's surface in
//! starship is the same shape (symbol / style / detect-files /
//! detect-folders / format / prefix / suffix). seki models it once.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LangModuleConfig {
    pub enabled: bool,
    pub symbol: String,
    pub style: StyleSpec,
    pub detect_files: Vec<String>,
    pub detect_folders: Vec<String>,
    pub detect_extensions: Vec<String>,
    pub format: String,
    /// Optional version-introspection command. NO SHELL — modules
    /// that detect a toolchain typically read a manifest file rather
    /// than shell out.
    pub version_format: String,
}

impl LangModuleConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            symbol: String::new(),
            style: StyleSpec::default(),
            detect_files: Vec::new(),
            detect_folders: Vec::new(),
            detect_extensions: Vec::new(),
            format: String::new(),
            version_format: String::new(),
        }
    }

    pub fn bare() -> Self {
        Self::disabled()
    }
}

impl Default for LangModuleConfig {
    fn default() -> Self {
        Self::disabled()
    }
}
