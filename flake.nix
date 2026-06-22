{
  description = "seki (席) — typed prompt renderer (starship fork)";

  nixConfig = {
    allow-import-from-derivation = true;
  };

  inputs = {
    nixpkgs.follows = "substrate/nixpkgs";
    crate2nix.url = "github:nix-community/crate2nix";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    substrate = {
      url = "github:pleme-io/substrate";
      inputs.fenix.follows = "fenix";
    };
    devenv = {
      url = "github:cachix/devenv";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    crate2nix,
    flake-utils,
    substrate,
    devenv,
    fenix,
    ...
  }:
    (import "${substrate}/lib/build/rust/workspace-release-flake.nix" {
      inherit nixpkgs crate2nix flake-utils fenix devenv;
    }) {
      toolName = "seki";
      packageName = "seki";
      src = self;
      repo = "pleme-io/seki";

      # ★ Configuration Management prime directive — see
      # pleme-io/theory/CONFIGURATION-MANAGEMENT.md. The HM /
      # NixOS / Darwin module trio is auto-generated from this
      # `module` attrset; per-segment options materialise via
      # `withShikumiConfig = true` reading the typed SekiConfig
      # YAML at ~/.config/seki/seki.yaml.
      module = {
        description = "seki (席) — typed prompt renderer with full per-segment Nix option surface";
        hmNamespace = "programs";
        withShikumiConfig = true;
        shikumiConfigPath = ".config/seki/seki.yaml";

        # The typed option groups every consumer gets. Each group
        # mirrors a `seki_core::config::*` struct so the surface
        # is generated, not hand-written twice.
        configGroups = {
          prompt = {
            description = "Top-level prompt configuration (order + trailing character).";
            fields = {
              order = "list of segment names to render in order";
            };
          };
          directory = {
            description = "CWD truncation + home symbol.";
            fields = {
              enabled = "render the directory segment";
              truncation_length = "max path components shown";
              home_symbol = "substituted for $HOME (default '~')";
              style = "starship-flavoured style string";
            };
          };
          git_branch = {
            description = "Current git branch with optional truncation.";
            fields = {
              enabled = "render the git_branch segment";
              symbol = "leading glyph (default ' ')";
              truncation_length = "max branch-name length";
              style = "starship-flavoured style string";
            };
          };
          git_status = {
            description = ''
              Live worktree status as per-category emoji/glyph indicators.
              Each non-zero category renders its symbol (companion defaults:
              🟡 modified · 🟢 staged · ⚪ untracked · 🔴 deleted · 🔁 renamed ·
              💥 conflicted · 📦 stashed) plus ahead/behind (⇡N ⇣N ⇕). Status
              is computed live (porcelain v2) or read hot from the refresh
              daemon — never stale.
            '';
            fields = {
              enabled = "render the git_status segment";
              format = "starship format, default '[$all_status$ahead_behind]($style) '";
              modified = "symbol when the worktree has modified files";
              staged = "symbol for index-staged changes";
              untracked = "symbol for untracked files";
              deleted = "symbol for deleted files";
              renamed = "symbol for renamed files";
              conflicted = "symbol for merge conflicts";
              stashed = "symbol when the stash is non-empty";
              ahead = "ahead-of-upstream glyph (supports \${count})";
              behind = "behind-upstream glyph (supports \${count})";
              diverged = "diverged glyph (supports \${ahead_count}/\${behind_count})";
              clean_symbol = "optional symbol when the tree is clean (default: render nothing)";
              style = "starship-flavoured style string";
            };
          };
          rust = {
            description = "Rust toolchain segment — on by default (Rust-dominant fleet); conditional on a detected Cargo/rust-toolchain repo.";
            fields = {
              enabled = "render the rust segment";
              symbol = "leading glyph (default '🦀 ')";
              detect_files = "files whose presence enables the segment";
              style = "starship-flavoured style string";
            };
          };
          nix_shell = {
            description = "IN_NIX_SHELL-aware segment (the ❄ fleet signature).";
            fields = {
              enabled = "render the nix_shell segment";
              symbol = "leading glyph (default '❄️ ')";
              impure_format = "label when IN_NIX_SHELL=impure";
              pure_format = "label when IN_NIX_SHELL=pure";
              style = "starship-flavoured style string";
            };
          };
          hostname = {
            description = "System hostname — ssh-only in the companion default (silent locally, appears on a remote fleet node).";
            fields = {
              enabled = "render the hostname segment";
              ssh_only = "only render when over SSH";
              trim_at = "truncate the hostname at this separator (default '.')";
              format = "starship format, default '🖥 [$hostname](dimmed $style) '";
              style = "starship-flavoured style string";
            };
          };
          cmd_duration = {
            description = "Elapsed time of the previous command, when it exceeded min_time.";
            fields = {
              enabled = "render the cmd_duration segment";
              min_time = "minimum elapsed ms before the segment shows";
              format = "starship format, default '⏱ [$duration]($style) '";
              style = "starship-flavoured style string";
            };
          };
          character = {
            description = "The trailing prompt character — the ❄ fleet snowflake.";
            fields = {
              success_symbol = "shown after a 0 exit (default '[❄](bold #88C0D0)')";
              error_symbol = "shown after a non-zero exit (default '[❄](bold #BF616A)')";
            };
          };
        };
      };
    };
}
