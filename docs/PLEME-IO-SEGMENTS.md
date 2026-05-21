# Pleme-io-native seki segments

The catalog of segments that surface pleme-io primitive state in the
operator's prompt. Each segment is a typed `Module` impl in
`seki-modules/` with a typed config in `seki-core/src/config/`. All
disabled by default; the blzsh-parity prescribed_default enables
the load-bearing ones (shikumi_tier, caixa, tend).

## Naming

Module names lowercase snake_case; segment files mirror the name:
`shikumi_tier` → `seki-modules/src/shikumi_tier.rs`.

## Theme

EVERY segment defaults to the pleme-io Nord-dark palette:
- Snow / frost (cool): `#88C0D0` `#81A1C1` `#5E81AC` `#8FBCBB`
- Snowstorm (foreground): `#D8DEE9` `#E5E9F0` `#ECEFF4`
- Polar night (background-tier accents): `#2E3440` `#3B4252` `#434C5E` `#4C566A`
- Aurora (status hits):
  - red `#BF616A` (error / dirty / failed)
  - orange `#D08770` (warning / pending)
  - yellow `#EBCB8B` (tier / mode)
  - green `#A3BE8C` (clean / ok / ahead)
  - purple `#B48EAD` (special / replace mode)
- Snowflake `❄` is the canonical pleme-io glyph; reach for it in
  modules that represent fleet-wide truth (saguão / fleet_node /
  tameshi / ishou).

## Catalog (M3)

### Tier 1 — ship now (3 segments)

| name | purpose | data source |
|---|---|---|
| `shikumi_tier` | Active `<APP>_TIER` env var across the shikumi catalog | env vars `MADO_TIER` `TATARA_TIER` `...` |
| `caixa` | Current repo's caixa.lisp kind + status | `./caixa.lisp` parse |
| `tend` | Tend workspace status (dirty repos count) | `tend report --format=json` or local SQLite |

### Tier 2 — ship next (5 segments)

| name | purpose | data source |
|---|---|---|
| `shikumi_config` | Resolved-tier label per active shikumi app | shikumi config_show probe |
| `tear` | Current tear session/pane identity | `TEAR_SESSION_NAME` `TEAR_PANE_ID` env vars |
| `vigy` | Registered reconciler count + tick rate | vigy HTTP probe (38821) |
| `fleet_node` | Current node identity + cluster role | `~/.config/kindling/node.yaml` |
| `cofre_tier` | Secret backend status (akeyless / sops / mock) | cofre health probe |

### Tier 3 — ship after (5 segments)

| name | purpose | data source |
|---|---|---|
| `shigoto` | Active job DAG state | shigoto daemon probe |
| `tatara_workload` | Workload count + scheduler tick | tatara client probe |
| `kindling_posture` | Fleet posture summary | kindling node/posture file |
| `nix_flake_drift` | Number of fleet inputs ahead of HEAD | `nix flake metadata --json` over selected inputs |
| `mado_session` | Live mado session count + frame perf | mado MCP probe |

### Tier 4 — substrate-themed (3 segments)

| name | purpose | data source |
|---|---|---|
| `ishou_theme` | Active fleet theme (Bare / PlemeDark) | `ishou_tokens::FleetDefaults::prescribed()` |
| `stylix` | Active stylix base16 scheme name | env / config file |
| `blackmatter` | Number of enabled blackmatter components | HM module state probe |

### Tier 5 — observability (3 segments)

| name | purpose | data source |
|---|---|---|
| `kasou_vm` | Running VM count | `kasou list --format=json` |
| `engenho` | K8s runtime status (when running) | engenho HTTP probe |
| `arnes_cache` | P2P content cache hit rate | arnes probe |

## Probe budget

Every segment that requires a subprocess / HTTP probe is gated on
`scan_timeout_ms` (default 100ms — matches blzsh's starship
ceiling). Segments that exceed their slice MUST cache for the next
`command_timeout_ms` window and emit `(stale)` next-render rather
than re-probe.

Env-var-only segments (`shikumi_tier`, `tear`, `ishou_theme`,
`stylix`) bypass the probe budget — they read a process-local env
var and return synchronously.

## Discoverability

`seki module shikumi_tier` (per-segment debug) returns the typed
`Segment` rendered against the current env. `seki config-show
default` shows every segment's config. New segments must:

1. Add a typed `<name>Config` to `seki-core/src/config/<name>.rs`.
2. Add the field to `SekiConfig`.
3. Add the `Module` impl to `seki-modules/src/<name>.rs`.
4. Register conditionally in `seki-modules/src/lib.rs::default_registry`.
5. Add tests covering enabled + disabled + bare + a fixture render.
6. Update this doc table.

## Operator surface

Once shipped, operators get:

```text
# default-tier rendering with all Tier 1 segments active in
# a tend-managed pleme-io repo with KENSHI_TIER=bare set:

cid · ~/code/github/pleme-io/seki main ❄ [Biblioteca] [kenshi:bare] [tend: 3 dirty]
```

Every bracket is one typed segment, one Nord-themed color, one
data source — no shell escapes, no toml-format-string gymnastics.
