# seki (席)

Typed prompt renderer for pleme-io — a starship fork wrapped in the
full pleme-io stack (`shikumi::TieredConfig`, NixOS/Darwin/HM module
trio, Pillar 12 typed config surface).

## Quick start

```bash
# Render the prompt at the default tier.
seki prompt

# Show what the default tier resolves to (YAML).
seki config-show default

# Diff bare against default.
diff <(seki config-show bare) <(seki config-show default)

# Emit shell-init snippet.
eval "$(seki init zsh)"

# Render one module for debugging.
seki module directory
```

## Tiers

| Tier         | What it gives you                                       |
|--------------|--------------------------------------------------------|
| `bare`       | Zero opinions — empty prompt order, all modules off.   |
| `discovered` | bare + auto-detect (M1: returns bare; M2: real detect).|
| `default`    | The fleet pleme-io prescribed look (5 segments).       |
| `custom`     | YAML overlay at `~/.config/seki/seki.yaml`.            |

Override per-launch: `SEKI_TIER=bare seki prompt`.

## See also

- `docs/SEKI.md` — design notes + architecture
- `module/README.md` — HM / NixOS / Darwin module surface
- The org-level prime directive: `~/code/github/pleme-io/CLAUDE.md`
