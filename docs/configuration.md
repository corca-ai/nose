# Configuration

Real projects shouldn't carry 200-character command lines. Commit a `nose.toml`
(or `.nose.toml`) at the repo root and nose reads it automatically. CLI flags
from [usage](usage.md) always win; the config supplies defaults; anything unset falls
back to the built-in default. Back to [home](home.md).

## `nose.toml`

```toml
[scan]
exclude     = ["tests/**", "**/*.generated.ts", "vendor/**"]
mode        = ["syntax", "semantic"]
sort        = "extractability"
min-value   = 200
min-members = 3
min-tokens  = 30
top         = 50
ignore-file = "nose.ignore.json"
```

Pass an alternate file with `--config <file>`. A malformed config is a **hard
error** — a silently-ignored typo'd setting would be worse than a crash.

### Keys

All keys are optional; an absent key means "no opinion — use the CLI value or
the built-in default". Keys are kebab-case and live under the `[scan]` table.

| key | type | default | same as flag |
|---|---|---|---|
| `exclude` | list of globs | `[]` | `--exclude` |
| `mode` | list of `syntax`\|`semantic`\|`near` | `["syntax", "semantic"]` | `--mode` |
| `sort` | `extractability`\|`value`\|`sites` | `extractability` | `--sort` |
| `min-value` | float | `0.0` | `--min-value` |
| `min-members` | int | `2` | `--min-members` |
| `threshold` | float | `0.70` when `near` is enabled | `--threshold` |
| `min-tokens` | int | `24` | `--min-tokens` |
| `min-lines` | int | `5` | `--min-lines` |
| `top` | int | `30` | `--top` |
| `ignore-file` | string path | auto-read `nose.ignore.json` when present | `--ignore-file` |

`mode` is a TOML array, even for one channel:

```toml
[scan]
mode = ["syntax"]                  # jscpd-style gate
# mode = ["syntax", "semantic"]    # same as omitting mode
# mode = ["syntax", "semantic", "near"]
```

`min-tokens` and `min-lines` apply to both structural units and the syntax copy-paste
floor. For `--mode syntax`, those two settings are the jscpd-style size gate.

`threshold` is valid only when `mode` includes `near`; `syntax` and `semantic` are
exact channels and do not use fuzzy similarity. When omitted for `near`, the
threshold defaults to `0.70`.

If `threshold` is set in config and a CLI `--mode` override excludes `near`, the run
fails instead of silently ignoring the threshold. Keep `threshold` next to a `mode` that
includes `near`, or pass both on the command line:

```sh
nose scan src --mode syntax,semantic,near --threshold 0.70
```

## Excludes

`exclude` is **additive**: the config's globs and any `--exclude` flags on the
command line are combined. Globs use gitignore syntax (`tests/**`,
`**/*.test.ts`, `vendor/**`) and are applied *during the directory walk*, so an
excluded directory is pruned, not just filtered out afterward.

`.gitignore` is always respected automatically, so vendored dependencies, build
output, and the like are skipped without any configuration.

## Structured ignores

`ignore-file` points to a structured suppression file for reviewed findings:

```toml
[scan]
ignore-file = "nose.ignore.json"
```

When unset, nose automatically reads `nose.ignore.json` in the current working
directory if it exists. Pass `--ignore-file <file>` to override the config for one
run. Ignored families are hidden from the active report and from `--fail` /
`--fail-on-new`, while `--format json` still includes them with their reason,
owner, note, and expiry metadata.

The file format, selector semantics, and expiry behavior are documented in
[structured-ignores](structured-ignores.md).

## Inline suppression

To mark one specific clone as intentionally kept, put `// nose-ignore` on or
just above the unit (function/class/block). nose drops that unit from
detection, so it never shows up as a family. Use this for a duplicate you've
consciously decided to live with, rather than excluding the whole file.

For a finding that should stay visible to audit tooling, prefer a structured
ignore entry. For accepting *all* of today's existing duplication at once — so
only *new* duplication is reported — use a baseline instead; see
[continuous-integration](continuous-integration.md).
