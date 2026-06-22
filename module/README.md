# seki — HM / NixOS / Darwin module trio

The module trio is **generated** by `substrate.lib.rustWorkspaceReleaseFlake`
from the `module = { … }` attrset in `flake.nix`. See that file for the
authoritative typed-option surface; per-segment groups follow the
shape of `seki_core::config::*` 1:1.

## Consume from a home-manager configuration

```nix
{ inputs, ... }:
{
  imports = [ inputs.seki.homeManagerModules.default ];

  programs.seki = {
    enable = true;
    # `settings` is rendered to the typed SekiConfig YAML at
    # ~/.config/seki/seki.yaml — every field of SekiConfig is overridable.
    settings = {
      # Re-order / enable / disable segments:
      prompt.order = [ "nix_shell" "directory" "git_branch" "git_status" "rust" "character" ];

      # Tweak the rich git_status emoji (companion defaults shown):
      git_status = {
        modified = "🟡"; staged = "🟢"; untracked = "⚪";
        deleted = "🔴"; renamed = "🔁"; conflicted = "💥"; stashed = "📦";
        ahead = "⇡\${count}"; behind = "⇣\${count}";
        style = "bold #EBCB8B";
      };

      git_branch.symbol = "🌿 ";
      git_branch.style  = "#A3BE8C";
      rust.symbol       = "🦀 ";
    };
  };
}
```

## The default — tuned for mado + the pleme-io Rust fleet

`seki`'s prescribed default is the **companion** prompt: a cold Nord-frost
base with warm emoji accents and the ❄ snowflake fleet signature, rendered
in 24-bit truecolor (perfect for mado). Every segment is conditional and
short-and-sweet:

```
📁 …/dir 🌿 main 🟡🟢⚪ 🦀 1.89.0 ❄
```

- **`rust`** is on by default (the fleet's mother tongue) but conditional —
  silent outside a Cargo/`rust-toolchain` repo.
- **`hostname`** is ssh-only — silent locally, appears the instant you're on
  a remote fleet node.
- **`git_status`** shows live per-category emoji and ahead/behind counts.

## Tiers — `SEKI_TIER`

| Tier | What you get |
|------|--------------|
| `bare` | zero-opinion empty floor |
| `discovered` | the companion default **adapted to the terminal it finds**: mado / truecolor → rich; narrow pane → tighter path truncation; SSH → host shown; dumb/CI/`NO_COLOR` → a plain ASCII fallback |
| `default` (the prescribed companion prompt) | the fleet-perfect default above |

```sh
SEKI_TIER=discovered seki prompt   # auto-adapt to this terminal
```

## Never-stale refresh — the FS-watch daemon

Stale status is the worst thing a prompt can show. `seki prompt` recomputes
git status **live** every render (always fresh), and an optional daemon makes
that freshness *efficient the way mado is — event-driven, not polling*:

```sh
seki daemon            # FS-watch (notify) hot-status cache over a unix socket
```

The daemon watches each repo you visit with kernel filesystem events and keeps
its status hot, so a prompt reads it in microseconds and the value is current
because every commit / `git add` / worktree edit / fetch already recomputed it.
`seki prompt` reads the daemon when present and falls back to the live fork
otherwise — so the result is identical either way, only the cost differs.

Frostmourne sets `SEKI_DAEMON=auto` in the generated init snippet, which
brings the daemon up automatically after the first prompt. Standalone shells
stay on the safe live-fork default unless you opt in:

```sh
export SEKI_DAEMON=auto    # auto-start the daemon after the first prompt
# override the socket location (default: $XDG_RUNTIME_DIR/seki/statusd.sock)
export SEKI_STATUSD_SOCK=/run/user/1000/seki.sock
```

The YAML at `~/.config/seki/seki.yaml` is rendered from the typed `settings`
block. Tier override: `SEKI_TIER=bare`.
