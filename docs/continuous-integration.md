# Continuous integration

nose is built to run in CI as a duplication gate. The pieces below turn the
report from [usage](usage.md) into a pass/fail check that flags only *new* duplication
and runs fast on every push. Back to [home](home.md).

## The `--fail-on any` gate

`--fail-on any` makes nose exit non-zero if **any** family survives the filters. Pick the
channels deliberately: `--mode syntax` is the closest jscpd replacement, while the
default also reports exact semantic Type-4 clones.

For a jscpd-style copy-paste gate:

```sh
nose scan src --mode syntax --fail-on any
```

For a broader exact gate, omit `--mode` and keep only substantial findings:

```sh
nose scan src --min-value 300 --min-members 3 --fail-on any
```

To include Type-3 near-duplicates in a review ratchet, add `near` and tune the fuzzy
threshold. This is usually better as a report or ratchet with `--min-value` than as a
bare "any finding fails" gate:

```sh
nose scan src --mode syntax,semantic,near --threshold 0.70 --min-value 300 --min-members 3 --fail-on any
```

For an exact semantic-only gate, use `--mode semantic`. It does not use a
similarity threshold.

With committed settings in `nose.toml`, the CI command can be just `nose scan src --fail-on any`.
If a wrapper needs to support multiple installed nose versions, have it query
`nose capabilities` first instead of scraping `--help`; the JSON contract is
documented in [capabilities](capabilities.md).

## Local CI mirror

For nose itself, use the repository scripts before opening or updating a PR:

```sh
./scripts/check-ci-local.sh --fast
```

The fast gate runs rustfmt, clippy with `-D warnings`, the `nose-cli` test suite,
and the docs wiki lint. It is also wired into `.githooks/pre-push` when hooks are
enabled with:

```sh
git config core.hooksPath .githooks
```

Before merge or release-sensitive work, run the full local CI mirror:

```sh
./scripts/check-ci-local.sh --full
# same as:
./scripts/check.sh
```

The full gate mirrors the GitHub Actions jobs: format, clippy, rustdoc warnings,
release build/tests, the self-hosted duplication gate, MSRV check, supply-chain
checks, docs wiki connectivity, and Lean soundness proofs.

## Baselines — incremental adoption

An existing codebase already has dozens of clone families, so a bare `--fail-on any`
gate is unusable on day one. A **baseline** records the currently-accepted
families; subsequent runs compare the current report to that accepted state, so
the gate can flag only duplication introduced *after* adoption.

```sh
# 1. Accept today's state (writes the baseline file and exits):
nose scan src --baseline .nose-baseline.json --write-baseline

# 2. From now on, show only NEW or CHANGED families:
nose scan src --baseline .nose-baseline.json

# 3. Make CI fail only when NEW or CHANGED families exist:
nose scan src --baseline .nose-baseline.json --fail-on new
```

`--baseline` by itself keeps the historical behavior and reports only families not
accepted by the baseline (the default whenever `--baseline` is present). Use
`--fail-on new` when you want a CI ratchet that ignores accepted debt but exits
non-zero for new or changed clone families. Plain `--fail-on any` still means "fail if
anything is reported after the active filters."

Commit `.nose-baseline.json`. Families are keyed by their members' (file, name),
so the baseline is stable across line moves. New baselines also record the member
list next to the reviewable note, which lets later scans classify exact matches as
`unchanged`, overlapping but re-keyed families as `changed`, missing accepted
families as `resolved`, and unmatched current families as `new`. Regenerate the
baseline deliberately (re-run `--write-baseline`) when you've paid down duplication
and want the lower bar locked in — it's a ratchet.

With `--format json`, the top-level `baseline` object carries those counts and each
reported family gets `baseline_status: "new"` or `"changed"`.

## Structured ignores — audited suppressions

Baselines accept the current state in bulk. Structured ignores are for individual
families that were reviewed and intentionally kept. Commit `nose.ignore.json`
next to the code, or point to another file with `--ignore-file` / `ignore-file`
in [configuration](configuration.md):

```sh
nose scan src --ignore-file nose.ignore.json --fail-on any
```

Ignored families are removed from the active report, so they do not fail `--fail-on any`
or `--fail-on new`. They are still present in `--format json` under
`ignored_families`, with the ignore entry's reason, note, owner, expiry, matched
selectors, and matched paths.

Malformed ignore files fail the run. Expired entries are reported as warnings and
listed in `ignore.expired`, but are not applied. That makes stale waivers visible
instead of silently hiding duplication. See [structured-ignores](structured-ignores.md)
for the file format and selector semantics.

## SARIF for code scanning

`--format sarif` emits SARIF 2.1.0, which GitHub code-scanning ingests to render
findings as inline PR annotations:

```sh
nose scan src --format sarif --top 0 > nose.sarif
# then upload nose.sarif via github/codeql-action/upload-sarif
```

**Pass `--top 0` for a complete upload.** `--top` (default 30) truncates *every*
output format, SARIF included — without `--top 0` a repo with more than 30 families
uploads only the first 30. The SARIF run records the full count in
`runs[].properties` (`total_families` / `shown_families`) and, when families were
hidden, adds a `note` notification under `runs[].invocations[]`, so a truncated upload
is at least detectable; `--top 0` avoids the cap entirely.

`--format json` is the general machine-readable form for any other tooling. Its
versioned contract is documented in [scan-json](scan-json.md); it is truncated by
`--top` in the same way.

## Fast re-runs: `--cache-dir`

`--cache-dir <dir>` caches each file's analysis keyed by content hash. Unchanged
files are reused on the next run — skipping parse, [normalization](normalization.md), and feature
extraction — which makes repeated invocations (CI, pre-commit, local iteration)
much faster. Point it at a directory your CI caches between runs.

```sh
nose scan src --cache-dir .nose-cache --fail-on any
```

See [`CONTRIBUTING`](../CONTRIBUTING.md) for the full gate list.
