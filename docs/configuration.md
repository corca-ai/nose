# Configuration

Real projects shouldn't carry 200-character command lines. Put a `nose.toml`
(or `.nose.toml`) in the directory where you invoke nose and it is read
automatically. The config supplies defaults for supported scan settings; most CLI flags
override those defaults, while `exclude` globs are additive. Anything unset falls back to
the built-in default.

## `nose.toml`

```toml
[scan]
exclude     = ["tests/**", "**/*.generated.ts", "vendor/**"]
mode        = ["syntax", "semantic"]
sort        = "extractability"
min-value   = 200
min-members = 3
min-size    = 30
top         = 50
ignore-file = "nose.ignore.json"
```

Pass an alternate file with `--config <file>`. A malformed config is a **hard
error** — a silently-ignored typo'd setting would be worse than a crash.

Put stable project policy in `nose.toml`: excludes, scan modes, ranking, size/value
thresholds, report limits, and the structured-ignore file. Keep one-off workflow choices on
the command line: output format, `--show` views, baselines, cache location, and CI failure
mode.

### Keys

All keys are optional; an absent key means "no opinion — use the CLI value or
the built-in default". Keys are kebab-case and live under the `[scan]` table.

| key | type | default | same as flag |
|---|---|---|---|
| `exclude` | list of globs | `[]` | `--exclude` |
| `mode` | list of `syntax`\|`semantic`\|`near[:T]` | `["syntax", "semantic"]` | `--mode` |
| `sort` | `extractability`\|`value`\|`sites`\|`hazard` | `extractability` | `--sort` |
| `min-value` | float | `0.0` | `--min-value` |
| `min-members` | int | `2` | `--min-members` |
| `min-size` | int (IL tokens) | `24` | `--min-size` |
| `min-lines` | int (advanced) | `5` | `--min-lines` |
| `top` | int | `30` | `--top` |
| `ignore-file` | string path | auto-read `nose.ignore.json` when present | `--ignore-file` |

`mode` is a TOML array, even for one channel:

```toml
[scan]
mode = ["syntax"]                  # jscpd-style gate
# mode = ["syntax", "semantic"]    # same as omitting mode
# mode = ["syntax", "semantic", "near"]
```

`min-size` (and the advanced `min-lines`) apply to both structural units and the syntax
copy-paste floor. For `--mode syntax`, they are the jscpd-style size gate.

The `near` channel's acceptance threshold rides on the `mode` value itself —
`mode = ["syntax", "semantic", "near:0.8"]` (or `--mode near:0.8`), default `0.70`.
There is no separate threshold setting, so it can never be mis-applied to the exact
`syntax`/`semantic` channels.

The hidden experimental `abstraction[:T]` mode is also accepted in `mode`, but it is
not a stable project-policy surface and is intentionally absent from
[capabilities](capabilities.md)' stable mode list. Prefer it for local research or
tooling experiments, not CI gates.

```sh
nose scan src --mode syntax,semantic,near:0.70
```

## Excludes

`exclude` is **additive**: the config's globs and any `--exclude` flags on the
command line are combined. Globs use gitignore syntax (`tests/**`,
`**/*.test.ts`, `vendor/**`) and are applied *during the directory walk*, so an
excluded directory is pruned, not just filtered out afterward.

`.gitignore` files inside each scanned tree are respected automatically, even when that
tree is not a git checkout, so vendored dependencies, build output, and the like are
skipped without any configuration. Parent ignore files above the scanned root are not
applied; pointing nose at an ignored subdirectory intentionally still scans it.

## Structured ignores

`ignore-file` points to a structured suppression file for reviewed findings:

```toml
[scan]
ignore-file = "nose.ignore.json"
```

When unset, nose automatically reads `nose.ignore.json` in the current working
directory if it exists. Pass `--ignore-file <file>` to override the config for one
run. Ignored families are hidden from the active report and from `--fail-on any` /
`--fail-on new`, while `--format json` still includes them with their reason,
owner, note, and expiry metadata.

The file format, selector semantics, and expiry behavior are documented in
[structured-ignores](structured-ignores.md).

## Inline suppression

To mark one site as intentionally kept, put a `nose-ignore` marker in a comment on the
unit's first line or immediately above it (`# nose-ignore`, `// nose-ignore`, and similar
comment syntax all work). nose drops that unit from detection, so that site cannot form a
family. Use this for a duplicate you've consciously decided to live with, rather than
excluding the whole file.

For a finding that should stay visible to audit tooling, prefer a structured
ignore entry. For accepting *all* of today's existing duplication at once — so
only *new* duplication is reported — use a baseline instead; see
[continuous-integration](continuous-integration.md).
