# seki — HM / NixOS / Darwin module trio

The module trio is **generated** by `substrate.lib.rustWorkspaceReleaseFlake`
from the `module = { … }` attrset in `flake.nix`. See that file for the
authoritative typed-option surface; per-segment groups follow the
shape of `seki_core::config::*` 1:1.

Consume from a home-manager configuration:

```nix
{ inputs, ... }:
{
  imports = [ inputs.seki.homeManagerModules.default ];

  programs.seki = {
    enable = true;
    settings = {
      prompt.order = [ "directory" "git_branch" "git_status" "rust" "nix_shell" ];
      git_branch.style = "bold cyan";
      rust.symbol = "🦀 ";
    };
  };
}
```

The YAML at `~/.config/seki/seki.yaml` is rendered from the typed
`settings` block. Tier override: `SEKI_TIER=bare`.
