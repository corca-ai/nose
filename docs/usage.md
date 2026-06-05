# Usage

The complete command and flag reference for `nose`. New here? Start with
[getting-started](getting-started.md) — it walks through a first scan and how to
read the report. For settings you'd commit to a repo see
[configuration](configuration.md); for CI use see
[continuous-integration](continuous-integration.md). Back to [home](home.md).

## Install

The quickest install (Homebrew or the install script) is in
[getting-started](getting-started.md#install). Prebuilt binaries for macOS (Apple
Silicon + Intel) and Linux (x86_64 + arm64) are attached to every
[release](https://github.com/corca-ai/nose/releases). To build from source:

```sh
cargo build --release
# binary at ./target/release/nose
```

nose is a single self-contained binary — no runtime, services, or network; point
it at source files. The examples below write `nose`, which works the same as a
from-source `./target/release/nose`.

## `nose scan`

`nose scan <paths…>` scans one or more files/directories (recursively, respecting
`.gitignore`), groups duplicated code into **families**, and ranks them by
**extractability** — how cleanly each family folds into one shared helper — so the
duplication you can actually act on surfaces first. With no `--mode`, it runs
`syntax,semantic`: CPD-style syntax runs plus exact semantic Type-4 clones.

```sh
nose scan path/to/project
```

How to read the resulting report — the `scanned …` scope line, the per-family
breakdown, scope tags, and the `→` hint — is covered in
[getting-started → How to read the report](getting-started.md#how-to-read-the-report).

### Flags

Grouped by what they do. Anything here can also be set in
[configuration](configuration.md).

**Filter & shape the report**

| flag | effect |
|---|---|
| `--sort KEY` | ranking: `extractability` (default), `value`, or `sites` (see [Ranking](#ranking)) |
| `--top N` | show only the top N families (default 30; `--top 0` = all) |
| `--min-members N` | only families with at least N duplicated sites (default 2) |
| `--min-value V` | hide families below this refactoring value (noise floor on large repos) |
| `--min-tokens N` | ignore units or syntax copy-paste runs smaller than this IL-token size (default 24) |
| `--min-lines N` | ignore units or syntax copy-paste runs spanning fewer source lines (default 5) |
| `--mode MODE` | one or more of `syntax`, `semantic`, `near`; comma-list or repeatable; when present, replaces the default |
| `--threshold T` | acceptance similarity in `[0,1]` for the `near` channel; invalid unless `--mode` includes `near` |
| `--exclude <glob>` | skip paths matching a gitignore-syntax glob (repeatable) |
| `--ignore-file <file>` | suppress reviewed families using a structured ignore file with reason/owner/expiry metadata |

### Ranking

`--sort` chooses what "most worth your attention first" means. `--min-value` is a
noise floor on raw value and applies under every sort.

| key | ranks by | use when |
|---|---|---|
| `extractability` *(default)* | invariant (shared) lines × copies × spread, weighted by tightness (shared/total) and penalized by parameter count | you want the duplication that folds *cleanly* into one helper — not the biggest block that merely looks similar |
| `value` | raw duplicated volume: removable lines × similarity × spread | you want the most *code* deleted, accepting that divergent copies cost more to merge |
| `sites` | number of copies | hunting the most-repeated patterns |

Extractability is the default because raw volume over-rewards a large block whose
copies share little: a 384-line family that shares only 22 lines across 14 varying
spots is mostly scaffolding (6% invariant), not an extraction — it ranks far below a
tight `15/15`-shared, zero-parameter pair. The honest `N/M shared · Pp` cell in the
report is the same signal the ranking uses. Same-language families with **no** shared
invariant lines (a language idiom, or two unrelated type literals with the same shape)
have nothing to extract and sink to the bottom, even at `sim 1.00`.

Extractability ranks by *fixability* (how cleanly duplication folds into one helper).
A complementary *severity* axis — ranking by how likely a clone is to be edited
inconsistently and cause a bug — is designed in [hazard-ranking](hazard-ranking.md)
(not yet shipped).

**Review what was found**

| flag | effect |
|---|---|
| `--diff` | show each family inline as a unified diff between its two representative copies — both versions and exactly what differs |
| `--proposal` | show an extraction skeleton per family — the shared structure with the differing parts marked as parameters |
| `--hotspots` | after the report, rank directories/modules by total duplicated lines (architecture view) |
| `--format human\|json\|markdown\|sarif` | output format (default `human`) |

`--format json` emits a versioned object with `schema_version`, `tool_version`,
scan scope, ranking metadata, and a `families` array. The stable contract and
compatibility rule are documented in [scan-json](scan-json.md).

**Workflow** (`--baseline`, `--write-baseline`, `--new-only`, `--fail-on-new`,
`--fail`, `--ignore-file`, `--cache-dir`, `--config`) is covered in
[continuous-integration](continuous-integration.md) and [configuration](configuration.md).
Structured suppressions are covered in [structured-ignores](structured-ignores.md).

### Scan modes

`nose scan` has three orthogonal channels. Omitting `--mode` runs `syntax,semantic`.
When `--mode` is present, nose runs exactly the channels you list; it does not add
them to the default. See [clone-types](clone-types.md) for what each finds against
the standard Type-1–4 taxonomy.

| mode | clone type | detector channel | use when |
|---|---|---|---|
| `syntax` | Type-1/2 floor | jscpd-style contiguous copy-paste runs | replacing jscpd / blocking copy-paste clones in CI |
| `semantic` | Type-4 | exact value-fingerprint matches | high-confidence semantic clones with no fuzzy threshold |
| `near` | Type-3 | shape candidates + fuzzy structural/value scoring | finding near-duplicates for review |

Examples:

```sh
nose scan src                                  # syntax + semantic
nose scan src --mode syntax --fail             # jscpd-style gate
nose scan src --mode semantic                  # exact Type-4 only
nose scan src --mode near --threshold 0.70     # Type-3 only
nose scan src --mode syntax,semantic,near      # all channels
nose scan src --mode syntax --mode semantic    # same as --mode syntax,semantic
```

Only `near` uses `--threshold` (default `0.70`). `syntax` and `semantic` are exact
channels, so `--threshold` is rejected unless `near` is also enabled.

The `syntax` channel is the CPD floor: it finds duplicated token runs even when they
start or end in the middle of a function. The normalized unit channels (`semantic` and
`near`) are where renamed identifiers, cross-language convergence, and Type-3 edits are
handled.

## Integrating with nose

Use `nose capabilities` when another tool needs to decide what this installed
binary supports before it invokes a scan:

```sh
nose capabilities
```

The command emits JSON only. It reports the tool version, platform, stable
commands, supported scan modes and output formats, JSON schema versions, config
keys, and scan capability flags such as baselines, caching, SARIF, and
structured ignores. Do not scrape `nose --help`; help text is for humans and may
change to improve readability. The stable contract is documented in
[capabilities](capabilities.md).

## Other commands

- `nose stats <paths…> [--top N] [--json]` — per-language IL lowering coverage (the
  Raw-node ratio), with the top unhandled surface kinds (`--top`, default 30; `--json`
  for machine output). Use it to spot a language/construct that isn't lowering well; see
  [languages](languages.md).
- `nose il <file> [--normalized] [--no-cfg-norm] [--format sexpr|json]` — dump the IL
  for one file (`--normalized` shows the canonical form after the
  [normalization](normalization.md) passes). A debugging tool for understanding why two
  snippets do or don't converge; see [architecture](architecture.md).

A `detect` command (raw clone pairs/groups) and `eval` / `ceiling` (scoring
predictions against a gold set) also exist as the strict/research surface. They are
hidden from `--help` because `scan` is the command for everyday use; the
[benchmark](benchmark.md) page documents them.
