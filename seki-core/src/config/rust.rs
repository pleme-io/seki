//! Typed config for the `rust` toolchain segment.

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RustConfig {
    pub enabled: bool,
    pub symbol: String,
    pub style: StyleSpec,
    /// Filenames whose presence in CWD triggers the segment.
    pub detect_files: Vec<String>,
    /// Folders whose presence in CWD triggers the segment.
    pub detect_folders: Vec<String>,
    pub prefix: String,
    pub suffix: String,
}

impl Default for RustConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            symbol: "🦀 ".to_owned(),
            style: StyleSpec::new("bold red"),
            detect_files: vec![
                "Cargo.toml".to_owned(),
                "rust-toolchain.toml".to_owned(),
                "rust-toolchain".to_owned(),
            ],
            detect_folders: Vec::new(),
            prefix: "via ".to_owned(),
            suffix: " ".to_owned(),
        }
    }
}

impl RustConfig {
    pub fn bare() -> Self {
        Self {
            enabled: false,
            symbol: String::new(),
            style: StyleSpec::default(),
            detect_files: Vec::new(),
            detect_folders: Vec::new(),
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}
