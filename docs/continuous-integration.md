# Continuous integration

nose is built to run in CI as a duplication gate. The pieces below turn the
report from [usage](usage.md) into a pass/fail check that flags only *new* duplication
and runs fast on every push.

The gate command is now [`nose query`](usage.md#nose-query): it carries the same
`--fail-on`/`--baseline`/`--ignore-file`/`--cache-dir` workflow flags and the same
`--format sarif` output as the old `nose scan`. `nose scan` takes the same flags and
still works, but it is **deprecated (0.10.0)** in favour of `nose query`; the examples
below use the `query` spelling throughout.

## The `--fail-on any` gate

`--fail-on any` makes nose exit non-zero if any family is reported on the **default
surface** (after filters) — families held back below that surface, visible only under
`all`, never trip the gate. See [the default surface](usage.md#the-default-surface).
**A gate should pin `--mode` explicitly** rather than ride the default: the default mix
serves the report/agent surface and now includes fuzzy `near` candidates, and a pinned
mode keeps the gate's surface stable across nose upgrades. `--mode syntax` is the
closest jscpd replacement.

For a jscpd-style copy-paste gate:

```sh
nose query src --mode syntax --fail-on any
```

For a broader exact gate, pin both exact channels and keep only substantial findings:

```sh
nose query src --mode syntax,semantic --min-value 300 --min-members 3 --fail-on any
```

To include Type-3 near-duplicates in a review ratchet, add `near` and tune the fuzzy
threshold. This is usually better as a report or ratchet with `--min-value` than as a
bare "any finding fails" gate:

```sh
nose query src --mode syntax,semantic,near:0.70 --min-value 300 --min-members 3 --fail-on any
```

For an exact semantic-only gate, use `--mode semantic`. It does not use a
similarity threshold.

With committed settings in `nose.toml`, the CI command can be just `nose query src --fail-on any`.
If a wrapper needs to support multiple installed nose versions, have it query
`nose capabilities` first instead of scraping `--help`; the JSON contract is
documented in [capabilities](capabilities.md).

Use `--fail-on any` for a greenfield or low-noise gate. Use `--baseline` plus
`--fail-on new` when adopting nose on an existing codebase, so old accepted duplication stays
visible in the baseline while new or changed families fail the build.

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

`--baseline` by itself keeps the historical behavior and reports only families not
accepted by the baseline (the default whenever `--baseline` is present). Use
`--fail-on new` when you want a CI ratchet that ignores accepted debt but exits
non-zero for new or changed clone families. Plain `--fail-on any` still means "fail if
anything is reported on the default surface after the active filters."

Commit `.nose-baseline.json`. Families are keyed by their sorted reported member
locations: displayed path, language, span, unit kind, symbol name, and fragment
metadata. **The key is deliberately span- and path-sensitive**, which has three
honest consequences (measured in [experiments §CB](experiments.md)): editing lines
*above* an accepted clone re-keys it (it resurfaces as `new`/`changed`); renaming
its file re-keys it; and the key embeds the detecting channel's unit shape, so a
baseline is only valid for the `--mode` it was written under — pin the mode in CI
and re-baseline after refactors that move accepted clones. Every drift direction
is loud (the gate fires; nothing is silently hidden). New baselines also record
those member identities next to the reviewable
note, which lets later scans classify exact matches as `unchanged`, overlapping
but re-keyed families as `changed`, missing accepted families as `resolved`, and
unmatched current families as `new`. Regenerate the baseline deliberately (re-run
`--write-baseline`) when you've paid down duplication and want the lower bar
locked in — it's a ratchet.

When `--baseline` is present, the file must exist and parse as a valid baseline.
Missing or malformed baselines are hard errors; otherwise a CI ratchet could
silently compare against an empty accepted state.

To read this temporal status from JSON under `nose query`, use the `since=<baseline>`
query term: it leaves every family in place and exposes each one's `status`
(`new`/`changed`/`unchanged`) as a queryable field — so `nose query src
since=.nose-baseline.json status=new --format json` is the machine-readable "what's new"
view. (The deprecated `nose scan --format json` instead carries a top-level `baseline`
object with the counts and a per-family `baseline_status`; query-JSON v2 has neither — see
[query-json](query-json.md).)

## Structured ignores — audited suppressions

Baselines accept the current state in bulk. Structured ignores are for individual
families that were reviewed and intentionally kept. Commit `nose.ignore.json`
next to the code, or point to another file with `--ignore-file` / `ignore-file`
in [configuration](configuration.md):

```sh
nose query src --ignore-file nose.ignore.json --fail-on any
```

Ignored families are removed from the active report, so they do not fail `--fail-on any`
or `--fail-on new`. The ignore file keeps each suppression's reason, note, owner, expiry, and
selectors as the audit record. (The deprecated `nose scan --format json` also echoes the
suppressed families back under an `ignored_families` array.)

Malformed ignore files fail the run. Expired entries are reported as warnings on stderr
and are not applied (the deprecated `nose scan --format json` also lists them under
`ignore.expired`). That makes stale waivers visible instead of silently hiding
duplication. See [structured-ignores](structured-ignores.md) for the file format and
selector semantics.

## SARIF for code scanning

`--format sarif` emits SARIF 2.1.0, which GitHub code-scanning ingests to render
findings as inline PR annotations:

```sh
nose query src --format sarif top=0 > nose.sarif   # then upload via github/codeql-action/upload-sarif
```

**Pass `top=0` for a complete upload.** Every output format truncates to the row limit —
`top=N` (default 30); `top=0` means *all* (matching the deprecated `nose scan --top 0`).
Without it a repo with more than 30 families uploads only the first 30. The SARIF run records
the full count in `runs[].properties` (`total_families` / `shown_families`) and, when families
were hidden, adds a `note` notification under `runs[].invocations[]`, so a truncated upload is
at least detectable; `top=0` avoids the cap entirely.

`--format json` is the general machine-readable form for any other tooling. The forward
versioned contract is [query-json](query-json.md) (`nose query --format json`, schema v2);
the deprecated equivalent is documented in [scan-json](scan-json.md) (schema v1). Both are
truncated by their respective top limit in the same way.

## Fast re-runs: `--cache-dir`

`--cache-dir <dir>` caches each file's analysis keyed by content hash. Unchanged
files are reused on the next run — skipping parse, [normalization](normalization.md), and feature
extraction — which makes repeated invocations (CI, pre-commit, local iteration)
much faster. Point it at a directory your CI caches between runs.

```sh
nose query src --cache-dir .nose-cache --fail-on any
```

---

Contributing to nose itself? The repository's own CI — the local preflight, the duplication
ratchet, the nightly soundness corpus-verify, and review-bot policy — lives in
[CONTRIBUTING](../CONTRIBUTING.md), not here.
