# seki — Claude context

> 席 — typed prompt renderer (starship fork)

## What this repo is

A 4-crate Rust workspace (seki-core / seki-modules / seki-shikumi /
seki) that ships a typed-from-the-ground-up prompt renderer for the
pleme-io fleet. Replaces frostmourne's last stringly-typed dependency
(upstream `starship` + TOML) with a `shikumi::TieredConfig`-backed
surface + HM / NixOS / Darwin module trio.

## Pillars adhered to

- **Pillar 1 (Rust + tatara-lisp + WASM)** — all four crates are Rust.
  Frostmourne's consumer side is a `(defprompt …)` tatara-lisp form
  emitted by `seki init frostmourne`.
- **Pillar 2 (shikumi)** — `seki-shikumi::TieredSekiConfig` implements
  the four-tier contract. `seki config-show <tier>` is wired from
  `shikumi::cli::ConfigShowCommand`.
- **Pillar 9 (substrate SDLC)** — flake uses
  `substrate.lib.build.rust.workspace-release-flake`; module trio is
  generated from the `module = { … }` attrset.
- **Pillar 12 (generation over composition)** — typed config groups
  in `seki-core/src/config/*.rs` are the single source of truth; the
  Nix option surface is generated from the same shape.

## Hard rules

- NO SHELL beyond 3-line glue.
- NO `format!()` of shell-init-script syntax — every snippet flows
  through `seki/src/init.rs::SnippetBuilder` (typed AST).
- `thiserror` for typed errors; `anyhow` reserved for `main.rs`.
- Every public type derives `Debug` + `Clone` where reasonable.

## M1 scope (this milestone — shipped)

- 5 segments: `directory`, `git_branch`, `git_status`, `rust`,
  `nix_shell`.
- TieredConfig bare / discovered / default with per-segment groups.
- `seki init` snippets for bash / zsh / fish / nu / frostmourne.
- HM / NixOS / Darwin module trio via substrate's workspace-release
  flake builder.

## Out of scope for M1

- Other 35+ starship segments → M2.
- pleme-io native segments (caixa, kasou, tear, vigy, mado) → M3.
- frostmourne flake migration → M4.

## When extending

1. New segment → new file in `seki-modules/src/<name>.rs` + new typed
   group in `seki-core/src/config/<name>.rs` + entry in
   `SekiConfig::default()` + entry in
   `seki_modules::default_registry()`.
2. New shell for `seki init` → new `build_<shell>()` constructor in
   `seki/src/init.rs` + new `Shell::<Variant>` in
   `seki-core/src/context.rs`. **Never** `format!` shell syntax.
3. New typed style primitive → extend `seki-core/src/style.rs`. The
   `StyleSpec::resolve` parser is the central site.
