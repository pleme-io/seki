//! Typed top-level [`SekiConfig`].
//!
//! Composes one typed config group per starship-known module. Active
//! modules ship a per-module typed struct ([`directory::DirectoryConfig`],
//! [`hostname::HostnameConfig`], …). Modules that blzsh keeps
//! disabled use the shared [`disabled::DisabledModuleConfig`] —
//! per the prime directive, the one-bit `disabled = true` shape is
//! modelled once and reused. Language modules share
//! [`lang::LangModuleConfig`] for the same reason.
//!
//! Every group derives `Default` with the seki-prescribed values
//! (matching blzsh's `starship.toml` field-for-field — see
//! `seki-shikumi/src/blzsh_parity.rs` for the const config).
//!
//! `bare()` on [`SekiConfig`] zero-fills every field;
//! `prescribed_default()` (in the `TieredConfig` impl in
//! seki-shikumi) delegates to [`SekiConfig::blzsh_parity`].

pub mod arnes_cache;
pub mod blackmatter;
pub mod caixa;
pub mod character;
pub mod cmd_duration;
pub mod cofre_tier;
pub mod custom;
pub mod directory;
pub mod disabled;
pub mod engenho;
pub mod env_var;
pub mod fleet_node;
pub mod git_branch;
pub mod git_status;
pub mod hostname;
pub mod ishou_theme;
pub mod kasou_vm;
pub mod kindling_posture;
pub mod lang;
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

use crate::style::StyleSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SekiConfig {
    /// Ordered list of module names to render. Names that don't
    /// resolve in the registry are silently skipped. Mirrors
    /// starship's `format` slot.
    pub prompt_order: Vec<String>,

    /// Right-prompt slot. Empty string disables it (matches
    /// blzsh's `right_format = ""`).
    pub right_format: String,

    /// Continuation prompt — rendered when the shell waits for
    /// additional input. blzsh uses `"[❄](bold #5E81AC) "`.
    pub continuation_prompt: String,

    /// `add_newline = false` → no blank row between prompts
    /// (mado cursor-position bug — see blzsh TOML comment).
    pub add_newline: bool,

    /// Top-level scan timeout in milliseconds; per-module scans
    /// cap themselves at this number.
    pub scan_timeout_ms: u32,

    /// Hard ceiling for custom-command timeout in milliseconds.
    pub command_timeout_ms: u32,

    // --- Active (rendering) modules ---
    pub character: character::CharacterConfig,
    pub directory: directory::DirectoryConfig,
    pub git_branch: git_branch::GitBranchConfig,
    pub git_status: git_status::GitStatusConfig,
    pub hostname: hostname::HostnameConfig,
    pub cmd_duration: cmd_duration::CmdDurationConfig,
    pub nix_shell: nix_shell::NixShellConfig,
    pub env_var: env_var::EnvVarConfig,
    pub custom: custom::CustomConfig,

    // --- Language modules (shared shape) ---
    pub rust: rust::RustConfig,
    pub golang: lang::LangModuleConfig,
    pub python: lang::LangModuleConfig,
    pub nodejs: lang::LangModuleConfig,
    pub ruby: lang::LangModuleConfig,
    pub lua: lang::LangModuleConfig,
    pub c: lang::LangModuleConfig,
    pub cmake: lang::LangModuleConfig,
    pub java: lang::LangModuleConfig,
    pub dart: lang::LangModuleConfig,
    pub elixir: lang::LangModuleConfig,
    pub elm: lang::LangModuleConfig,
    pub erlang: lang::LangModuleConfig,
    pub haskell: lang::LangModuleConfig,
    pub kotlin: lang::LangModuleConfig,
    pub nim: lang::LangModuleConfig,
    pub ocaml: lang::LangModuleConfig,
    pub perl: lang::LangModuleConfig,
    pub php: lang::LangModuleConfig,
    pub swift: lang::LangModuleConfig,
    pub zig: lang::LangModuleConfig,

    // --- Disabled-by-default modules (one-bit shared shape) ---
    pub username: disabled::DisabledModuleConfig,
    pub git_commit: disabled::DisabledModuleConfig,
    pub git_state: disabled::DisabledModuleConfig,
    pub git_metrics: disabled::DisabledModuleConfig,
    pub fill: disabled::DisabledModuleConfig,
    pub time: disabled::DisabledModuleConfig,
    pub jobs: disabled::DisabledModuleConfig,
    pub docker_context: disabled::DisabledModuleConfig,
    pub kubernetes: disabled::DisabledModuleConfig,
    pub terraform: disabled::DisabledModuleConfig,
    pub aws: disabled::DisabledModuleConfig,
    pub gcloud: disabled::DisabledModuleConfig,
    pub package: disabled::DisabledModuleConfig,
    pub conda: disabled::DisabledModuleConfig,
    pub shell: disabled::DisabledModuleConfig,
    pub shlvl: disabled::DisabledModuleConfig,
    pub memory_usage: disabled::DisabledModuleConfig,
    pub battery: disabled::DisabledModuleConfig,
    pub status: disabled::DisabledModuleConfig,

    // --- Pleme-io-native modules (Tier 1 — ship now) ---
    pub shikumi_tier: shikumi_tier::ShikumiTierConfig,
    pub caixa: caixa::CaixaConfig,
    pub tend: tend::TendConfig,

    // --- Pleme-io-native modules (Tier 2 — ship next) ---
    pub shikumi_config: shikumi_config::ShikumiConfigConfig,
    pub tear: tear::TearConfig,
    pub vigy: vigy::VigyConfig,
    pub fleet_node: fleet_node::FleetNodeConfig,
    pub cofre_tier: cofre_tier::CofreTierConfig,

    // --- Pleme-io-native modules (Tier 3 — opt-in, cost > 0) ---
    pub shigoto: shigoto::ShigotoConfig,
    pub tatara_workload: tatara_workload::TataraWorkloadConfig,
    pub kindling_posture: kindling_posture::KindlingPostureConfig,
    pub nix_flake_drift: nix_flake_drift::NixFlakeDriftConfig,
    pub mado_session: mado_session::MadoSessionConfig,

    // --- Pleme-io-native modules (Tier 4 — substrate-themed) ---
    pub ishou_theme: ishou_theme::IshouThemeConfig,
    pub stylix: stylix::StylixConfig,
    pub blackmatter: blackmatter::BlackmatterConfig,

    // --- Pleme-io-native modules (Tier 5 — observability) ---
    pub kasou_vm: kasou_vm::KasouVmConfig,
    pub engenho: engenho::EngenhoConfig,
    pub arnes_cache: arnes_cache::ArnesCacheConfig,
}

impl Default for SekiConfig {
    fn default() -> Self {
        Self::seki_default()
    }
}

impl SekiConfig {
    /// The seki-flavoured default (non-blzsh). Used by the
    /// `TieredConfig` `prescribed_default()` impl as a fallback;
    /// the M3 blzsh-parity config is the actual prescribed default
    /// shipped by seki-shikumi.
    pub fn seki_default() -> Self {
        Self {
            prompt_order: vec![
                "directory".to_owned(),
                "git_branch".to_owned(),
                "git_status".to_owned(),
                "rust".to_owned(),
                "nix_shell".to_owned(),
            ],
            right_format: String::new(),
            continuation_prompt: ">> ".to_owned(),
            add_newline: false,
            scan_timeout_ms: 100,
            command_timeout_ms: 500,

            character: character::CharacterConfig::default(),
            directory: directory::DirectoryConfig::default(),
            git_branch: git_branch::GitBranchConfig::default(),
            git_status: git_status::GitStatusConfig::default(),
            hostname: hostname::HostnameConfig::default(),
            cmd_duration: cmd_duration::CmdDurationConfig::default(),
            nix_shell: nix_shell::NixShellConfig::default(),
            env_var: env_var::EnvVarConfig::default(),
            custom: custom::CustomConfig::default(),

            rust: rust::RustConfig::default(),
            golang: lang::LangModuleConfig::disabled(),
            python: lang::LangModuleConfig::disabled(),
            nodejs: lang::LangModuleConfig::disabled(),
            ruby: lang::LangModuleConfig::disabled(),
            lua: lang::LangModuleConfig::disabled(),
            c: lang::LangModuleConfig::disabled(),
            cmake: lang::LangModuleConfig::disabled(),
            java: lang::LangModuleConfig::disabled(),
            dart: lang::LangModuleConfig::disabled(),
            elixir: lang::LangModuleConfig::disabled(),
            elm: lang::LangModuleConfig::disabled(),
            erlang: lang::LangModuleConfig::disabled(),
            haskell: lang::LangModuleConfig::disabled(),
            kotlin: lang::LangModuleConfig::disabled(),
            nim: lang::LangModuleConfig::disabled(),
            ocaml: lang::LangModuleConfig::disabled(),
            perl: lang::LangModuleConfig::disabled(),
            php: lang::LangModuleConfig::disabled(),
            swift: lang::LangModuleConfig::disabled(),
            zig: lang::LangModuleConfig::disabled(),

            username: disabled::DisabledModuleConfig::default(),
            git_commit: disabled::DisabledModuleConfig::default(),
            git_state: disabled::DisabledModuleConfig::default(),
            git_metrics: disabled::DisabledModuleConfig::default(),
            fill: disabled::DisabledModuleConfig::default(),
            time: disabled::DisabledModuleConfig::default(),
            jobs: disabled::DisabledModuleConfig::default(),
            docker_context: disabled::DisabledModuleConfig::default(),
            kubernetes: disabled::DisabledModuleConfig::default(),
            terraform: disabled::DisabledModuleConfig::default(),
            aws: disabled::DisabledModuleConfig::default(),
            gcloud: disabled::DisabledModuleConfig::default(),
            package: disabled::DisabledModuleConfig::default(),
            conda: disabled::DisabledModuleConfig::default(),
            shell: disabled::DisabledModuleConfig::default(),
            shlvl: disabled::DisabledModuleConfig::default(),
            memory_usage: disabled::DisabledModuleConfig::default(),
            battery: disabled::DisabledModuleConfig::default(),
            status: disabled::DisabledModuleConfig::default(),

            shikumi_tier: shikumi_tier::ShikumiTierConfig::default(),
            caixa: caixa::CaixaConfig::default(),
            tend: tend::TendConfig::default(),

            shikumi_config: shikumi_config::ShikumiConfigConfig::default(),
            tear: tear::TearConfig::default(),
            vigy: vigy::VigyConfig::default(),
            fleet_node: fleet_node::FleetNodeConfig::default(),
            cofre_tier: cofre_tier::CofreTierConfig::default(),

            shigoto: shigoto::ShigotoConfig::default(),
            tatara_workload: tatara_workload::TataraWorkloadConfig::default(),
            kindling_posture: kindling_posture::KindlingPostureConfig::default(),
            nix_flake_drift: nix_flake_drift::NixFlakeDriftConfig::default(),
            mado_session: mado_session::MadoSessionConfig::default(),

            ishou_theme: ishou_theme::IshouThemeConfig::default(),
            stylix: stylix::StylixConfig::default(),
            blackmatter: blackmatter::BlackmatterConfig::default(),

            kasou_vm: kasou_vm::KasouVmConfig::default(),
            engenho: engenho::EngenhoConfig::default(),
            arnes_cache: arnes_cache::ArnesCacheConfig::default(),
        }
    }

    /// Zero-opinion floor — see the `TieredConfig` impl in
    /// seki-shikumi for the canonical entrypoint.
    pub fn bare() -> Self {
        Self {
            prompt_order: Vec::new(),
            right_format: String::new(),
            continuation_prompt: String::new(),
            add_newline: false,
            scan_timeout_ms: 0,
            command_timeout_ms: 0,

            character: character::CharacterConfig::bare(),
            directory: directory::DirectoryConfig::bare(),
            git_branch: git_branch::GitBranchConfig::bare(),
            git_status: git_status::GitStatusConfig::bare(),
            hostname: hostname::HostnameConfig::bare(),
            cmd_duration: cmd_duration::CmdDurationConfig::bare(),
            nix_shell: nix_shell::NixShellConfig::bare(),
            env_var: env_var::EnvVarConfig::bare(),
            custom: custom::CustomConfig::bare(),

            rust: rust::RustConfig::bare(),
            golang: lang::LangModuleConfig::bare(),
            python: lang::LangModuleConfig::bare(),
            nodejs: lang::LangModuleConfig::bare(),
            ruby: lang::LangModuleConfig::bare(),
            lua: lang::LangModuleConfig::bare(),
            c: lang::LangModuleConfig::bare(),
            cmake: lang::LangModuleConfig::bare(),
            java: lang::LangModuleConfig::bare(),
            dart: lang::LangModuleConfig::bare(),
            elixir: lang::LangModuleConfig::bare(),
            elm: lang::LangModuleConfig::bare(),
            erlang: lang::LangModuleConfig::bare(),
            haskell: lang::LangModuleConfig::bare(),
            kotlin: lang::LangModuleConfig::bare(),
            nim: lang::LangModuleConfig::bare(),
            ocaml: lang::LangModuleConfig::bare(),
            perl: lang::LangModuleConfig::bare(),
            php: lang::LangModuleConfig::bare(),
            swift: lang::LangModuleConfig::bare(),
            zig: lang::LangModuleConfig::bare(),

            username: disabled::DisabledModuleConfig::bare(),
            git_commit: disabled::DisabledModuleConfig::bare(),
            git_state: disabled::DisabledModuleConfig::bare(),
            git_metrics: disabled::DisabledModuleConfig::bare(),
            fill: disabled::DisabledModuleConfig::bare(),
            time: disabled::DisabledModuleConfig::bare(),
            jobs: disabled::DisabledModuleConfig::bare(),
            docker_context: disabled::DisabledModuleConfig::bare(),
            kubernetes: disabled::DisabledModuleConfig::bare(),
            terraform: disabled::DisabledModuleConfig::bare(),
            aws: disabled::DisabledModuleConfig::bare(),
            gcloud: disabled::DisabledModuleConfig::bare(),
            package: disabled::DisabledModuleConfig::bare(),
            conda: disabled::DisabledModuleConfig::bare(),
            shell: disabled::DisabledModuleConfig::bare(),
            shlvl: disabled::DisabledModuleConfig::bare(),
            memory_usage: disabled::DisabledModuleConfig::bare(),
            battery: disabled::DisabledModuleConfig::bare(),
            status: disabled::DisabledModuleConfig::bare(),

            shikumi_tier: shikumi_tier::ShikumiTierConfig::bare(),
            caixa: caixa::CaixaConfig::bare(),
            tend: tend::TendConfig::bare(),

            shikumi_config: shikumi_config::ShikumiConfigConfig::bare(),
            tear: tear::TearConfig::bare(),
            vigy: vigy::VigyConfig::bare(),
            fleet_node: fleet_node::FleetNodeConfig::bare(),
            cofre_tier: cofre_tier::CofreTierConfig::bare(),

            shigoto: shigoto::ShigotoConfig::bare(),
            tatara_workload: tatara_workload::TataraWorkloadConfig::bare(),
            kindling_posture: kindling_posture::KindlingPostureConfig::bare(),
            nix_flake_drift: nix_flake_drift::NixFlakeDriftConfig::bare(),
            mado_session: mado_session::MadoSessionConfig::bare(),

            ishou_theme: ishou_theme::IshouThemeConfig::bare(),
            stylix: stylix::StylixConfig::bare(),
            blackmatter: blackmatter::BlackmatterConfig::bare(),

            kasou_vm: kasou_vm::KasouVmConfig::bare(),
            engenho: engenho::EngenhoConfig::bare(),
            arnes_cache: arnes_cache::ArnesCacheConfig::bare(),
        }
    }
}

/// Tiny helper to make the blzsh-parity config readable.
pub(crate) fn _typed_style(s: &str) -> StyleSpec {
    StyleSpec::new(s)
}
