# Continuous integration

nose is built to run in CI as a duplication gate. The pieces below turn the
report from [usage](usage.md) into a pass/fail check that flags only *new* duplication
and runs fast on every push. Back to [home](home.md).

## The `--fail` gate

`--fail` makes nose exit non-zero if **any** family survives the filters. Pick the
channels deliberately: `--mode syntax` is the closest jscpd replacement, while the
default also reports exact semantic Type-4 clones.

For a jscpd-style copy-paste gate:

```sh
nose scan src --mode syntax --fail
```

For a broader exact gate, omit `--mode` and keep only substantial findings:

```sh
nose scan src --min-value 300 --min-members 3 --fail
```

To include Type-3 near-duplicates in a review ratchet, add `near` and tune the fuzzy
threshold. This is usually better as a report or ratchet with `--min-value` than as a
bare "any finding fails" gate:

```sh
nose scan src --mode syntax,semantic,near --threshold 0.70 --min-value 300 --min-members 3 --fail
```

For an exact semantic-only gate, use `--mode semantic`. It does not use a
similarity threshold.

With committed settings in `nose.toml`, the CI command can be just `nose scan src --fail`.

## Baselines — incremental adoption

An existing codebase already has dozens of clone families, so a bare `--fail`
gate is unusable on day one. A **baseline** records the currently-accepted
families; subsequent runs hide them and don't trip `--fail`, so the gate flags
only duplication introduced *after* adoption.

```sh
# 1. Accept today's state (writes the baseline file and exits):
nose scan src --baseline .nose-baseline.json --write-baseline

# 2. From now on, report/fail only on NEW families:
nose scan src --baseline .nose-baseline.json --fail
```

Commit `.nose-baseline.json`. Families are keyed by their members' (file, name),
so the baseline is stable as long as the duplicated code doesn't move. Regenerate
it deliberately (re-run `--write-baseline`) when you've paid down duplication and
want the lower bar locked in — it's a ratchet.

## SARIF for code scanning

`--format sarif` emits SARIF 2.1.0, which GitHub code-scanning ingests to render
findings as inline PR annotations:

```sh
nose scan src --format sarif > nose.sarif
# then upload nose.sarif via github/codeql-action/upload-sarif
```

`--format json` is the general machine-readable form for any other tooling.

## Fast re-runs: `--cache-dir`

`--cache-dir <dir>` caches each file's analysis keyed by content hash. Unchanged
files are reused on the next run — skipping parse, [normalization](normalization.md), and feature
extraction — which makes repeated invocations (CI, pre-commit, local iteration)
much faster. Point it at a directory your CI caches between runs.

```sh
nose scan src --cache-dir .nose-cache --fail
```

See [`CONTRIBUTING`](../CONTRIBUTING.md) for the full gate list.
