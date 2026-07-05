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
```

Use `--max-commits` to keep exploratory runs bounded. Merge commits are skipped
by default because a single parent diff is easier to audit; pass
`--merge-policy first-parent` when that is the intended review model.

## Output

The JSON schema is `nose.divergent_history.v1`. It records:

- run provenance, including the repository, revision range, and nose binary hash;
- per-commit `base=<parent>` summaries and item JSON copied from the normal
  divergent-edit v2 output, including the underlying query JSON schema version;
- grouped findings keyed by lane, base family id, taxonomy hint, and stable site
  identity;
- strict-finding and strict-group counts for triage.

The grouped output preserves the active v2 `strict`, `review`, and `report-only`
vocabulary from [divergent edits](divergent-edits.md). Structured ignores are
applied by the underlying `base=<ref>` run before grouping, so accepted findings
drop out of active history results rather than appearing as repeated groups.
History mining does not change `base=<ref> --fail-on any`: report-only
`new-copy` evidence remains advisory and never becomes a default-failing gate.

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
