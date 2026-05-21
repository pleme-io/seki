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

pub mod arnes_cache;
pub mod blackmatter;
pub mod caixa;
pub mod cmd_duration;
pub mod cofre_tier;
pub mod custom;
pub mod directory;
pub mod engenho;
pub mod env_var;
pub mod fleet_node;
pub mod git_branch;
pub mod git_status;
pub mod hostname;
pub mod ishou_theme;
pub mod kasou_vm;
pub mod kindling_posture;
pub mod mado_session;
pub mod nix_flake_drift;
pub mod nix_shell;
pub mod rust;
pub mod shigoto;
pub mod shikumi_config;
pub mod shikumi_tier;
pub mod stylix;
pub mod tatara_workload;
pub mod tear;
pub mod tend;
pub mod vigy;

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
    // ── Pleme-io-native segments (Tier 1) ──────────────────────
    if cfg.shikumi_tier.enabled {
        reg.register(shikumi_tier::ShikumiTierModule::new(cfg.shikumi_tier.clone()));
    }
    if cfg.caixa.enabled {
        reg.register(caixa::CaixaModule::new(cfg.caixa.clone()));
    }
    if cfg.tend.enabled {
        reg.register(tend::TendModule::new(cfg.tend.clone()));
    }
    // ── Pleme-io-native segments (Tier 2) ──────────────────────
    if cfg.shikumi_config.enabled {
        reg.register(shikumi_config::ShikumiConfigModule::new(
            cfg.shikumi_config.clone(),
        ));
    }
    if cfg.tear.enabled {
        reg.register(tear::TearModule::new(cfg.tear.clone()));
    }
    if cfg.vigy.enabled {
        reg.register(vigy::VigyModule::new(cfg.vigy.clone()));
    }
    if cfg.fleet_node.enabled {
        reg.register(fleet_node::FleetNodeModule::new(cfg.fleet_node.clone()));
    }
    if cfg.cofre_tier.enabled {
        reg.register(cofre_tier::CofreTierModule::new(cfg.cofre_tier.clone()));
    }
    // ── Pleme-io-native segments (Tier 3 — opt-in) ─────────────
    if cfg.shigoto.enabled {
        reg.register(shigoto::ShigotoModule::new(cfg.shigoto.clone()));
    }
    if cfg.tatara_workload.enabled {
        reg.register(tatara_workload::TataraWorkloadModule::new(
            cfg.tatara_workload.clone(),
        ));
    }
    if cfg.kindling_posture.enabled {
        reg.register(kindling_posture::KindlingPostureModule::new(
            cfg.kindling_posture.clone(),
        ));
    }
    if cfg.nix_flake_drift.enabled {
        reg.register(nix_flake_drift::NixFlakeDriftModule::new(
            cfg.nix_flake_drift.clone(),
        ));
    }
    if cfg.mado_session.enabled {
        reg.register(mado_session::MadoSessionModule::new(cfg.mado_session.clone()));
    }
    // ── Pleme-io-native segments (Tier 4 — substrate-themed) ───
    if cfg.ishou_theme.enabled {
        reg.register(ishou_theme::IshouThemeModule::new(cfg.ishou_theme.clone()));
    }
    if cfg.stylix.enabled {
        reg.register(stylix::StylixModule::new(cfg.stylix.clone()));
    }
    if cfg.blackmatter.enabled {
        reg.register(blackmatter::BlackmatterModule::new(cfg.blackmatter.clone()));
    }
    // ── Pleme-io-native segments (Tier 5 — observability) ─────
    if cfg.kasou_vm.enabled {
        reg.register(kasou_vm::KasouVmModule::new(cfg.kasou_vm.clone()));
    }
    if cfg.engenho.enabled {
        reg.register(engenho::EngenhoModule::new(cfg.engenho.clone()));
    }
    if cfg.arnes_cache.enabled {
        reg.register(arnes_cache::ArnesCacheModule::new(cfg.arnes_cache.clone()));
    }
    reg
}
