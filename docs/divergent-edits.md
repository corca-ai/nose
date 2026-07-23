# Catch missed sibling edits

`nose query <path> base=<ref>` finds clone families where a change touched one
copy but skipped a sibling. It is useful after fixing duplicated code: the
untouched copy may need the same fix.

This view requires a Git repository because it compares the working tree with a
base ref. It is a review aid, not proof of a bug; the copies may have diverged
intentionally.

## Quick start

```sh
# Review uncommitted changes
nose query . base=HEAD

# Review a branch against its merge target
nose query . base=origin/main
```

A finding looks like this:

```text
9f2c1a  similar · prod · base-divergence · strict
  changed:      src/fs.rs:88-95  normalize_path
  not updated:  src/router.py:212-220  clean_route
```

Open the location under `not updated` and decide whether the edit belongs there
too. If not, use a [structured ignore](structured-ignores.md) to record the
intentional difference.

## How it works

1. nose reads the changed lines from `git diff <base>`.
2. It detects clone families in the base revision, before the edit could change
   a copy enough to hide the relationship.
3. It reports a family when some members overlap changed lines and other members
   were not updated.
4. It orders the strongest missed-propagation candidates first.

All copies updated is a consistent change and is not reported. No copies updated
is irrelevant to the diff.

## Report and gate levels

The report contains more evidence than the optional CI gate:

| Level | Meaning | Fails `--fail-on any`? |
|---|---|---|
| `strict` | Production finding with proof that the edit touched shared logic | Yes, when `gate.fail_default=true` |
| `review` | Plausible sibling-edit candidate without the strict gate proof | No |
| `report-only` | Advisory context such as test or newly copied code | No |
| `suppressed` | Accepted by a structured ignore | No; omitted from active output |

The strict policy is intentionally conservative, but it is still a candidate
review gate rather than a correctness proof. Its checked development baseline
had 45 true positives among 80 strict findings. Start observe-only, review the
noise in your repository, and enable failure only when the signal is useful.

Machine consumers should use `items[].gate.fail_default`; do not re-create the
policy from labels or scores. The detailed schema, measurements, and
qualification history are in the [policy and qualification record](divergent-edits-policy.md).

## Flags and output

The `base=` view shares the main detection options such as `--mode`,
`--min-size`, `--exclude`, and `--config`. It deliberately rejects ordinary
family-list filters and baselines because this is a diff view.

| Option | Effect |
|---|---|
| `base=<ref>` | Compare the working tree with `HEAD`, `origin/main`, or another Git ref |
| `--mode syntax,semantic` | Pin the conservative CI detection surface |
| `--fail-on any` | Exit non-zero when an unsuppressed item has `gate.fail_default=true` |
| `--format human\|json\|markdown\|sarif` | Choose human or machine output |
| `--ignore-file <file>` | Use a structured ignore file |
| `top=N` | Show at most N findings; `top=0` emits all |

Without `--mode`, this view defaults to `syntax,semantic`. Add `near` only for an
explicit audit that accepts more similarity-based candidates.

## Suppress an intentional divergence

For a strict finding, either update the skipped sibling or commit a narrow ignore
when the difference is intentional:

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

nose auto-reads `nose.ignore.json` from the current directory, or you can pass
`--ignore-file`. See [structured ignores](structured-ignores.md) for selectors,
expiry, and auditing.

## Add it to CI

Begin with a non-blocking SARIF or step-summary workflow. When reviewed results
justify enforcement:

```sh
nose query . \
  base="origin/${GITHUB_BASE_REF}" \
  --mode syntax,semantic \
  --fail-on any
```

For code scanning:

```sh
nose query . \
  base="origin/${GITHUB_BASE_REF}" \
  --mode syntax,semantic \
  --format sarif \
  top=0 > nose-divergence.sarif
```

Skipped siblings may be outside the pull-request diff, so GitHub can retain a
code-scanning result without showing an inline annotation. The
[CI guide](continuous-integration.md#divergent-edits-in-pull-requests) includes
rollout examples and a step summary.

## Limits and history

- The command checks one diff: the base ref against the current working tree.
- It detects the main families in the base revision. A bounded report-only lane
  can surface a newly added copy in small diffs.
- Ranking prioritizes review; it does not certify that propagation is required.
- For a bounded offline audit across older commits, use
  [divergent history mining](divergent-history-mining.md), not PR-time CI.
