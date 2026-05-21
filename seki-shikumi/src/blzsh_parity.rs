//! blzsh-parity `SekiConfig` — matches the reference starship.toml
//! at `/tmp/blzsh-starship.toml` (snapshot 2026-05-21) field-for-field.
//!
//! This is the M3 "configure seki like blzsh" deliverable. Operators
//! switching from blzsh's starship → frostmourne's seki see the
//! exact same Nord-themed snowflake prompt by default. Each
//! comment-anchor `// blzsh: <field> = <value>` paired with the
//! actual assignment is a hand-verifiable correspondence to the
//! TOML source.
//!
//! `prescribed_default()` in [`crate::TieredConfig`] reads from this
//! function (not [`SekiConfig::seki_default`]). Operators inspect
//! the materialized config via `seki config-show default`; the
//! `examples/blzsh-parity.yaml` is the same data exported to YAML.

use seki_core::SekiConfig;
use seki_core::config::{
    character::CharacterConfig,
    cmd_duration::CmdDurationConfig,
    custom::CustomConfig,
    directory::DirectoryConfig,
    disabled::DisabledModuleConfig,
    env_var::{EnvVarConfig, EnvVarEntry},
    git_branch::GitBranchConfig,
    git_status::GitStatusConfig,
    hostname::HostnameConfig,
    lang::LangModuleConfig,
    nix_shell::NixShellConfig,
    rust::RustConfig,
};
use seki_core::style::StyleSpec;
use std::collections::BTreeMap;

/// The blzsh-parity config — the seki equivalent of
/// `/tmp/blzsh-starship.toml`. Hand-verified field-for-field
/// against the reference TOML (see `examples/blzsh-parity.yaml`
/// for the operator-readable serialization).
pub fn blzsh_parity_config() -> SekiConfig {
    SekiConfig {
        // blzsh: format = "$nix_shell$env_var_WORKSPACE$env_var_TEAR_SESSION_NAME
        //                  $custom$hostname$directory$git_branch$git_status$character"
        // Order matches the format string exactly.
        prompt_order: vec![
            "nix_shell".to_owned(),
            "env_var".to_owned(),       // WORKSPACE + TEAR_SESSION_NAME
            "custom".to_owned(),        // tear_pane
            "hostname".to_owned(),
            "directory".to_owned(),
            "git_branch".to_owned(),
            "git_status".to_owned(),
            "character".to_owned(),
        ],
        // blzsh: right_format = ""
        right_format: String::new(),
        // blzsh: continuation_prompt = "[❄](bold #5E81AC) "
        continuation_prompt: "[❄](bold #5E81AC) ".to_owned(),
        // blzsh: add_newline = false
        add_newline: false,
        // blzsh: scan_timeout = 100
        scan_timeout_ms: 100,
        // blzsh: command_timeout = 500
        command_timeout_ms: 500,

        // [character]
        // success_symbol = "[❄](bold #88C0D0)"
        // error_symbol   = "[❄](bold #BF616A)"
        // vicmd_symbol   = "[❄](bold #A3BE8C)"
        // vimcmd_replace_one_symbol = "[❄](bold #B48EAD)"
        // vimcmd_replace_symbol     = "[❄](bold #B48EAD)"
        // vimcmd_visual_symbol      = "[❄](bold #EBCB8B)"
        // format = "$symbol "
        character: CharacterConfig {
            enabled: true,
            format: "$symbol ".to_owned(),
            success_symbol: "[❄](bold #88C0D0)".to_owned(),
            error_symbol: "[❄](bold #BF616A)".to_owned(),
            vicmd_symbol: "[❄](bold #A3BE8C)".to_owned(),
            vimcmd_replace_one_symbol: "[❄](bold #B48EAD)".to_owned(),
            vimcmd_replace_symbol: "[❄](bold #B48EAD)".to_owned(),
            vimcmd_visual_symbol: "[❄](bold #EBCB8B)".to_owned(),
            style: StyleSpec::new("bold #88C0D0"),
        },

        // [git_branch]
        // format = " [$symbol$branch]($style)"
        // symbol = " "
        // style  = "#A3BE8C"
        // truncation_length = 20
        // truncation_symbol = "…"
        // only_attached = false
        git_branch: GitBranchConfig {
            enabled: true,
            format: " [$symbol$branch]($style)".to_owned(),
            symbol: " ".to_owned(),
            style: StyleSpec::new("#A3BE8C"),
            truncation_length: 20,
            truncation_symbol: "…".to_owned(),
            only_attached: false,
            // M1 module impl still uses prefix/suffix; mirror blzsh's
            // implicit prefix/suffix by leaving the leading space in
            // format string and adding none here.
            prefix: String::new(),
            suffix: String::new(),
        },

        // [git_status]
        // format = '[$all_status$ahead_behind]($style) '
        // style  = "bold #EBCB8B"
        // stashed = "$$"   ahead = "⇡${count}"   behind = "⇣${count}"
        // diverged = "⇕${ahead_count}⇣${behind_count}"
        // conflicted = "=" deleted = "✘" renamed = "»"
        // modified = "!"   staged = "+" untracked = "?"
        // up_to_date = ""
        git_status: GitStatusConfig {
            enabled: true,
            format: "[$all_status$ahead_behind]($style) ".to_owned(),
            style: StyleSpec::new("bold #EBCB8B"),
            stashed: "$$".to_owned(),
            ahead: "⇡${count}".to_owned(),
            behind: "⇣${count}".to_owned(),
            diverged: "⇕${ahead_count}⇣${behind_count}".to_owned(),
            conflicted: "=".to_owned(),
            deleted: "✘".to_owned(),
            renamed: "»".to_owned(),
            modified: "!".to_owned(),
            staged: "+".to_owned(),
            untracked: "?".to_owned(),
            up_to_date: String::new(),
            clean_symbol: String::new(), // blzsh leaves this empty (up_to_date == "")
            prefix: String::new(),
            suffix: " ".to_owned(),
        },

        // [hostname]
        // disabled = false   ssh_only = false
        // format = "[$hostname](dimmed $style) · "
        // style  = "#88C0D0"
        // trim_at = "."
        hostname: HostnameConfig {
            enabled: true,
            ssh_only: false,
            trim_at: ".".to_owned(),
            style: StyleSpec::new("dimmed #88C0D0"),
            format: "[$hostname](dimmed $style) · ".to_owned(),
        },

        // [directory]
        // format = "[$path]($style)"
        // style  = "bold #81A1C1"
        // truncation_length = 3
        // truncate_to_repo  = true
        // truncation_symbol = "…/"
        // home_symbol = "~"
        // read_only        = " 󰌾"   read_only_style = "#BF616A"
        directory: DirectoryConfig {
            enabled: true,
            format: "[$path]($style)".to_owned(),
            truncation_length: 3,
            truncate_to_repo: true,
            truncation_symbol: "…/".to_owned(),
            home_symbol: "~".to_owned(),
            read_only: " \u{f0c33}".to_owned(), // nf-md-lock_outline
            read_only_style: StyleSpec::new("#BF616A"),
            style: StyleSpec::new("bold #81A1C1"),
            suffix: String::new(),
        },

        // [cmd_duration]
        // min_time = 2_000   format = "[$duration]($style) "
        // style    = "bold #D08770"
        cmd_duration: CmdDurationConfig {
            enabled: true,
            min_time: 2_000,
            style: StyleSpec::new("bold #D08770"),
            format: "[$duration]($style) ".to_owned(),
            show_milliseconds: false,
        },

        // [nix_shell]
        // disabled = false
        // format = '[$symbol]($style) '
        // symbol = "❄"
        // style  = "bold #88C0D0"
        nix_shell: NixShellConfig {
            enabled: true,
            format: "[$symbol]($style) ".to_owned(),
            symbol: "❄".to_owned(),
            style: StyleSpec::new("bold #88C0D0"),
            impure_format: "impure".to_owned(),
            pure_format: "pure".to_owned(),
            unknown_format: "nix".to_owned(),
            prefix: String::new(),
            suffix: " ".to_owned(),
        },

        // [env_var.WORKSPACE]
        // format = '[\[$env_value\]]($style) '
        // style = 'dimmed italic'   variable = 'WORKSPACE'
        //
        // [env_var.TEAR_SESSION_NAME]
        // variable = 'TEAR_SESSION_NAME'   default = ''
        // format = '[~ $env_value]($style) '
        // style  = 'bold #88C0D0'
        env_var: EnvVarConfig {
            entries: {
                let mut m = BTreeMap::new();
                m.insert(
                    "WORKSPACE".to_owned(),
                    EnvVarEntry {
                        enabled: true,
                        variable: Some("WORKSPACE".to_owned()),
                        default: String::new(),
                        style: StyleSpec::new("dimmed italic"),
                        format: "[\\[$env_value\\]]($style) ".to_owned(),
                    },
                );
                m.insert(
                    "TEAR_SESSION_NAME".to_owned(),
                    EnvVarEntry {
                        enabled: true,
                        variable: Some("TEAR_SESSION_NAME".to_owned()),
                        default: String::new(),
                        style: StyleSpec::new("bold #88C0D0"),
                        format: "[~ $env_value]($style) ".to_owned(),
                    },
                );
                m
            },
        },

        // [custom.tear_pane]
        // description = "Truncated TEAR_PANE_ID inside a tear pane"
        // command = 'printf "%s" "${TEAR_PANE_ID:0:6}"'
        // when    = '[ -n "$TEAR_PANE_ID" ]'
        // format  = '[· $output]($style) '
        // style   = 'dimmed #88C0D0'
        // ignore_timeout = true
        custom: CustomConfig {
            entries: {
                let mut m = BTreeMap::new();
                m.insert(
                    "tear_pane".to_owned(),
                    seki_core::config::custom::CustomEntry {
                        enabled: true,
                        description: "Truncated TEAR_PANE_ID inside a tear pane".to_owned(),
                        command: "printf \"%s\" \"${TEAR_PANE_ID:0:6}\"".to_owned(),
                        when: Some("[ -n \"$TEAR_PANE_ID\" ]".to_owned()),
                        style: StyleSpec::new("dimmed #88C0D0"),
                        format: "[· $output]($style) ".to_owned(),
                        ignore_timeout: true,
                    },
                );
                m
            },
        },

        // [rust] / [golang] / [python] / [nodejs] / [ruby] / [lua] /
        // [docker_context] / [kubernetes] / [terraform] / [aws] /
        // [gcloud] / [c] / [cmake] / [java] / [dart] / [elixir] /
        // [elm] / [erlang] / [haskell] / [kotlin] / [nim] / [ocaml] /
        // [perl] / [php] / [swift] / [zig] / [package] / [conda]
        //   disabled = true
        rust: RustConfig {
            enabled: false,
            ..RustConfig::default()
        },
        golang: LangModuleConfig::disabled(),
        python: LangModuleConfig::disabled(),
        nodejs: LangModuleConfig::disabled(),
        ruby: LangModuleConfig::disabled(),
        lua: LangModuleConfig::disabled(),
        c: LangModuleConfig::disabled(),
        cmake: LangModuleConfig::disabled(),
        java: LangModuleConfig::disabled(),
        dart: LangModuleConfig::disabled(),
        elixir: LangModuleConfig::disabled(),
        elm: LangModuleConfig::disabled(),
        erlang: LangModuleConfig::disabled(),
        haskell: LangModuleConfig::disabled(),
        kotlin: LangModuleConfig::disabled(),
        nim: LangModuleConfig::disabled(),
        ocaml: LangModuleConfig::disabled(),
        perl: LangModuleConfig::disabled(),
        php: LangModuleConfig::disabled(),
        swift: LangModuleConfig::disabled(),
        zig: LangModuleConfig::disabled(),

        // [username] / [git_commit] / [git_state] / [git_metrics] /
        // [fill] / [time] / [jobs] / [docker_context] / [kubernetes] /
        // [terraform] / [aws] / [gcloud] / [package] / [conda] /
        // [shell] / [shlvl] / [memory_usage] / [battery] / [status]
        //   disabled = true
        username: DisabledModuleConfig::default(),
        git_commit: DisabledModuleConfig::default(),
        git_state: DisabledModuleConfig::default(),
        git_metrics: DisabledModuleConfig::default(),
        fill: DisabledModuleConfig::default(),
        time: DisabledModuleConfig::default(),
        jobs: DisabledModuleConfig::default(),
        docker_context: DisabledModuleConfig::default(),
        kubernetes: DisabledModuleConfig::default(),
        terraform: DisabledModuleConfig::default(),
        aws: DisabledModuleConfig::default(),
        gcloud: DisabledModuleConfig::default(),
        package: DisabledModuleConfig::default(),
        conda: DisabledModuleConfig::default(),
        shell: DisabledModuleConfig::default(),
        shlvl: DisabledModuleConfig::default(),
        memory_usage: DisabledModuleConfig::default(),
        battery: DisabledModuleConfig::default(),
        status: DisabledModuleConfig::default(),

        // ── Pleme-io-native (M3 Tier 1) ───────────────────────
        // shikumi_tier surfaces active <APP>_TIER env vars. Default
        // ON so operators see tier overrides instantly — Nord-aurora
        // yellow signals "override in effect".
        shikumi_tier: seki_core::config::shikumi_tier::ShikumiTierConfig::default(),
        // caixa surfaces the current repo's caixa.lisp kind when the
        // cwd sits inside a caixa-typed repo. Nord-frost blue signals
        // "this is a pleme-io SDLC-typed repo" at a glance.
        caixa: seki_core::config::caixa::CaixaConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blzsh_parity_enables_blzsh_active_modules() {
        let c = blzsh_parity_config();
        // Active modules in the reference TOML.
        assert!(c.character.enabled);
        assert!(c.git_branch.enabled);
        assert!(c.git_status.enabled);
        assert!(c.hostname.enabled);
        assert!(c.directory.enabled);
        assert!(c.cmd_duration.enabled);
        assert!(c.nix_shell.enabled);
        assert!(c.env_var.entries["WORKSPACE"].enabled);
        assert!(c.env_var.entries["TEAR_SESSION_NAME"].enabled);
        assert!(c.custom.entries["tear_pane"].enabled);
    }

    #[test]
    fn blzsh_parity_disables_blzsh_disabled_modules() {
        let c = blzsh_parity_config();
        assert!(!c.rust.enabled);
        assert!(!c.golang.enabled);
        assert!(!c.python.enabled);
        assert!(!c.nodejs.enabled);
        assert!(!c.kubernetes.enabled);
        assert!(!c.terraform.enabled);
        assert!(!c.aws.enabled);
        assert!(!c.username.enabled);
        assert!(!c.git_commit.enabled);
        assert!(!c.time.enabled);
        assert!(!c.battery.enabled);
    }

    #[test]
    fn blzsh_parity_character_uses_nord_frost_snowflake() {
        let c = blzsh_parity_config();
        assert_eq!(c.character.success_symbol, "[❄](bold #88C0D0)");
        assert_eq!(c.character.error_symbol, "[❄](bold #BF616A)");
        assert_eq!(c.character.vicmd_symbol, "[❄](bold #A3BE8C)");
    }

    #[test]
    fn blzsh_parity_continuation_uses_nord_frost_blue() {
        let c = blzsh_parity_config();
        assert_eq!(c.continuation_prompt, "[❄](bold #5E81AC) ");
    }

    #[test]
    fn blzsh_parity_timeouts() {
        let c = blzsh_parity_config();
        assert_eq!(c.scan_timeout_ms, 100);
        assert_eq!(c.command_timeout_ms, 500);
        assert!(!c.add_newline);
    }

    #[test]
    fn blzsh_parity_prompt_order_matches_format_string() {
        let c = blzsh_parity_config();
        // Format string: nix_shell, env_var_WORKSPACE,
        // env_var_TEAR_SESSION_NAME, custom, hostname, directory,
        // git_branch, git_status, character.
        // seki collapses env_var_* into one module, so we expect:
        let expected = [
            "nix_shell",
            "env_var",
            "custom",
            "hostname",
            "directory",
            "git_branch",
            "git_status",
            "character",
        ];
        let actual: Vec<&str> = c.prompt_order.iter().map(String::as_str).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn blzsh_parity_env_vars_match_reference() {
        let c = blzsh_parity_config();
        let ws = &c.env_var.entries["WORKSPACE"];
        assert_eq!(ws.variable.as_deref(), Some("WORKSPACE"));
        assert_eq!(ws.format, "[\\[$env_value\\]]($style) ");
        assert_eq!(ws.style.as_str(), "dimmed italic");
        let tear = &c.env_var.entries["TEAR_SESSION_NAME"];
        assert_eq!(tear.variable.as_deref(), Some("TEAR_SESSION_NAME"));
        assert_eq!(tear.format, "[~ $env_value]($style) ");
    }

    #[test]
    fn blzsh_parity_custom_tear_pane_matches_reference() {
        let c = blzsh_parity_config();
        let tp = &c.custom.entries["tear_pane"];
        assert_eq!(tp.command, "printf \"%s\" \"${TEAR_PANE_ID:0:6}\"");
        assert_eq!(tp.when.as_deref(), Some("[ -n \"$TEAR_PANE_ID\" ]"));
        assert_eq!(tp.format, "[· $output]($style) ");
        assert!(tp.ignore_timeout);
    }
}
