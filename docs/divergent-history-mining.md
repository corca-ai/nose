# Divergent History Mining

`nose query <path> base=<ref>` is the PR-time divergent-edit gate. It answers
"did this diff change one clone copy while skipping its siblings?" for one
working tree against one base ref.

History mining is the offline version for maintainers who want to inspect a
bounded commit range after the fact. The maintained harness is the [`scripts/divergent-history-mining.py`](../scripts/divergent-history-mining.py) script.
It checks out each selected commit in a temporary git worktree, runs the normal
`base=<parent>` JSON view with `top=0`, and groups repeated findings so the same
long-lived skipped sibling does not become a separate review item for every
commit.

## Example

```sh
cargo build --release -p nose-cli
python3 scripts/divergent-history-mining.py \
  --range "origin/main..HEAD" \
  --path . \
  --mode syntax,semantic \
  --nose target/release/nose \
  --output target/divergent-history.json

python3 scripts/divergent-history-mining.py \
  --check-artifact target/divergent-history.json
```

Use `--max-commits` to keep exploratory runs bounded. Merge commits are skipped
by default because a single parent diff is easier to audit; pass
`--merge-policy first-parent` when that is the intended review model.

## Output

The JSON schema is `nose.divergent_history.v1`. It records:

- run provenance, including the repository, revision range, and nose binary hash;
- hardened revision-2 provenance for new artifacts, including the exact argv,
  script hash, nose version, dirty tracked-file state, bounded/offline metadata,
  and source-redaction policy;
- per-commit `base=<parent>` summaries and item JSON copied from the normal
  divergent-edit v2 output, including the underlying query JSON schema version;
- grouped findings keyed by lane, base family id, taxonomy hint, and stable site
  identity;
- strict, review, report-only, lane, taxonomy, and default-failing counts for
  triage.

The grouped output preserves the active v2 `strict`, `review`, and `report-only`
vocabulary from [divergent edits](divergent-edits.md). Structured ignores are
applied by the underlying `base=<ref>` run before grouping, so accepted findings
drop out of active history results rather than appearing as repeated groups.
History mining does not change `base=<ref> --fail-on any`: report-only
`new-copy` evidence remains advisory and never becomes a default-failing gate.

## Maintainer Grouped Review Workflow

Review `groups[]` first. Each group has one `representative` occurrence for the
main decision surface and an `occurrences[]` list for commit provenance. That
keeps a long-lived skipped sibling from becoming a new review task every time it
appears in the selected history range.

Record disposition separately from the artifact. Use buckets such as
`should-propagate`, `intentional-variant`, `no-propagation-needed`,
`test-scaffolding`, `grouping-artifact/not-a-clone`, and `unclear`. A strict
finding means "would fail under the opt-in enforcing workflow," not "must block"
or "is ground truth." If the team accepts a strict finding as intentional, add a
structured ignore and rerun the same bounded command with `--ignore-file` so the
group drops out of active history output.

Do not run this script in PR-time CI. PR workflows should use the normal
observe-only or enforcing examples from [continuous integration](continuous-integration.md)
and make decisions from `items[].gate.fail_default`.

## Checked Pilot

The #687 maintainer pilot is recorded in the checked
[divergent history pilot](divergent-history-mining-pilot-687.md). It validates
the checked history artifact, records a local observe-only pilot, and explicitly
keeps the result as opt-in evidence rather than a default-on readiness claim.

## Suppression Workflow

History-mined findings are inspection candidates, not proof of a bug. When a
finding is intentional, suppress it in the same way as a PR-time divergent edit.
For post-hoc audits over old commits, prefer passing the current audit file with
`--ignore-file`; otherwise the temporary worktree can only auto-read
`nose.ignore.json` as it existed in each historical commit. The next run will
reuse the underlying `base=<ref>` suppression behavior and the grouped item will
drop out of the active results.

## Limits

- The harness is intentionally bounded and read-only; it does not patch code.
- It replays each selected commit against its first parent, so root commits and
  skipped merge commits are reported as skipped rows.
- Stable grouping is based on family/site identity. Large rewrites that change
  family ids or symbol names may appear as separate groups and should be
  reviewed manually.
