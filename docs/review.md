# Review — catch un-propagated changes

`nose review` flags clone families that were **edited inconsistently** in a change set:
some copies changed, sibling copies not. That is the classic way a duplicated bug fix
slips through — you fix one copy and never learn the others exist, because they were
renamed or restructured enough that grep and your IDE can't find them. `review` finds the
siblings for you and asks: *should this change have gone there too?*

Where [scan](usage.md) is stateless (point it at any source, no history), `review` needs
a **git repository** — it compares the working tree to a ref. It shares scan's detection
channels, size gates, excludes, and config loading; scan-only config/report shaping such as
`sort`, `min-value`, `min-members`, baselines, config `top`, and config `ignore-file` does
not carry over. For the standard clone taxonomy see [clone types](clone-types.md).

## Quick start

```sh
# Review your uncommitted local changes (pre-commit):
nose review

# Review a PR branch against its merge target (CI):
nose review --base origin/main
```

```
reviewing changes vs `origin/main` · 3 files changed

⚠ 1 clone family changed inconsistently — a copy was edited but its sibling(s) were not:

#1  changed: normalize_path (src/fs.rs:88-95)  (sim 0.94)
    not updated: clean_route (src/router.py:212-220)
    → review whether the change should also apply to the sibling(s)
```

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
Measured on replayed merged PRs ([experiments §BR](experiments.md)), review fires on
roughly a third of real changes and the top finding is a genuine missed propagation in
~4% of fires — real catches exist (including one later fixed upstream for exactly the
flagged reason), but treat `--fail` as an explicitly-opted, policy-tuned gate, not a
default-on blocker, until the conservative fire policy lands.

## Flags

The review command shares the detection flags with [scan](usage.md): `--mode`
(`syntax`/`semantic`/`near[:T]`), `--min-size`, advanced `--min-lines`, `--exclude`,
`--config`. One deliberate difference: review's default mix stays the conservative
`syntax,semantic` (scan's default also includes `near`) — review feeds a gate, where a
false fire costs more than a missed candidate. Add `--mode syntax,semantic,near` to
review with the fuzzy channel included.

| flag | effect |
|---|---|
| `--base <ref>` | compare the working tree against this git ref (default `HEAD` = uncommitted changes; `origin/main` for a PR branch) |
| `--format human\|json\|markdown\|sarif` | output format (default `human`; `markdown` is accepted but currently uses the human-readable review report) |
| `--fail` | exit non-zero if any family changed inconsistently (CI gate) |
| `--ignore-file <file>` | suppress accepted divergences (auto-reads `nose.ignore.json`) |
| `--top N` | show at most N findings (`0` = all; default 30) |

## Exact fragment context

Semantic mode can flag exact sub-function fragments, not only whole functions or methods.
Those small fragments are often too small to be default refactoring candidates, but they
are useful review hazards when the changed lines touch one copy and skip another. Review
output therefore carries the same stable fragment metadata as scan JSON:
`is_fragment`, `fragment_kind`, `reason_code`, `span_lines`, `span_tokens`, and
`enclosing_unit` when a containing function/method/class is recovered exactly.

Human and SARIF output keep annotations anchored on the changed or not-updated fragment
span, while the context text names the enclosing unit. Human output prints fragment context
for both changed and not-updated sites so a one-line guard or effect is reviewed inside its
surrounding function. JSON output includes the full fragment metadata for both `changed` and
`not_updated` sites. `proof_facts` are not emitted; fragment `reason_code` explains the
exact proof shape, not the broader family/actionability reasons planned in #11.

## Suppressing intentional divergences

Some clones are *meant* to diverge (a fast path vs a clear path, a sync vs async variant).
So a true fork doesn't re-fail every PR, `review` honors the same
[structured ignores](structured-ignores.md) as `scan`: copy a finding's `family_id` (from
`--format json`) into `nose.ignore.json`, with a reason. nose auto-reads that file from the
current working directory, and the
suppressed family no longer trips `--fail`.

## In CI

Run it on a pull request and fail the build (or post SARIF annotations) when a change
lands in one copy but not its clones:

```sh
nose review --base "origin/${GITHUB_BASE_REF}" --fail
# or, for inline PR annotations on the un-updated copies:
nose review --base "origin/${GITHUB_BASE_REF}" --format sarif > nose-review.sarif
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
