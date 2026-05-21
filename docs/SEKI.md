# seki — design notes

> 席 — "seat / position". The cursor's seat in the prompt.

## Why fork starship?

frostmourne (mado's default shell) was the last surface in the
pleme-io fleet still depending on a stringly-typed TOML config — every
other operator-facing tool had been migrated to `shikumi::TieredConfig`
(see the org-level `CLAUDE.md`'s configuration prime directive). seki
closes that gap with a clean-slate, Rust+Lisp+Nix-native prompt
renderer that:

- exposes every starship knob as a typed Rust struct field
- materialises the HM / NixOS / Darwin module trio mechanically from
  the same typed groups (Pillar 12 — generation over composition)
- consumes the shikumi `bare / discovered / default / custom` tier
  model fleet-wide (`SEKI_TIER` env var; `seki config-show <tier>`)
- emits frostmourne init as a `(defprompt …)` tatara-lisp form so the
  shell-side wiring is typed at both ends, not just the renderer

## Architecture

| crate          | role                                                |
|---------------|-----------------------------------------------------|
| `seki-core`    | typed Config + segment rendering engine             |
| `seki-modules` | typed `Module` impls per segment                    |
| `seki-shikumi` | `TieredConfig` impl + tier loader                   |
| `seki` (bin)   | clap CLI matching starship's command-line subset    |

Bin depends on all three libs; shikumi sits on top of core; modules
sit on top of core. No cycles.

## M1 ships five segments

| segment        | purpose                                  |
|---------------|------------------------------------------|
| `directory`    | CWD truncation + `~` substitution        |
| `git_branch`   | current branch from `.git/HEAD`          |
| `git_status`   | coarse clean/modified/conflicted         |
| `rust`         | toolchain channel from rust-toolchain.toml |
| `nix_shell`    | IN_NIX_SHELL pure/impure/unknown         |

Each segment lives in its own file. Each owns its typed config in
`seki-core/src/config/<name>.rs`. Each ships ≥1 unit test.

## Typed init-snippet emission

`seki/src/init.rs` builds a typed `SnippetBuilder` of typed
`SnippetLine` enum variants — never `format!`'s shell syntax. The
renderer turns the typed AST into a string per shell; adding a sixth
shell is a new `build_<shell>()` constructor, not another
`format!`-string slinger.

This matches the fleet-wide [typed-emission rule](https://github.com/pleme-io/theory/blob/main/TYPED-EMISSION.md).

## Roadmap

- **M1 (this milestone)** — 5 segments, TieredConfig, init snippets,
  module trio. **Local cargo build + test only.** ✓
- **M2** — kubernetes / aws / docker / nodejs / python / golang / 30+
  starship segments. Real git porcelain via gix.
- **M3** — pleme-io native segments (caixa status, kasou VM health,
  tear pane info, vigy reconciler tick rate, mado session count).
- **M4** — frostmourne adoption: replace upstream `starship` in
  `frostmourne/flake.nix` with seki, retire the TOML shim.

## Naming

`seki` (席) joins the pleme-io Japanese-named foundational crates
(tatara, shikumi, sekkei, takumi, forge, …). Pillar 12 (generation
over composition) is what makes the typed-from-the-start architecture
worth the fork.
