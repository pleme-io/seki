//! seki-modules — typed [`seki_core::Module`] impls per segment.
//!
//! M2 ships nine active segments: `directory`, `git_branch`,
//! `git_status`, `hostname`, `cmd_duration`, `nix_shell`, `env_var`,
//! `custom`, `rust`. Each lives in its own file; each owns its
//! own typed config import from `seki_core::config::*`. The
//! [`default_registry`] constructor builds the canonical registry
//! from a borrowed [`SekiConfig`]; only enabled modules are
//! installed so the renderer never wastes a call on a disabled
//! segment.

use seki_core::{SekiConfig, module::ModuleRegistry};

pub mod caixa;
pub mod cmd_duration;
pub mod custom;
pub mod directory;
pub mod env_var;
pub mod git_branch;
pub mod git_status;
pub mod hostname;
pub mod nix_shell;
pub mod rust;
pub mod shikumi_tier;
pub mod tend;

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
    if cfg.hostname.enabled {
        reg.register(hostname::HostnameModule::new(cfg.hostname.clone()));
    }
    if cfg.cmd_duration.enabled {
        reg.register(cmd_duration::CmdDurationModule::new(cfg.cmd_duration.clone()));
    }
    if cfg.rust.enabled {
        reg.register(rust::RustModule::new(cfg.rust.clone()));
    }
    if cfg.nix_shell.enabled {
        reg.register(nix_shell::NixShellModule::new(cfg.nix_shell.clone()));
    }
    if cfg.env_var.entries.values().any(|e| e.enabled) {
        reg.register(env_var::EnvVarModule::new(cfg.env_var.clone()));
    }
    if cfg.custom.entries.values().any(|e| e.enabled) {
        reg.register(custom::CustomModule::new(cfg.custom.clone()));
    }
    // ── Pleme-io-native segments ───────────────────────────────
    if cfg.shikumi_tier.enabled {
        reg.register(shikumi_tier::ShikumiTierModule::new(cfg.shikumi_tier.clone()));
    }
    if cfg.caixa.enabled {
        reg.register(caixa::CaixaModule::new(cfg.caixa.clone()));
    }
    if cfg.tend.enabled {
        reg.register(tend::TendModule::new(cfg.tend.clone()));
    }
    reg
}
