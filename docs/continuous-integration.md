# Continuous integration

nose is built to run in CI as a duplication gate. The pieces below turn the
report from [usage](usage.md) into a pass/fail check that flags new or changed duplication
and runs fast on every push.

The gate command is [`nose query`](usage.md#nose-query): it carries
`--fail-on`/`--baseline`/`--ignore-file`/`--cache-dir` workflow flags and
`--format sarif` output.

## The `--fail-on any` gate

`--fail-on any` makes nose exit non-zero if any family is reported on the **default
surface** (after filters) — families held back below that surface, visible only under
`all`, never trip the gate. See [the default surface](usage.md#the-default-surface).
**A gate should pin `--mode` explicitly** rather than ride the default: the default mix
serves the report/agent surface and now includes fuzzy `near` candidates, and a pinned
mode keeps the gate's surface stable across nose upgrades. `--mode syntax` is the
closest jscpd replacement.

## Jscpd-style size budgets

For a jscpd-style copy-paste gate, run only the syntax channel and decide which family size
crosses the project's budget. The gate fires on the family selection left after the query
terms; `top=N` only truncates display, not the gate.

```sh
nose query src --mode syntax --min-size 80 'lines>25' --fail-on any
nose query src --mode syntax --min-size 80 'shared>20' --fail-on any
nose query src --mode syntax --min-size 80 'dup>80' --fail-on any
```

The knobs are intentionally explicit:

- `--min-size N` is the minimum duplicated IL-token run; in `--mode syntax` it is the
  copy-paste run floor.
- `lines>N` keeps families whose mean per-copy span is larger than `N` source lines.
- `shared>N` keeps families with more than `N` invariant lines across the copies.
- `dup>N` keeps families whose duplicated-line volume is above `N`; this is the closest
  family-level stand-in for "how much repeated code did this introduce?"

Quote comparison terms in shell examples (`'dup>80'`) because bare `>` is a redirection
operator.

`dup>N` is usually the best first CI policy because it accounts for both copy size and
copy count. For an existing codebase, ratchet from the current state instead of failing on
all historical duplication:

```sh
nose query src --mode syntax --min-size 80 'dup>80' \
  --baseline .nose-baseline.json --write-baseline
nose query src --mode syntax --min-size 80 'dup>80' \
  --baseline .nose-baseline.json --fail-on new
```

Use `--fail-on any` for a greenfield or low-noise gate. Use `--baseline` plus
`--fail-on new` when adopting nose on an existing codebase, so old accepted duplication stays
visible in the baseline while new or changed families fail the build.

## Broader gates

For a broader exact gate, pin both exact channels and keep only substantial findings:

```sh
nose query src --mode syntax,semantic --min-value 300 --min-members 3 --fail-on any
```

To include Type-3 near-duplicates, make that an explicit audit or ratchet opt-in:
add `near` and tune the fuzzy threshold. This is usually better as a report or ratchet
with `--min-value` than as a bare "any finding fails" gate:

```sh
nose query src --mode syntax,semantic,near:0.70 --min-value 300 --min-members 3 --fail-on any
```

For an exact semantic-only gate, use `--mode semantic`. It does not use a
similarity threshold.

With committed settings in `nose.toml`, the CI command can omit stable analysis flags such as
`--mode`, `--min-size`, and `--exclude`; query terms such as `'dup>80'` stay on the command
line:

```sh
nose query src 'dup>80' --fail-on any
```

If a wrapper needs to support multiple installed nose versions, have it query
`nose capabilities` first instead of scraping `--help`; the JSON contract is
documented in [capabilities](capabilities.md).

## Baselines — incremental adoption

An existing codebase already has dozens of clone families, so a bare `--fail-on any`
gate is unusable on day one. A **baseline** records the currently-accepted
families; subsequent runs compare the current report to that accepted state, so
the gate can flag only duplication introduced *after* adoption.

```sh
# 1. Accept today's state (writes the baseline file and exits):
nose query src --baseline .nose-baseline.json --write-baseline

# 2. From now on, show only NEW or CHANGED families:
nose query src --baseline .nose-baseline.json

# 3. Make CI fail only when NEW or CHANGED families exist:
nose query src --baseline .nose-baseline.json --fail-on new
```

Real gates should repeat the same pinned `--mode`, size flags, and query terms in the
write-baseline and fail-on-new commands; the jscpd-style example above shows that full form.

`--baseline` by itself keeps the historical behavior and reports only families not
accepted by the baseline (the default whenever `--baseline` is present). Use
`--fail-on new` when you want a CI ratchet that ignores accepted debt but exits
non-zero for new or changed clone families. Plain `--fail-on any` still means "fail if
anything is reported on the default surface after the active filters."

Commit `.nose-baseline.json`. A baseline is an accepted set of duplicated
members, not just a list of family ids. Each accepted member records its exact
member identity and a source digest next to an auditable note. Later runs hide a
current family only when every current member is already accepted with the same
digest. That means a family can reshape — for example, a three-copy accepted
family becomes an accepted two-copy family — without firing the gate, while an
edited member is reported again as `changed`.

The family id is still the `id=` handle and remains span- and path-sensitive (see
[structured-ignores › Family IDs](structured-ignores.md#family-ids)), but the
baseline decision is digest-backed: exact accepted members are `unchanged`,
accepted-plus-new members are `changed`, missing accepted families are
`resolved`, and unmatched current families are `new`. Baselines are valid for
the detection mode they were written under, so pin `--mode` in CI and regenerate
the baseline deliberately (re-run `--write-baseline`) when you've paid down
duplication and want the lower bar locked in — it's a ratchet.

When `--baseline` is present, the file must exist and parse as a valid baseline.
Missing or malformed baselines are hard errors; otherwise a CI ratchet could
silently compare against an empty accepted state.

To read this temporal status from JSON under `nose query`, use the `since=<baseline>`
query term: it leaves every family in place and exposes each one's `status`
(`new`/`changed`/`unchanged`) as a queryable field — so `nose query src
since=.nose-baseline.json status!=unchanged --format json` is the machine-readable
"what changed since the accepted snapshot" view. See [query-json](query-json.md).

## Structured ignores — audited suppressions

Baselines accept the current state in bulk. Structured ignores are for individual
families that were accepted and intentionally kept. Commit `nose.ignore.json`
next to the code, or point to another file with `--ignore-file` / `ignore-file`
in [configuration](configuration.md):

```sh
nose query src --ignore-file nose.ignore.json --fail-on any
```

Ignored families are removed from the active report, so they do not fail `--fail-on any`
or `--fail-on new`.

Malformed ignore files fail the run. Expired entries are reported as warnings on stderr
and are not applied. That makes stale waivers visible instead of silently hiding
duplication. See [structured-ignores](structured-ignores.md) for the file format and
selector semantics.

## SARIF for code scanning

`--format sarif` emits SARIF 2.1.0, which GitHub code-scanning ingests to render
findings as inline PR annotations:

```sh
nose query src --mode syntax --format sarif top=0 > nose.sarif   # then upload via github/codeql-action/upload-sarif
```

For GitHub Actions, upload third-party SARIF with
`github/codeql-action/upload-sarif@v4`. The job needs `security-events: write`,
`contents: read`, and, for private repositories, `actions: read`; private and internal
repositories also need GitHub Code Security enabled. Public fork pull requests usually do
not receive a token that can write code-scanning results, so PR workflows should not use
`pull_request_target` as a workaround for untrusted code.

**Pass `top=0` for a complete upload.** Every output format truncates to the row limit —
`top=N` (default 30); `top=0` means *all*.
Without it a repo with more than 30 families uploads only the first 30. The SARIF run records
the full count in `runs[].properties` (`total_families` / `shown_families`) and, when families
were hidden, adds a `note` notification under `runs[].invocations[]`, so a truncated upload is
at least detectable; `top=0` avoids the cap entirely.

`--format json` is the general machine-readable form for any other tooling. The forward
versioned contract is [query-json](query-json.md) (`nose query --format json`; schema v8
for `base=<ref>`, schema v9 for the other query views).
It is truncated by the active top limit in the same way.

## Divergent-edit v2 gate tiers

The divergent-edit gate (`nose query . base=<ref>`) is an opt-in PR review gate.
It uses an explicit tiered contract so CI wrappers can distinguish default-failing
items from review-only context without re-running nose internals:

```sh
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --fail-on any
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --format sarif top=0 > nose-divergence.sarif
```

Wrappers should preflight the installed binary with `nose capabilities` before
running this gate. Require `schemas.query_json` to contain `8`,
`query.output_formats` to contain `sarif` when uploading SARIF, and these
query capability flags to be true: `base_divergence`, `query_base_json_v8`,
`query_base_gate_fail_default`, `query_base_sarif`, `structured_ignores`, and
`query_base_structured_ignores`. Reject older binaries instead of inferring
support from `nose --help` or from the package version alone.

The `base=` default is already the conservative `syntax,semantic` mix, but pinning
`--mode` keeps CI diffs explicit across upgrades. `top=0` should be used for SARIF
uploads; otherwise only the active row limit is emitted, with a truncation note in
the SARIF invocation.

| tier | default CI effect | SARIF rule id | SARIF level |
|---|---|---|---|
| `strict` | fails only when `properties.gate.fail_default == true` | `nose.divergent.strict` | `error` |
| `review` | visible, non-failing by default | `nose.divergent.review` | `warning` |
| `report-only` | visible advisory, never default-failing | `nose.divergent.report-only` | `note` |
| `suppressed` | omitted from active output and never failing | not emitted in normal SARIF | none |

For v2 SARIF, each result's rule id and `properties.tier` agree. Results also carry
`properties.tier_reasons`, `properties.taxonomy_hint`, `properties.gate`,
`properties.policy`, `properties.lane`, `properties.family_id`, and optional
`properties.base_family_id`. `properties.gate.fail_default` is the authoritative default
CI decision: it is true only for unsuppressed `strict` results. Normal SARIF omits
suppressed results; a future suppressed/debug SARIF surface must emit
`properties.tier="suppressed"` and `properties.suppression` with the structured-ignore
metadata.

The legacy `fire_eligible` field remains in JSON as the serialized v1 conservative
verdict. In the current implementation it is computed from proven shared-logic touch on
at least one direct `targets[]` edge and not-all-test scope, so mixed-scope findings may
still be `fire_eligible=true`.
Wrappers should display `tier`, but decide pass/fail only from `gate.fail_default`.
They should not reconstruct gate behavior from raw fields such as `touches_shared`,
`scope`, `witness_kind`, or `graded` when a `tier` is present.

JSON and SARIF keep one finding/result per family and carry the same `targets[]` target IDs.
SARIF target-backed primary and related locations repeat `target_id` in location properties,
so an annotation identifies the exact skipped sibling and changed source without promoting a
bridge member reached only through transitive family closure.
Each target also mirrors the same closed `variant_evidence`: strong pair evidence is separated
from weak name/path/version hints and explicit caveats. It is development evidence in v2, not
another CI authority; wrappers must still decide only from `gate.fail_default`.

Structured ignores apply before the gate: a suppressed divergent-edit family must not
produce a `strict` failure, and report-only lanes such as newly added clone evidence
must not fail default CI. Newly added clone evidence appears as `lane="new-copy"`,
`tier="report-only"`, `base_family_id=null`, and current-tree `current_only[]` sites;
`properties.gate.fail_default` remains `false` in SARIF.

Checked closeout evidence supports opt-in enforcement, not default-on blocking. The
v2 replay records strict precision 0.562 while retaining 45/45 confirmed v1
missed-propagation positives. The #847 precision-first cycle added target-local semantic,
direct-edge, and variant evidence, but no admissible v3 policy had development support;
the blind population therefore remains sealed and the final official-v0.19.0 comparison
also misses the 5% runtime budget at 8.74% control-adjusted
([results](../eval/divergence_fire/RESULTS.md), [#854 closeout](divergent-gate-closeout-854.md)).
The [CI examples](examples/ci/divergent-edit-observe-only.yml) and
[enforcing workflow](examples/ci/divergent-edit-enforcing.yml) show the recommended
observe-only-to-enforcing rollout, the [#687 pilot](divergent-history-mining-pilot-687.md)
keeps history mining offline, and the [#688 evidence](divergent-gate-product-runtime-688.md)
records non-`base=` product-output stability plus runtime checks.

When a strict divergent-edit finding is accepted as intentional, commit a structured
ignore with a reason/owner/expiry instead of teaching the wrapper to reinterpret the
finding. The `base=` view auto-reads `nose.ignore.json`, accepts `--ignore-file`, and
honors `[query] ignore-file = "..."` from `nose.toml`; path and language selectors must
cover every member of the reported family before they suppress it.

Base-divergence SARIF locations point at the skipped sibling, where a propagated edit
may be missing; changed copies are attached as related locations. `new-copy` report-only
SARIF locations point at the current-tree added/copied/renamed copy and link its clone
siblings as related locations.

### GitHub Actions rollout examples

Copy one of these PR workflows into `.github/workflows/` and pin `NOSE_VERSION` to a
reviewed release:

- [docs/examples/ci/divergent-edit-observe-only.yml](examples/ci/divergent-edit-observe-only.yml) writes JSON/SARIF, uploads SARIF when the token can do so, and adds a step summary without failing on findings.
- [docs/examples/ci/divergent-edit-enforcing.yml](examples/ci/divergent-edit-enforcing.yml) does the same, then fails only when `items[].gate.fail_default == true`.

Both examples use `on: pull_request`, `actions/checkout@v7` with `fetch-depth: 0`,
`persist-credentials: false`, a `nose capabilities` preflight, an explicit fetch and
`git rev-parse --verify --quiet "${BASE_REF}^{commit}"` base-ref check, pinned
`--mode syntax,semantic`, and complete JSON/SARIF output with `top=0`.

The enforcing workflow uploads SARIF before its final failing step so code-scanning
results are available when GitHub accepts the upload. It still treats the step summary as
the reliable PR surface: GitHub only displays inline code-scanning annotations for alerts
whose locations are in the pull request diff, while a divergent-edit SARIF result often
anchors on the skipped sibling. Fork PRs may skip SARIF upload because the token is
read-only; do not switch to `pull_request_target` for untrusted PR code.

The examples intentionally run JSON and SARIF without `--fail-on any` and make the final
decision themselves from `gate.fail_default`. That preserves upload-before-fail ordering
and avoids accidental failures from `fire_eligible`, SARIF severity, human output,
`summary.strict`, `scope`, `touches_shared`, or `tier` alone.

### Maintainer observe-only pilot

For an adoption pilot, start with the observe-only workflow as a non-required PR
check. Record the number of scanned PRs, PRs with findings, findings reviewed,
strict/default-failing findings, report-only findings, structured ignores added,
and maintainer disposition buckets such as `should-propagate`,
`intentional-variant`, `no-propagation-needed`, `test-scaffolding`, and
`unclear`.

Keep the pilot language narrow: a strict finding "would fail under the opt-in
enforcing workflow"; it is not a default-on readiness claim. History mining is a
separate offline maintainer tool and should not be added to PR-time CI. See the
checked #687 nose-on-nose [divergent history pilot](divergent-history-mining-pilot-687.md)
for the recorded artifact and disposition shape.

## Fast re-runs: `--cache-dir`

`--cache-dir <dir>` caches each file's analysis in the
[portable layered CAS](portable-cache-artifacts.md), keyed by a SHA-256 digest of its complete
post-resolution IL/reporting identity and unit-affecting options. Entries have stage/schema
identity plus an independent payload checksum; corrupt or truncated bytes recompute. Unchanged
files reuse [normalization](normalization.md), feature extraction, and syntax streams on the next
run, including when the same relative source is checked out below another absolute root.

The active cache layer still rediscovers, reads, parses, and lowers the whole selected corpus,
resolves cross-file facts, and repeats global detection and presentation. Raw/resolved portable
formats are ready for dependency-aware activation in #874, but #873 does not claim those stages
are skipped. Point the directory at storage your CI preserves between runs; see the
[incremental cache benchmark](incremental-cache-benchmark.md) for the exact performance and
clean-scan-equivalence contract.

```sh
nose query src --cache-dir .nose-cache --fail-on any
```

---

Contributing to nose itself? The repository's own CI — the local preflight, the duplication
ratchet and the nightly soundness corpus-verify policy — lives in
[contributing](contributing.md), not here.
