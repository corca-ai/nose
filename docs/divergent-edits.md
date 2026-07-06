# Divergent Edits

`nose query <paths> base=<ref>` flags clone families that were **edited inconsistently** in a change set:
some copies changed, sibling copies not. That is the classic way a duplicated bug fix
slips through — you fix one copy and never learn the others exist, because they were
renamed or restructured enough that grep and your IDE can't find them. `base=<ref>` finds the
siblings for you and asks: *should this change have gone there too?*

Where plain [`nose query`](usage.md#nose-query) is stateless (point it at any source, no
history), the `base=` view needs a **git repository** — it compares the working tree to a ref.
It shares most of `nose query`'s surface (the shared flags are listed under [Flags and
terms](#flags-and-terms)); the report-shaping controls — the `sort=` term and the
`--min-value` / `--min-members` flags — and baselines (`--baseline` / `--fail-on new`) do not
carry over. For the standard clone taxonomy see [clone types](clone-types.md).

## Quick start

```sh
# Inspect your uncommitted local changes (pre-commit):
nose query . base=HEAD

# Inspect a PR branch against its merge target (CI):
nose query . base=origin/main
```

```
1 divergent family vs `origin/main` (3 files changed; 1 strict, 1 legacy fire-eligible):
  9f2c1a  similar · prod · base-divergence · strict (likely missed propagation)
    changed:      src/fs.rs:88-95  normalize_path
    not updated:  src/router.py:212-220  clean_route

next:
  nose query . base=origin/main --fail-on any   # fail CI on strict divergences
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
4. Findings are ordered with the most likely un-propagated fix first. Divergence-surface exact
   fragments with enclosing context rank ahead of generic low-risk clone divergences, then
   the hazard score and changed-site complexity break ties.

This is a **candidate surfacer, not a proof**: nose tells you a sibling exists and wasn't
touched, not that the change definitely belongs there. Inspect each flagged sibling.

## The gate (`--fail-on any`)

The report and the gate are deliberately different surfaces. The report shows active
base-tree divergence findings plus bounded advisory lanes; on
`nose query <path> base=<ref>`, **`--fail-on any` fires only on unsuppressed `strict`
findings** ([experiments](experiments.md)). A `strict`
finding must satisfy the legacy conservative shared-logic proof and the v2 tier policy:

- the diff **provably touches lines the changed copy shares with its un-updated
  sibling** — by the family's own equivalence proof for `exact-value-graph` families
  (a renamed twin's every line is shared logic), or by subtracting the member's
  varying spots for token/fuzzy families (an edit inside the part that already
  differed is not a propagation hazard); unprovable cases do not become `strict`
  — the gate fires on proof, never on absence of one; and
- the family is production scope (`scope="prod"`). Mixed and all-test findings remain
  visible, but they are `report-only` by default.

Measured on replayed merged PRs against judge-labeled findings: the v1 conservative
`fire_eligible` policy kept every genuine missed propagation while firing 73% less
often than span-overlap firing (change-level: 15% of merged changes vs 33%), at 3.7×
the precision. The #672 v2 strict policy keeps the same confirmed positives on the
checked #670 labelset while demoting mixed/test evidence from default-failing output.
Each JSON finding carries legacy `fire_eligible`, the v2 `tier` and `gate.fail_default`,
`witness_kind`, `scope`, per-changed-site `touches_shared`, and — for near families —
the family's [graded witness](graded-witness.md) (`graded`: `equal_modulo_holes`,
`holes`, `patterns`, `referent_mismatches`, `caveat_names`), so a CI wrapper can use
the emitted tier without re-deriving the analysis.

The graded witness is **evidence for the consumer, not a fire gate**: a clean
`equal_modulo_holes` family is a strong missed-propagation candidate, while a
`referent-mismatch` / `decorator-differs` family is one whose copies are not really
the same logic (a likely false fire the consumer can down-rank). It deliberately does
**not** gate legacy `fire_eligible` — a decorator or a same-named-but-different-referent
difference does not stop a shared-*body* fix from being a genuine missed propagation,
so suppressing on it would risk the keep-every-propagation property the shared-logic policy
is measured against. The shared-logic proof stays separate from graded-witness
presentation evidence; the v2 tier decides whether that proof is default-failing.

## V2 gate tiers (design contract)

#670 refreshed the replay measurement and changed the next implementation target:
the useful signal is not only the top-ranked finding. The v2 contract therefore
separates **what nose reports** from **what may fail CI** with an explicit tier on
each divergent-edit finding. The v1 `fire_eligible` field remains as compatibility
evidence, but default CI uses the v2 `strict` tier.

| tier | CI behavior | evidence requirement | intended reader action |
|---|---|---|---|
| `strict` | `base=<ref> --fail-on any` exits non-zero when at least one unsuppressed `strict` finding is shown | `fire_eligible=true`, `scope="prod"`, `taxonomy_hint="missed_propagation"`, and no higher-priority report-only or suppression reason | treat as a likely missed sibling edit; block or require an explicit suppression |
| `review` | reported in human/JSON/SARIF, does not fail by default | base-tree divergent-edit candidate that is not suppressed, not report-only, and not strict | inspect during review; optionally fail in a custom wrapper |
| `report-only` | reported only as advisory evidence, never fails default CI | useful context outside the default gate: test-only scaffolding, grouping artifacts, or newly added current-tree copies with no base member | use as reviewer/agent context; do not treat as a blocker |
| `suppressed` | omitted from active human/SARIF gate output and never fails | matched by a structured ignore or accepted suppression | audit through the ignore file, not through repeated PR noise |

The v2 enums are closed for schema v8. Adding, renaming, or removing one requires a
schema bump. `taxonomy_hint` is an evidence label for routing and UI copy, not a
claim that the code is correct or incorrect:

| taxonomy bucket | product meaning | v2 routing |
|---|---|---|
| `missed_propagation` | the changed logic likely belongs in an un-updated sibling | `strict` when the evidence is proven and unsuppressed; otherwise `review` |
| `no_propagation_needed` | the span overlaps a clone member, but the edit does not touch the shared logic that should propagate | `review` |
| `intentional_variant` | copies are deliberately specialized, such as sync/async, platform, protocol, or shell variants | `review` unless explicitly ignored; `suppressed` after a committed structured ignore |
| `test_scaffolding` | the candidate is test fixture/setup/expectation churn rather than product logic | `report-only` |
| `grouping_artifact` | the family is too broad or not actually the same logic | `report-only` |
| `unclear` | evidence is insufficient for a stable product decision | `review` |

`tier_reasons[]` is also closed for schema v8. The allowed reason codes are
`shared_logic_touched`, `shared_logic_not_touched`, `shared_logic_unproven`,
`non_test_scope`, `test_scope`, `variant_signal`, `test_scaffolding`,
`grouping_artifact`, `new_copy_no_base_member`, `structured_ignore`, and
`unclassified`.

The tier decision is deterministic. Apply these rules in order:

1. A finding matched by a structured ignore is `suppressed`; active human, JSON, and
   SARIF output omit it, and it never fails a gate. A partial path/language ignore
   still obeys the existing all-members selector semantics.
2. `test_scaffolding`, `grouping_artifact`, `test_scope`, and
   `new_copy_no_base_member` route to `report-only`. In the implemented #672
   strict policy, `test_scope` covers both `scope="test"` and `scope="mixed"`;
   only `scope="prod"` can be `strict`.
3. An unsuppressed finding with `fire_eligible=true` and
   `taxonomy_hint="missed_propagation"` in `scope="prod"` routes to `strict`.
4. Every other unsuppressed base-tree divergent-edit candidate routes to `review`.

Newly added copy evidence is a separate report-only lane, not a base-tree
propagation verdict. It is detected from the current tree when an added, copied,
or renamed path becomes part of a current clone family with an untouched sibling.
To avoid adding full current-tree detection cost to broad PRs, this advisory lane
runs only when the diff touches at most two source files. It carries
`lane="new-copy"`, `base_family_id=null`, `current_only[]` sites with
`tree="current"`, and `new_copy_no_base_member` so CI wrappers never promote it
to `strict`. Pure moves with no current-side changed range stay quiet; moved files
are reported only when current clone evidence, not path similarity alone, supports
the relationship.

The v2 policy preserves the current fail-closed posture: if shared-line proof,
source spans, graded witness data, or suppression data are unavailable, the finding
can be reported for review, but it must not be promoted to `strict`.

## Flags and terms

The `base=` view shares [`nose query`](usage.md#nose-query)'s detection flags — `--mode`
(`syntax`/`semantic`/`near[:T]`), `--min-size`, advanced `--min-lines`, `--exclude`,
`--config` — plus `--format`, `--ignore-file`, and the gate `--fail-on any`. One deliberate
difference from a plain `nose query`: when `--mode` is omitted the `base=` view defaults to
the conservative `syntax,semantic` mix (a plain `nose query` also runs `near`) — it feeds a
gate, where a false fire costs more than a missed candidate. Add `--mode syntax,semantic,near`
to include the fuzzy channel. `base=` is a diff view, not a family list view, so ordinary
query filters (`path~`, `witness=`, `group=`, `since=`, `id=`) are rejected instead of ignored.

| flag / term | effect |
|---|---|
| `base=<ref>` | compare the working tree against this git ref (`HEAD` = uncommitted changes; `origin/main` for a PR branch) |
| `--fail-on any` | exit non-zero when at least one unsuppressed `strict` finding has `gate.fail_default=true` (see *The gate* above) |
| `--format human\|json\|markdown\|sarif` | output format (default `human`; `markdown` currently renders the human-readable divergent-edit report) |
| `--ignore-file <file>` | suppress accepted divergences (auto-reads `nose.ignore.json`) |
| `top=N` | show at most N findings (`0` = all; default 30) |

Machine output follows the same limit. JSON records the total and shown finding counts
(`summary.divergences` / `summary.shown_divergences`). SARIF records
`runs[].properties.total_families` and `shown_families`, plus a note pointing to
`top=0`, so CI uploads can detect when only the top findings were emitted.

## Exact fragment context

Semantic mode can flag exact sub-function fragments, not only whole functions or methods.
Those small fragments are often too small to be default refactoring candidates, but they
are useful divergence hazards when the changed lines touch one copy and skip another. Each
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
[structured ignores](structured-ignores.md) as the rest of `nose query`: copy
`items[].family_id` from `--format json` into the `family_id` of a `nose.ignore.json`
entry, with a reason. nose auto-reads that file from the current working directory,
or from `[query] ignore-file = "..."` in `nose.toml`; the suppressed family no
longer appears in active human/JSON/SARIF output and no longer trips
`--fail-on any`.

For a strict finding (`gate.fail_default=true`), use one of two outcomes:

1. propagate the edit to the skipped sibling, or otherwise change the code so the
   family is no longer divergent;
2. commit an audited structured ignore when the divergence is intentional.

Keep ignores narrow. A `paths` or `languages` selector applies only when every
member of the reported family matches it; an entry covering just the changed copy
does not hide the un-updated sibling. Prefer `family_id` for a one-off accepted
divergence:

```json
{
  "ignores": [
    {
      "family_id": "479389f590c1234a",
      "reason": "intentional-variant",
      "owner": "runtime",
      "expires_at": "2026-12-31"
    }
  ]
}
```

## In CI

Run it on a pull request and fail the build (or post SARIF annotations) when a change
lands in one copy but not its clones:

```sh
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --fail-on any
# or, for inline PR annotations on the un-updated copies:
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --format sarif top=0 > nose-divergence.sarif
```

Pin `--mode` in CI even though the `base=` default is already conservative; it makes
upgrade diffs explicit. `top=0` is the complete-upload spelling for SARIF. Report-only
findings, including mixed/test scope and `new-copy` current-tree evidence, remain visible
but do not fail the default gate.

Base-divergence SARIF results are anchored on the **un-updated sibling** (where the fix
may be missing), so a code-scanning annotation lands on the copy the change skipped. The
rule id, level, message, and `properties.tier` identify whether a finding is `strict`,
`review`, or `report-only`.

## History Mining

For offline audits across a bounded commit range, use the maintained [`scripts/divergent-history-mining.py`](../scripts/divergent-history-mining.py) harness.
The harness checks out each selected commit in a temporary worktree, runs the
normal `base=<parent>` JSON view with `top=0`, and groups repeated findings so a
long-lived skipped sibling is reviewable once instead of once per commit. See
[divergent history mining](divergent-history-mining.md) for the workflow and
schema.

## Limits

- Checks a **single diff** (`base..worktree`). Mining a whole history for old, still-
  unreconciled divergences is handled by the bounded history-mining harness above.
- The default base-divergence lane detects clone families at the base. A clone whose copy is
  **newly added** in the change has no base member, so it is considered only by the bounded
  `new-copy` report-only lane when the diff is small enough to keep runtime predictable.
- The harm ordering is a structural heuristic (~0.6–0.65 on mined divergence labels; see
  [hazard-benchmark](hazard-benchmark.md)). It prioritizes candidates; it does not certify
  them.
