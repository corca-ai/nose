# Fragment quality audit, 2026-06-10

This audit closes the follow-up from the 18-repo semantic corpus pass in
[field evaluation](field-evaluation.md). That pass showed Java and Python had many
hidden/review exact-fragment families. The question was whether those fragments were
useful diagnostic substrate, acceptable low-value noise for the default surface, or
detector output that should be pruned.

The checked-in data artifact is
[bench/labels/fragment_quality_audit_2026_06_10.json](../bench/labels/fragment_quality_audit_2026_06_10.json).

## Sample

The sample takes the top five hidden/review exact-fragment families, in scan JSON order,
from each of four audited repositories:

| language | repos | candidates |
|---|---|---:|
| Java | `commons-lang`, `retrofit` | 10 |
| Python | `poetry`, `packaging` | 10 |

The source scan cache was `/tmp/nose-semantic-eval-current-517ad5c`, produced with:

```sh
nose scan bench/repos/<repo> --mode semantic --format json --top 0
```

Each candidate was labeled independently by three reviewers using this schema:

| label | meaning |
|---|---|
| `useful_diagnostic` | correct exact fragment with meaningful synchronization, refactor, or review signal |
| `acceptable_low_value` | correct or plausible diagnostic fragment, but too tiny, boilerplate, test-scaffold-like, or common for default action output |
| `noise` | misleading, duplicate bookkeeping artifact, incorrect, or should be pruned rather than merely hidden/review |

## Results

Consensus labels:

| slice | useful diagnostic | acceptable low value | noise |
|---|---:|---:|---:|
| all 20 | 2 | 15 | 3 |
| Java | 1 | 7 | 2 |
| Python | 1 | 8 | 1 |
| original `review` surface | 2 | 9 | 0 |
| original `hidden` surface | 0 | 6 | 3 |

The main read is positive for the semantic kernel: most fragment families were exact and
explainable. They were just not refactoring recommendations. The `review` surface had no
consensus noise in this sample, and the two useful items were exactly the kind of
synchronization signal review output should preserve:

| candidate | label | why it matters |
|---|---|---|
| `java-07` | `useful_diagnostic` | 11-line RxJava2/RxJava3 constructor state initialization mirrors many behavioral flags across adapter modules. |
| `python-03` | `useful_diagnostic` | 8-line Poetry authenticator repository config ties two credential-path tests that should stay aligned. |
| `java-02` | `acceptable_low_value` | test fixture constructor self-field assignments are correct but boilerplate. |
| `python-01`, `python-02`, `python-04` | `acceptable_low_value` | 3-line test setup/assertion `expr-effect` fragments are real but too small for review output. |
| `java-05`, `python-10` | `noise` | both exposed pre-#199 `family_id` collisions with different nearby hidden families. |
| `java-03` | `noise` | one-line direct-return fragments are too generic and can group awkwardly with enclosing methods. |

## Policy

The audit supports the current separation between exactness and product placement:

- Keep exact fragments in full scan JSON. They are useful proof/review substrate for
  integrations, audits, and changed-line review workflows.
- Keep `recommended_surface == "default"` as the human-action filter. Hidden/review
  fragments should not become default findings merely because they are exact.
- Do not broad-prune Java/Python fragments. The dominant class is "correct but low-value",
  not "incorrect".
- Demote tiny test-only scaffold fragments from `review` to `hidden`: all-test exact
  fragments with an enclosing unit and mean span <= 3 lines, plus all-test
  effect/body fragments up to 4 lines. This keeps 3-line arrange/assert snippets and
  fixture constructors diagnostic-only while leaving larger test setup blocks available for
  review.

One follow-up remains outside this narrow policy change. The stable family identity
follow-up was closed by #199: scan JSON IDs now include span and fragment metadata
so distinct hidden fragment families do not share one `family_id`.

- **One-line direct returns:** `exact-direct-return` one-liners are correct too often to
  drop blindly, but the sample shows they need a stricter low-context pruning pass or a
  stronger enclosing/effect boundary before they become useful diagnostics.
