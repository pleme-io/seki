//! Typed error surface for seki-core + seki-modules.
//!
//! Per the fleet rule: `thiserror` for typed errors, `anyhow` reserved
//! for `main.rs` glue.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SekiError {
    #[error("module {0:?} not registered")]
    UnknownModule(String),

    #[error("invalid format string for module {module:?}: {message}")]
    InvalidFormat { module: String, message: String },

    #[error("invalid style spec {0:?}: {1}")]
    InvalidStyle(String, String),

    #[error("i/o error in module {module:?}: {source}")]
    Io {
        module: String,
        #[source]
        source: std::io::Error,
    },

    #[error("config parse failure: {0}")]
    Config(String),
}

pub type SekiResult<T> = std::result::Result<T, SekiError>;
