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
`nose query <path> base=<ref>`, **`--fail-on any` fires only when an emitted item has
`gate.fail_default=true`**. Today that means an unsuppressed `strict` finding
([experiments](experiments.md)). A default-failing finding must satisfy the legacy
conservative shared-logic proof and the v2 tier policy:

- the diff **provably touches lines the changed copy shares with its un-updated
  sibling** — by the family's own equivalence proof for `exact-value-graph` families
  (a renamed twin's every line is shared logic), or by subtracting the member's
  varying spots for token/fuzzy families (an edit inside the part that already
  differed is not a propagation hazard); unprovable cases do not become `strict`
  — the gate fires on proof, never on absence of one; and
- the family is production scope (`scope="prod"`). Mixed and all-test findings remain
  visible, but they are `report-only` by default.

Measured on replayed merged PRs against judge-labeled findings: the final checked v2
strict baseline fires on 80 findings, with 45 true positives and 35 false positives,
for strict precision 0.562. It retains 45/45 confirmed v1 missed-propagation
positives while demoting mixed/test evidence from default-failing output. The
remaining strict false positives split into 17 `no_propagation_needed`, 13
`intentional_divergence`, and 5 `not_a_clone` findings. This supports an opt-in
review gate; it is not a default-on blocking readiness claim.

The next gate-policy cycle is governed by the [sealed #848 precision
protocol](../eval/divergence_fire/RESULTS.md#sealed-precision-first-protocol-2026-07-14-848).
The existing 28-repository, 179-label set is now development-only. Blind decisions use
a fresh repository-disjoint, source-redacted population; a separate post-seal temporal
reserve is required before any default-on claim. The protocol fixes target, finding,
and change precision, one-sided 95% Wilson bounds, and repository-atomic stopping with
at least 100 distinct strict findings across 20 complete repositories, including two
complete repositories per supported language. It permits only an aggregate
seven-language readiness claim, not post-hoc per-language claims. The temporal
1,000-change selection uses fixed future checkpoints, exact first-parent ancestry, and
secret ordering, so it cannot be chosen after seeing blind quality. Until both the blind
target gate and the stricter temporal default-on gate pass, `--fail-on any` remains an
opt-in review gate.

Each JSON finding carries legacy `fire_eligible`, the v2 `tier` and `gate.fail_default`,
`witness_kind`, `scope`, per-changed-site `touches_shared`, and — for near families —
the family's [graded witness](graded-witness.md) (`graded`: `equal_modulo_holes`,
`holes`, `patterns`, `referent_mismatches`, `caveat_names`), so a CI wrapper can use
the emitted `gate.fail_default` value without re-deriving the analysis.

It also carries `targets[]`, the exact propagation edges under evaluation. Clone families are
transitive components: if A matched B and B matched C, family membership alone does not prove
that A matched C. nose therefore retains detector-accepted pairs before clustering and emits a
target only when one direct edge crosses from a changed member to a skipped member. A bridge or
other transitive member remains in `changed[]` / `not_updated[]` as review context but cannot be
named as the endpoint of a strict action without its own accepted edge.

Each target has a stable `target_id`, base-tree `changed` and `skipped` sites, and a pair-local
`direct_witness` with detector kind and similarity. Shared-line contact and bounded semantic
change are recomputed for that exact pair. The ID is derived from repo-relative base coordinates
and unit metadata, so changing the temporary worktree path or moving/renaming the current-tree
file does not change the base target. JSON and SARIF expose the same ID; SARIF also attaches it
to the skipped primary location and changed related location. The #852 development
qualification found no non-degenerate v3 candidate, so the v2 family gate remains the active
authority and target evidence remains inspectable review context.

Each target also carries `variant_evidence`. Its closed `status` is `none`, `advisory`, or
`disqualifying`; the last value means that at least one strong, pair-local signal was observed.
Role and definition-modifier evidence compares the current changed result with the base-tree
skipped endpoint, while identity remains anchored to the direct base edge.
Strong signals are limited to resolved referent mismatch, definition decorator/attribute
mismatch, async or observable-effect role mismatch, incompatible explicit protocol roles, and
provably disjoint platform constraints. Every signal names its exact changed/skipped evidence.
Name, path, and version-label differences are emitted only with `strength="weak"`; they cannot
become hard-block authority alone. Projection/source loss, unresolved referents, truncation, and
overlapping or incomparable platform constraints remain explicit advisory `caveats[]`. #851
records these facts but deliberately leaves the v2 tier and `gate.fail_default` unchanged. #852
priced the admissible target-local policy class and did not qualify a v3 consumer.

The graded witness is **evidence for the consumer, not a fire gate**: a clean
`equal_modulo_holes` family is a strong missed-propagation candidate, while a
`referent-mismatch` / `decorator-differs` family is evidence that the copies may have
different roles or referents. It deliberately does
**not** gate legacy `fire_eligible` — a decorator or a same-named-but-different-referent
difference does not stop a shared-*body* fix from being a genuine missed propagation,
so suppressing on it would risk the keep-every-propagation property the shared-logic policy
is measured against. The shared-logic proof stays separate from graded-witness
presentation evidence; the v2 tier decides whether that proof is default-failing.

## V3 development qualification outcome

#852 froze a monotone precision-first policy class: a hard-block target must have a direct
changed→skipped edge, pair-local shared contact, a caveat-free `complete` semantic witness for a
mapped replacement or deletion, and no strong variant or uncertainty caveat. Closed variant
signals may only demote; missing evidence cannot promote.

On the 80-finding public development slice, 168 direct targets were emitted but zero had a
`complete` semantic witness. Because every admissible policy requires that witness, every
policy in the class has zero support before any variant-code choice is made. A diagnostic that
relaxed the fail-closed caveat requirements selected 13 findings at only 0.538 finding
precision. Development labels also adjudicate findings rather than the newly introduced direct
targets, so they cannot establish target precision.

The checked result is therefore `no-policy-qualifies`: no v3 binary or threshold was frozen for
blind replay, the sealed blind labels remain unopened, schema v8 and capability flags were not
bumped, and `divergent-edit-v2-strict` remains the active opt-in policy. Human, JSON, SARIF, and
exit status now consume one internal policy-decision object, with `items[].gate.fail_default`
remaining the sole machine authority. See the [development result](../eval/divergence_fire/RESULTS.md#v3-policy-development-qualification-2026-07-18-852).

#853 then ran the sealed-replay preflight, not the private replay. Because no runnable v3
candidate identity existed, opening the held-out population would not evaluate a defined policy
and would only consume the seal. Its single checked verdict is `failed` at the
`pre-unseal-development-qualification` stage: zero private repositories or changes were opened,
no quality labels were created or revealed, and the held-out population remains
`sealed-unjudged`. This is a failed default-on cycle, not a blind precision measurement.

## Bounded semantic-change evidence

For an already-flagged base divergence, each family-level changed site and each direct target's
changed site also carries
`semantic_change`. nose aligns that base unit with its current-tree unit, compares bounded
value-DAG and behavior-sink projections, and maps affected base nodes into a capped set of
skipped siblings. Family-level evidence retains the capped aggregate for review; target-level
evidence maps against exactly one skipped endpoint. This distinguishes source contact with no normalized semantic delta from
replacement, deletion, and insertion of value, return, control, or effect behavior.

The analysis is candidate-local: at most 64 selected base/current files of 2 MiB each, 16
changed sites, 16 skipped siblings, and 64 direct targets per family, 512 units per file, and 2,048 value nodes
per unit participate. It does not discover or scan the repository again. Unsupported
fragments or declarative/container languages, parse/lower failures, missing current units,
lossy lowering, unresolved referents, ambiguous or heuristic alignment, pure insertions,
mixed changes, and cap exhaustion remain explicit advisory or unavailable evidence.

This is deliberately not a v2 decision input. `tier()` and `gate.fail_default` continue to
use the frozen v2 policy above; even a complete semantic-change witness cannot promote a
finding in this implementation. The evidence remains review context; a future policy cycle
would need a fresh target-adjudicated development set before it could be frozen and evaluated
blind.

## V2 gate tiers (design contract)

#670 refreshed the replay measurement and changed the next implementation target:
the useful signal is not only the top-ranked finding. The v2 contract therefore
separates **what nose reports** from **what may fail CI** with an explicit tier on
each divergent-edit finding. The v1 `fire_eligible` field remains as compatibility
evidence, but default CI uses `gate.fail_default`; the current policy sets it only for
unsuppressed `strict` findings.

| tier | CI behavior | evidence requirement | intended reader action |
|---|---|---|---|
| `strict` | `base=<ref> --fail-on any` exits non-zero only when the item has `gate.fail_default=true` | `fire_eligible=true`, `scope="prod"`, `taxonomy_hint="missed_propagation"`, and no higher-priority report-only or suppression reason | treat as a likely missed sibling edit; propagate it or require an explicit suppression |
| `review` | reported in human/JSON/SARIF, does not fail by default | base-tree divergent-edit candidate that is not suppressed, not report-only, and not strict | inspect during review; keep advisory unless a later measured policy changes it |
| `report-only` | reported only as advisory evidence, never fails default CI | useful context outside the default gate: test-only scaffolding, grouping artifacts, or newly added current-tree copies with no base member | use as reviewer/agent context; do not treat as a blocker |
| `suppressed` | omitted from active human/SARIF gate output and never fails | matched by a structured ignore or accepted suppression | audit through the ignore file, not through repeated PR noise |

The v2 enums are closed for schema v8. Adding, renaming, or removing one requires a
schema bump. `taxonomy_hint` is an evidence label for routing and UI copy, not a
claim that the code is harmful, correct, intentional, or not a clone:

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
   `taxonomy_hint="missed_propagation"` in `scope="prod"` routes to `strict`. This v2
   compatibility decision remains family-level; #852 did not qualify a target-local v3 policy.
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
only for an explicit audit that opts into the fuzzy channel; the documented enforcing mode is
`syntax,semantic`. `base=` is a diff view, not a family list view, so ordinary
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

Run it on a pull request as an opt-in review gate, or first post SARIF annotations without
failing, when a change lands in one copy but not its clones:

```sh
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --fail-on any
# or, for code-scanning results:
nose query . base="origin/${GITHUB_BASE_REF}" --mode syntax,semantic --format sarif top=0 > nose-divergence.sarif
```

Pin `--mode` in CI even though the `base=` default is already conservative; it makes
upgrade diffs explicit. `top=0` is the complete-upload spelling for SARIF. Report-only
findings, including mixed/test scope and `new-copy` current-tree evidence, remain visible
but do not fail the default gate; only `gate.fail_default=true` does.

Base-divergence SARIF results are anchored on the **un-updated sibling** (where the fix
may be missing). GitHub can show inline code-scanning annotations only when the reported
location is in the pull request diff, so a skipped-sibling result may be visible in code
scanning without appearing inline on the Conversation or Files changed tabs. Use the
GitHub Actions examples in [continuous integration](continuous-integration.md#github-actions-rollout-examples)
for a step summary that always reports strict/review/report-only counts and fails only
from `gate.fail_default`.

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
- The hazard ordering is a structural heuristic (~0.6–0.65 on mined divergence labels; see
  [hazard-benchmark](hazard-benchmark.md)). It prioritizes candidates; it does not certify
  them.
