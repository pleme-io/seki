//! seki-modules — typed [`seki_core::Module`] impls per segment.
//!
//! M1 ships five segments: `directory`, `git_branch`, `git_status`,
//! `rust`, `nix_shell`. Each lives in its own file; each owns its
//! own typed config import from `seki_core::config::*`. The
//! [`default_registry`] constructor builds the canonical registry
//! from a borrowed [`SekiConfig`]; only enabled modules are
//! installed so the renderer never wastes a call on a disabled
//! segment.

use seki_core::{SekiConfig, module::ModuleRegistry};

pub mod directory;
pub mod git_branch;
pub mod git_status;
pub mod nix_shell;
pub mod rust;

/// Build a registry containing every enabled module configured in
/// `cfg`. Modules with `enabled = false` are omitted.
pub fn default_registry(cfg: &SekiConfig) -> ModuleRegistry {
    let mut reg = ModuleRegistry::new();
    if cfg.directory.enabled {
        reg.register(directory::DirectoryModule::new(cfg.directory.clone()));
    }
    if cfg.git_branch.enabled {
        reg.register(git_branch::GitBranchModule::new(cfg.git_branch.clone()));
    }
    if cfg.git_status.enabled {
        reg.register(git_status::GitStatusModule::new(cfg.git_status.clone()));
    }
    if cfg.rust.enabled {
        reg.register(rust::RustModule::new(cfg.rust.clone()));
    }
    if cfg.nix_shell.enabled {
        reg.register(nix_shell::NixShellModule::new(cfg.nix_shell.clone()));
    }
    reg
}
