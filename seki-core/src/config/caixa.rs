//! Typed config for the `caixa` segment.
//!
//! Pleme-io-native: surfaces the current repo's `caixa.lisp` kind when
//! the cwd sits inside a caixa-typed repo. The five typed caixa kinds
//! (Biblioteca / Binario / Servico / Supervisor / Aplicacao) are the
//! canonical pleme-io SDLC primitive — seeing one in the prompt tells
//! the operator at a glance what shape of repo they're editing.
//!
//! # Theme
//!
//! Nord-frost blue `#88C0D0` for the resolved kind (the load-bearing
//! pleme-io frost colour — same palette as the snowflake glyph and
//! the `nix_shell` segment). Nord-aurora red `#BF616A` is reserved
//! for a future parse-error state (M3.1).
//!
//! # Probe budget
//!
//! Filesystem walk from `RenderContext::cwd` upward, capped at the
//! top-level [`crate::SekiConfig::scan_timeout_ms`]. Stops at the
//! first `caixa.lisp` it finds, the filesystem root, or the parent
//! of a `.git` directory (whichever comes first).

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

/// Typed config for the caixa prompt segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaixaConfig {
    pub enabled: bool,
    /// Format string. Substitutions:
    /// - `$kind` — resolved caixa kind (Biblioteca / Binario /
    ///   Servico / Supervisor / Aplicacao)
    /// - `$path` — relative path to `caixa.lisp` from `cwd`
    /// Starship-style `[…]($style)` markup is stripped; the renderer
    /// applies `style` directly.
    pub format: String,
    /// Style applied to the rendered text when the file parses
    /// cleanly. Defaults to Nord-frost blue `#88C0D0`.
    pub style: StyleSpec,
    /// Style applied when `caixa.lisp` is present but unparseable.
    /// Defaults to Nord-aurora red `#BF616A`.
    pub error_style: StyleSpec,
}

impl Default for CaixaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: "[$kind]($style)".to_owned(),
            style: StyleSpec::new("bold #88C0D0"),
            error_style: StyleSpec::new("bold #BF616A"),
        }
    }
}

impl CaixaConfig {
    /// Zero-opinion: nothing scanned, nothing rendered.
    pub fn bare() -> Self {
        Self {
            enabled: false,
            format: String::new(),
            style: StyleSpec::default(),
            error_style: StyleSpec::default(),
        }
    }
}
