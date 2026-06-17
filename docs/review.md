# Review — catch un-propagated changes

> **Deprecated since 0.10.0** (still works in 0.11.0). `nose review --base <ref>` is now `nose query <paths> base=<ref>`
> — the same detection and the same `fire_eligible` gate, under the unified
> [query](usage.md#nose-query) surface (`base=REF --fail-on any` is the gate). `review` still
> works and `capabilities` lists it under `commands.deprecated`; it is slated for removal in a
> later release. The mechanics below are unchanged and describe both spellings.

`nose review` flags clone families that were **edited inconsistently** in a change set:
some copies changed, sibling copies not. That is the classic way a duplicated bug fix
slips through — you fix one copy and never learn the others exist, because they were
renamed or restructured enough that grep and your IDE can't find them. `review` finds the
siblings for you and asks: *should this change have gone there too?*

Where plain [`nose query`](usage.md#nose-query) is stateless (point it at any source, no
history), the `base=` view needs a **git repository** — it compares the working tree to a ref.
It shares query's detection channels, size gates, excludes, config loading, structured
ignores, and `top=`; report-shaping controls — the `sort=` term and the `--min-value` /
`--min-members` flags — and baselines (`--baseline` / `--fail-on new`) do not carry over. For
the standard clone taxonomy see [clone types](clone-types.md).

## Quick start

```sh
# Review your uncommitted local changes (pre-commit):
nose query . base=HEAD

# Review a PR branch against its merge target (CI):
nose query . base=origin/main
```

```
1 divergent family vs `origin/main` (3 files changed; 1 touch shared logic):
  9f2c1a  similar · prod · shared-logic (likely missed propagation)
    changed:      src/fs.rs:88-95  normalize_path
    not updated:  src/router.py:212-220  clean_route

next:
  nose query . base=origin/main --fail-on any   # fail CI on a proven divergence
```

(The deprecated `nose review` / `nose review --base origin/main` spelling still works and
prints the same findings in its own layout.)

The location listed under **not updated** is the copy your change skipped — open it and
decide whether the edit belongs there too, or whether the divergence is intentional.

## How it works

1. `git diff --unified=0 <base>` gives the lines your change touched.
2. nose detects clone families **at the base** — *before* your edit, where every copy
   still matches. This is deliberate: an edit can change a copy's shape enough to push it
   out of its own clone family, so detecting on the current tree would miss exactly the
   divergence you care about. A throwaway git worktree provides the base tree without
   disturbing your working tree.
3. For each family, members whose base span overlaps a changed line are **changed**; the
   rest are **not updated**. A family with *some but not all* members changed is flagged.
   (All copies changed = a consistent update, not flagged. None changed = irrelevant.)
4. Findings are ordered with the most likely un-propagated fix first. Review-surface exact
   fragments with enclosing context rank ahead of generic low-risk clone divergences, then
   the hazard score and changed-site complexity break ties.

This is a **candidate surfacer, not a proof**: nose tells you a sibling exists and wasn't
touched, not that the change definitely belongs there. Review each flagged sibling.

## The gate (`--fail-on any`)

The report and the gate are deliberately different surfaces. The report shows every
inconsistently-changed family; on `nose query <path> base=<ref>`, **`--fail-on any` fires
only on findings that pass the conservative shared-logic policy** ([experiments](experiments.md)):

- the diff **provably touches lines the changed copy shares with its un-updated
  sibling** — by the family's own equivalence proof for `exact-value-graph` families
  (a renamed twin's every line is shared logic), or by subtracting the member's
  varying spots for token/fuzzy families (an edit inside the part that already
  differed is not a propagation hazard); unprovable cases do not fire — the gate
  fires on proof, never on absence of one; and
- the family is not all-test scaffolding (`scope != "test"`).

Measured on replayed merged PRs against judge-labeled findings: the policy
keeps **every** genuine missed propagation while firing 73% less often than
span-overlap firing (change-level: 15% of merged changes vs 33%), at 3.7× the
precision. On the query spelling, `base=<ref> --fail-on any` *is* this conservative gate;
the broad fire-on-every-flagged-finding tier survives only on the deprecated
`nose review --fail --fail-on any`, for ratchet-style use. Each JSON finding carries
`fire_eligible`, `witness_kind`,
`scope`, per-changed-site `touches_shared`, and — for near families — the family's
[graded witness](graded-witness.md) (`graded`: `equal_modulo_holes`, `holes`,
`patterns`, `referent_mismatches`, `caveat_names`), so a CI wrapper can apply its own
tier without re-deriving the analysis.

The graded witness is **evidence for the consumer, not a fire gate**: a clean
`equal_modulo_holes` family is a strong missed-propagation candidate, while a
`referent-mismatch` / `decorator-differs` family is one whose copies are not really
the same logic (a likely false fire the consumer can down-rank). It deliberately does
**not** gate `fire_eligible` — a decorator or a same-named-but-different-referent
difference does not stop a shared-*body* fix from being a genuine missed propagation,
so suppressing on it would risk the keep-every-propagation property the shared-logic policy
is measured against. The fire decision stays the shared-logic proof; the witness only
makes a borderline fire explainable.

## Flags and terms

The `base=` view shares [`nose query`](usage.md#nose-query)'s detection flags — `--mode`
(`syntax`/`semantic`/`near[:T]`), `--min-size`, advanced `--min-lines`, `--exclude`,
`--config` — plus `--format`, `--ignore-file`, and the gate `--fail-on any`. One deliberate
difference from a plain `nose query`: when `--mode` is omitted the `base=` view defaults to
the conservative `syntax,semantic` mix (a plain `nose query` also runs `near`) — it feeds a
gate, where a false fire costs more than a missed candidate. Add `--mode syntax,semantic,near`
to include the fuzzy channel.

| flag / term | effect |
|---|---|
| `base=<ref>` | compare the working tree against this git ref (`HEAD` = uncommitted changes; `origin/main` for a PR branch) |
| `--fail-on any` | exit non-zero when the gate fires — the conservative shared-logic policy (see *The gate* above) |
| `--format human\|json\|markdown\|sarif` | output format (default `human`; `markdown` currently renders the human-readable review report) |
| `--ignore-file <file>` | suppress accepted divergences (auto-reads `nose.ignore.json`) |
| `top=N` | show at most N findings (`0` = all; default 30) |

The deprecated `nose review` spells these as flags: `--base <ref>`, `--top N`, and a
two-knob gate `--fail` with `--fail-on shared-logic|any` (default `shared-logic`, where `any`
fires on every flagged finding).

## Exact fragment context

Semantic mode can flag exact sub-function fragments, not only whole functions or methods.
Those small fragments are often too small to be default refactoring candidates, but they
are useful review hazards when the changed lines touch one copy and skip another. Each
`base=` finding therefore carries stable per-site fragment metadata in `--format json`:
`is_fragment`, `fragment_kind`, `reason_code`, `span_lines`, `span_tokens`, and
`enclosing_unit` when a containing function/method/class is recovered exactly.

Human and SARIF output keep annotations anchored on the changed or not-updated fragment
span, while the context text names the enclosing unit. Human output prints fragment context
for both changed and not-updated sites so a one-line guard or effect is reviewed inside its
surrounding function. JSON output includes the full fragment metadata for both `changed` and
`not_updated` sites. `proof_facts` are not emitted; fragment `reason_code` explains the
exact proof shape, not the broader family/actionability reasons (future work).

## Suppressing intentional divergences

Some clones are *meant* to diverge (a fast path vs a clear path, a sync vs async variant).
So a true fork doesn't re-fail every PR, the `base=` view honors the same
[structured ignores](structured-ignores.md) as the rest of `nose query`: copy a finding's
`id` (from `--format json`) into the `family_id` of a `nose.ignore.json` entry, with a reason.
nose auto-reads that file from the current working directory, and the suppressed family no
longer trips `--fail-on any`.

## In CI

Run it on a pull request and fail the build (or post SARIF annotations) when a change
lands in one copy but not its clones:

```sh
nose query . base="origin/${GITHUB_BASE_REF}" --fail-on any
# or, for inline PR annotations on the un-updated copies:
nose query . base="origin/${GITHUB_BASE_REF}" --format sarif > nose-review.sarif
```

SARIF results are anchored on the **un-updated sibling** (where the fix may be missing),
so a code-scanning annotation lands on the copy the change skipped.

## Limits (v1)

- Reviews a **single diff** (`base..worktree`). Mining a whole history for old, still-
  unreconciled divergences is future work.
- Detection is at the base, so a clone whose copy is **newly added** in the change (it did
  not exist at the base) is not yet considered.
- The harm ordering is a structural heuristic (~0.6–0.65 on mined divergence labels; see
  [hazard-benchmark](hazard-benchmark.md)). It prioritizes candidates; it does not certify
  them.
