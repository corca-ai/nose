# Dogfooding nose on nose

nose polices its own duplication with the same query surface it gives users.
This page is the current operating guide for that gate; the detailed review log
and older candidate-by-candidate judgments live in [dogfooding history](dogfooding-history.md).

The third-party counterpart is [field evaluation](field-evaluation.md). The
repository workflow that runs this gate is in [contributing](contributing.md).

## Current gate

The CI gate is [`scripts/check-duplication.sh`](../scripts/check-duplication.sh).
It runs:

```sh
nose query crates all top=0 --mode near --min-value 40 --format json
```

and compares content-bound default-surface review fingerprints with the
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) baseline.
The baseline file is the machine source of truth for the accepted family set,
budget, mode, minimum value, and output surface.

Tests are included in the ratchet so fixture/scaffolding copy-paste stays
visible instead of being policed only by the file-length gate. A family
disappearing also requires a baseline/docs update, so an unrelated removal
cannot mask a newly introduced duplicate.

## Current baseline

The current default surface reports 18 reviewed families within a hard budget
of 20, as recorded in `scripts/duplication-baseline.json`. Shared semantic-query,
Promise, provenance, binding-domain, HOF, and call-target fixtures remove
repeated test workflows. Shared frontend CST adapters and one callee-identity
matcher own the common production mechanics without erasing language-specific
policy. The detailed family movement is recorded in the history below.

The accepted review records are intentionally kept in the
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) baseline
rather than copied into prose. The review trail for why families were accepted,
deduped, or reclassified is in [dogfooding history](dogfooding-history.md).

## Review identity

Live family IDs include source lines so users can navigate to distinct code
occurrences. The gate instead binds existing region content keys to reported
files, symbols, enclosing owners and member-relative shared computations,
together with scope and witness. Member order, unrelated line/byte shifts and
whole-file hash changes do not change approval. Code, ownership, witness and
occurrence-count changes do. The live ID remains a navigation hint in each
review record; it is not the approval key.

Missing content provenance and ambiguous fingerprints fail closed. New,
missing and over-budget families still fail, so this identity migration does
not disable recall review or loosen the ratchet. The repository uses v0.2
review records; legacy v0.1 baselines keep their ID-based behavior.

Use `scripts/check-duplication-baseline.py --dump-current` to inspect current
review records without accepting or writing them. Regression tests exercise
position changes, same-span body edits, ownership, multiplicity and invalid
evidence; the gate also runs the real CLI on a small movement/body-edit fixture.

## Updating the baseline

When the duplication gate changes, review the family delta before editing the
baseline:

- If the new family is avoidable duplication, remove it instead of accepting it.
- If the new or changed family is intentionally accepted, update
  [`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json) and
  append the short decision trail to [dogfooding history](dogfooding-history.md).
- If a family disappears, review whether source changed, ranking crossed the
  threshold or detection regressed before updating the baseline. Record that
  decision in [dogfooding history](dogfooding-history.md).

Keep this page focused on the live gate and process. Put detailed chronology,
candidate tables, and budget movement notes in the history page.
