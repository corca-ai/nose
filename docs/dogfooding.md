# Dogfooding nose on nose

nose polices its own duplication with the same query surface it gives users.
This page is the current operating guide for that gate; the detailed review log
and older candidate-by-candidate judgments live in [dogfooding history](dogfooding-history.md).

The third-party counterpart is [field evaluation](field-evaluation.md). The
repository workflow that runs this gate is in [CONTRIBUTING](../CONTRIBUTING.md).

## Current gate

The CI gate is [`scripts/check-duplication.sh`](../scripts/check-duplication.sh).
It runs:

```sh
nose query crates all top=0 --mode near --min-value 40 --format json
```

and compares the default-surface family IDs with the
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) baseline.
The baseline file is the machine source of truth for the accepted family set,
budget, mode, minimum value, and output surface.

Tests are included in the ratchet so fixture/scaffolding copy-paste stays
visible instead of being policed only by the file-length gate. A family
disappearing also requires a baseline/docs update, so an unrelated removal
cannot mask a newly introduced duplicate.

## Current baseline

The current reviewed default-surface budget is 29 families, matching
`scripts/duplication-baseline.json`. The latest budget-tightening cleanup is
the Guava map factory fixture cleanup, which tightened the budget from 31 to 29
by moving the repeated `ImmutableMap.of` hard-negative IL/evidence fixture and
unsupported-arity payload builder into `nose_semantics::test_support`. The
latest documented cleanup is the callee-dependency matcher policy-helper
cleanup, which kept the budget at 29 while reducing the reviewed remaining
matcher family.

The accepted family IDs are intentionally kept in the
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) baseline
rather than copied into prose. The review trail for why families were accepted,
deduped, or reclassified is in [dogfooding history](dogfooding-history.md).

## Updating the baseline

When the duplication gate changes, review the family delta before editing the
baseline:

- If the new family is avoidable duplication, remove it instead of accepting it.
- If the new or changed family is intentionally accepted, update
  [`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) and
  append the short decision trail to [dogfooding history](dogfooding-history.md).
- If a family disappears, tighten the baseline and record the cleanup or
  representative-ID movement in [dogfooding history](dogfooding-history.md).

Keep this page focused on the live gate and process. Put detailed chronology,
candidate tables, and budget movement notes in the history page.
