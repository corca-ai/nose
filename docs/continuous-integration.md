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

To include Type-3 near-duplicates in an audit ratchet, add `near` and tune the fuzzy
threshold. This is usually better as a report or ratchet with `--min-value` than as a
bare "any finding fails" gate:

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
nose query src --format sarif top=0 > nose.sarif   # then upload via github/codeql-action/upload-sarif
```

**Pass `top=0` for a complete upload.** Every output format truncates to the row limit —
`top=N` (default 30); `top=0` means *all*.
Without it a repo with more than 30 families uploads only the first 30. The SARIF run records
the full count in `runs[].properties` (`total_families` / `shown_families`) and, when families
were hidden, adds a `note` notification under `runs[].invocations[]`, so a truncated upload is
at least detectable; `top=0` avoids the cap entirely.

`--format json` is the general machine-readable form for any other tooling. The forward
versioned contract is [query-json](query-json.md) (`nose query --format json`, schema v7).
It is truncated by the active top limit in the same way.

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
ratchet and the nightly soundness corpus-verify policy — lives in
[CONTRIBUTING](../CONTRIBUTING.md), not here.
