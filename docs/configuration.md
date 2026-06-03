# Configuration

Real projects shouldn't carry 200-character command lines. Commit a `nose.toml`
(or `.nose.toml`) at the repo root and nose reads it automatically. CLI flags
from [usage](usage.md) always win; the config supplies defaults; anything unset falls
back to the built-in default. Back to [home](home.md).

## `nose.toml`

```toml
[scan]
exclude     = ["tests/**", "**/*.generated.ts", "vendor/**"]
min-value   = 200
min-members = 3
threshold   = 0.72
min-tokens  = 30
top         = 50
```

Pass an alternate file with `--config <file>`. A malformed config is a **hard
error** — a silently-ignored typo'd setting would be worse than a crash.

### Keys

All keys are optional; an absent key means "no opinion — use the CLI value or
the built-in default". Keys are kebab-case and live under the `[scan]` table.

| key | type | default | same as flag |
|---|---|---|---|
| `exclude` | list of globs | `[]` | `--exclude` |
| `min-value` | float | `0.0` | `--min-value` |
| `min-members` | int | `2` | `--min-members` |
| `threshold` | float | `0.70` | `--threshold` |
| `min-tokens` | int | `24` | `--min-tokens` |
| `top` | int | `30` | `--top` |
| `min-lines` | int | `5` | *(advanced; no flag)* |

`min-lines` is a coarse source-line size floor kept only as an advanced config
key — the CLI exposes a single size knob, `--min-tokens` (structural size in IL
nodes), which is what the detector actually gates on.

## Excludes

`exclude` is **additive**: the config's globs and any `--exclude` flags on the
command line are combined. Globs use gitignore syntax (`tests/**`,
`**/*.test.ts`, `vendor/**`) and are applied *during the directory walk*, so an
excluded directory is pruned, not just filtered out afterward.

`.gitignore` is always respected automatically, so vendored dependencies, build
output, and the like are skipped without any configuration.

## Inline suppression

To mark one specific clone as intentionally kept, put `// nose-ignore` on or
just above the unit (function/class/block). nose drops that unit from
detection, so it never shows up as a family. Use this for a duplicate you've
consciously decided to live with, rather than excluding the whole file.

For accepting *all* of today's existing duplication at once — so only *new*
duplication is reported — use a baseline instead; see
[continuous-integration](continuous-integration.md).
