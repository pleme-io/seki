//! seki-core — typed prompt rendering engine.
//!
//! This crate owns the **type system** behind seki: the [`Module`]
//! trait every segment implements, the [`RenderContext`] runtime
//! carrier handed to each module, the [`Segment`] / [`StyledFragment`]
//! pure-data result of a single render call, and the top-level
//! [`SekiConfig`] struct that composes the typed per-module config
//! groups.
//!
//! seki-modules sits on top of this crate and ships concrete
//! [`Module`] impls (`directory`, `git_branch`, …). seki-shikumi
//! sits on top of both and provides the `TieredConfig` impl + the
//! YAML/Lisp/env-var entrypoints. seki (the bin) wires it all into
//! a clap CLI.
//!
//! NO `format!()` for shell-init script syntax in this crate — every
//! emitted string goes through a typed renderer (see
//! [`render::render_prompt`] + [`style::apply`]).

pub mod config;
pub mod context;
pub mod error;
pub mod format;
pub mod init;
pub mod module;
pub mod palette;
pub mod render;
pub mod segment;
pub mod style;

pub use config::SekiConfig;
pub use context::{RenderContext, Shell};
pub use error::{SekiError, SekiResult};
pub use init::InitScript;
pub use module::Module;
pub use render::render_prompt;
pub use segment::{Segment, StyledFragment};
pub use style::{Color, Style, StyleSpec};
