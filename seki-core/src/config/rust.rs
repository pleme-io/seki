//! Typed config for the `rust` toolchain segment.

use crate::style::StyleSpec;
use ishou_tokens::{SekiSignals, SignalMode};
use serde::{Deserialize, Serialize};

/// The prescribed rust glyph for the prompt, sourced from the fleet
/// [`SekiSignals`] atlas. Emoji mode (`🦀`) is used to match seki's
/// historical `rust` symbol exactly — the adoption moves the SOURCE to
/// the fleet atlas without changing the rendered symbol. Touching
/// `SekiSignals` now propagates to seki on the next compile.
fn rust_symbol() -> String {
    // `SekiSignals::prescribed` is a `const` factory — compile-time-known.
    const PRESCRIBED: SekiSignals = SekiSignals::prescribed();
    format!("{} ", PRESCRIBED.lang_rust.render(SignalMode::Emoji))
}

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
            // 🦀 from SekiSignals.lang_rust (emoji) + trailing space.
            symbol: rust_symbol(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Forcing function: the prescribed `rust` symbol MUST equal the
    /// fleet [`SekiSignals`]`.lang_rust` emoji glyph (with seki's
    /// trailing space composed on). If the atlas glyph drifts, this
    /// fails the build — keeping seki's language icon fleet-uniform.
    #[test]
    fn rust_symbol_is_sourced_from_seki_signals() {
        let s = SekiSignals::prescribed();
        let expected = format!("{} ", s.lang_rust.render(SignalMode::Emoji));
        assert_eq!(RustConfig::default().symbol, expected);
        // Adoption is glyph-identical: the atlas emoji matches seki's
        // historical 🦀 symbol.
        assert_eq!(s.lang_rust.render(SignalMode::Emoji), "🦀");
    }
}
