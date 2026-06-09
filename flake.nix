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
            description = "Coarse worktree status (clean/modified/conflicted).";
            fields = {
              enabled = "render the git_status segment";
              clean_symbol = "shown when worktree is clean";
              conflicted_symbol = "shown on merge conflict";
              style = "starship-flavoured style string";
            };
          };
          rust = {
            description = "Rust toolchain segment.";
            fields = {
              enabled = "render the rust segment";
              symbol = "leading glyph (default '🦀 ')";
              detect_files = "files whose presence enables the segment";
              style = "starship-flavoured style string";
            };
          };
          nix_shell = {
            description = "IN_NIX_SHELL-aware segment.";
            fields = {
              enabled = "render the nix_shell segment";
              symbol = "leading glyph (default '❄️ ')";
              impure_format = "label when IN_NIX_SHELL=impure";
              pure_format = "label when IN_NIX_SHELL=pure";
              style = "starship-flavoured style string";
            };
          };
        };
      };
    };
}
