# Configuration

Real projects shouldn't carry 200-character command lines. Put a `nose.toml`
(or `.nose.toml`) in the directory where you invoke nose and it is read
automatically. The config supplies defaults for supported query settings; most CLI flags
override those defaults, while `exclude` and `generated-paths` globs are additive. Anything
unset falls back to the built-in default.

## `nose.toml`

```toml
[query]
exclude     = ["tests/**", "**/*.generated.ts", "vendor/**"]
generated-paths = ["generated/**", "**/snapshots/mypy/**"]
mode        = ["syntax", "semantic"]
sort        = "extractability"
min-value   = 200
min-members = 3
min-size    = 30
ignore-file = "nose.ignore.json"
semantic-packs = ["semantic-packs/python-math-prod.json"]
# Or, for content-pinned typed v1 authorization instead of unlocked paths:
# semantic-pack-lock = "nose.semantic-pack-lock.json"
cache-max-bytes = 5368709120 # 5 GiB; used when --cache-dir is present
```

Pass an alternate file with `--config <file>`. A malformed config is a **hard
error** — a silently-ignored typo'd setting would be worse than a crash.

Put stable project policy in `nose.toml`: excludes, generated-artifact assertions, detection
modes, ranking, size/value thresholds, the structured-ignore file, and explicit local
semantic-pack opt-ins. Keep one-off workflow choices on the command line or in query terms:
output format, the drill/view terms (`id=`, `group=`, `full`), baselines, cache location, and
CI failure mode.

### Keys

All keys are optional; an absent key means "no opinion — use the CLI value or
the built-in default". Keys are kebab-case and live under the `[query]` table.

The **CLI override** column gives the per-run flag (or, where they differ, the `nose query`
term — `nose query` spells `sort` as the DSL term `sort=`, not `--sort`).

| key | type | default | CLI override |
|---|---|---|---|
| `cache-max-bytes` | int (bytes) | `5368709120` (5 GiB) | `--cache-max-bytes` |
| `exclude` | list of globs | `[]` | `--exclude` |
| `generated-paths` | list of root-anchored globs | `[]` | `--generated-path` |
| `mode` | list of `syntax`\|`semantic`\|`near[:T]` | `["syntax", "semantic", "near"]` | `--mode` |
| `sort` | `extractability`\|`value`\|`sites`\|`hazard` | `extractability` | `sort=` (query term) |
| `min-value` | finite non-negative float | `0.0` | `--min-value` |
| `min-members` | int | `2` | `--min-members` |
| `min-size` | int (IL tokens) | `24` | `--min-size` |
| `min-lines` | int (advanced) | `5` | `--min-lines` |
| `ignore-file` | string path | auto-read `nose.ignore.json` when present | `--ignore-file` |
| `semantic-packs` | list of file or directory paths | `[]` | `--semantic-pack` |
| `semantic-pack-lock` | project-lock file path | unset | `--semantic-pack-lock` |

`mode` is a TOML array, even for one channel:

```toml
[query]
mode = ["syntax"]                          # jscpd-style gate (exact copy-paste only)
# mode = ["syntax", "semantic"]            # exact channels only, no fuzzy near
# mode = ["syntax", "semantic", "near"]    # same as omitting mode (the default)
```

`cache-max-bytes` bounds nose-managed files below `--cache-dir`. It does not choose or enable a
cache directory; keep the location as a CLI/CI workflow choice. A run that writes cache data
automatically prunes old schemas, superseded generations, orphaned state, and then the oldest
artifacts until it reaches the limit. Eviction changes only future run time, never query output.
See [faster repeated queries](query-cache.md) for when to enable a cache and how to inspect or
clean it.

`min-size` (and the advanced `min-lines`) apply to both structural units and the syntax
copy-paste floor. For `--mode syntax`, they are the jscpd-style size gate.

The `near` channel's acceptance threshold rides on the `mode` value itself —
`mode = ["syntax", "semantic", "near:0.8"]` (or `--mode near:0.8`), default `0.70`.
There is no separate threshold setting, so it can never be mis-applied to the exact
`syntax`/`semantic` channels.

```sh
nose query src --mode syntax,semantic,near:0.70
```

The hidden experimental `abstraction[:T]` mode is also accepted in `mode`, but it is
not a stable project-policy surface and is intentionally absent from
[capabilities](capabilities.md)' stable mode list. Prefer it for local research or
tooling experiments, not CI gates. If `near:T` and `abstraction:T` appear together,
they must name the same threshold because both modes share one fuzzy acceptance
cutoff.

Config file paths are resolved from the config file's directory, so committed
project paths do not depend on where `nose` was invoked. This applies to
`ignore-file`, `semantic-packs`, and `semantic-pack-lock`. CLI path flags such
as `--ignore-file`, `--semantic-pack`, and `--semantic-pack-lock` remain
current-working-directory relative. `generated-paths` entries are globs rather than config
file paths: they are always anchored to each analyzed root.

`semantic-packs` is additive with repeated `--semantic-pack` flags. Each entry is
an explicit local opt-in to a semantic-pack v0 or v1 manifest file, or a
directory of direct `*.json` manifests. Unlocked packs are metadata-only: nose
validates and reports them but they do not change analysis results.

`semantic-pack-lock` selects one content-pinned
[`nose.semantic-pack-lock.v1`](semantic-pack-project-lock.md) file. It is
the reviewed way to let eligible v1 rows influence results. It is mutually
exclusive with `semantic-packs` and `--semantic-pack`: the lock owns the complete
manifest set and cannot be extended by an unlocked path.
A missing, stale, altered, incompatible, conflicting, or path-escaped lock is a
hard error before analysis. The CLI `--semantic-pack-lock` value overrides a
configured lock, while the same no-mixing rule remains in force.

See the [semantic-pack overview](semantic-packs.md) before configuring either
option. Pack-author details are in the [loading and trust policy](semantic-pack-loading.md).

## Excludes

`exclude` is **additive**: the config's globs and any `--exclude` flags on the
command line are combined. Globs use gitignore syntax (`tests/**`,
`**/*.test.ts`, `vendor/**`) and are applied *during the directory walk*, so an
excluded directory is pruned, not just filtered out afterward. Invalid exclude
globs are hard errors; silently analyzing a path the user meant to exclude is
worse than failing early.

`.gitignore` files inside each analyzed tree are respected automatically, even when that
tree is not a git checkout, so vendored dependencies, build output, and the like are
skipped without any configuration. Parent ignore files above the analyzed root are not
applied; pointing nose at an ignored subdirectory intentionally still analyzes it.

## Generated paths

`generated-paths` is additive: config assertions and repeated `--generated-path` flags are
combined. Unlike `exclude`, these patterns do not prune files or delete findings. A family
moves to `surface=generated` only when every member is covered by caller or nose provenance;
recover it with `all top=0` or `surface=generated`.

Patterns use positive gitignore syntax and are automatically anchored to each query root.
Use `generated/**` for an immediate child or `**/generated/**` at any depth. Empty,
negated, absolute, parent-relative, and backslash patterns are hard errors. Matching first
checks canonical root containment: an explicitly supplied symlink root works, while a symlink
below a root cannot mark a file outside that root. Missing, unreadable, out-of-root, and mixed
families fail open. See [caller-provided generated paths](caller-generated-path-provenance.md)
for the trust, JSON, multi-root, and portability contract.

## Structured ignores

`ignore-file` points to a structured suppression file for accepted findings:

```toml
[query]
ignore-file = "nose.ignore.json"
```

When unset, nose automatically reads `nose.ignore.json` in the current working
directory if it exists. Pass `--ignore-file <file>` to override the config for one
run. Ignored families are hidden from the active report and from `--fail-on any` /
`--fail-on new`.

The file format, selector semantics, and expiry behavior are documented in [structured-ignores](structured-ignores.md).

## Inline suppression

To mark one site as intentionally kept, put a `nose-ignore` marker in a comment on the
unit's first line or immediately above it (`# nose-ignore`, `// nose-ignore`, and similar
comment syntax all work). nose drops that unit from detection, so that site cannot form a
family. Use this for a duplicate you've consciously decided to live with, rather than
excluding the whole file.

For when to reach for this inline marker vs a structured ignore entry vs a
baseline, see [Which suppression to use](structured-ignores.md#which-suppression-to-use).
Baselines are set up in [continuous-integration](continuous-integration.md).
