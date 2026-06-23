# Semantic pack candidate pricing

Status: operating procedure for issue
[#505](https://github.com/corca-ai/nose/issues/505). This workflow chooses the
next semantic-pack row by measured corpus impact and soundness readiness. It is
not a broad ecosystem adoption plan.

## Principle

Corpus presence is a queue signal, not proof. A popular API surface becomes
implementation work only after it prices as a narrow row with:

- real corpus presence across repos or a clearly recorded no-prevalence result;
- current detector/query context for why the row is not already covered;
- a bounded semantic claim;
- dependency-backed evidence the kernel can check;
- positive fixtures and hard negatives that attack the exact proof invariant;
- a product-output and runtime measurement plan;
- a small rollback path.

The unit of work is a row slice, not an ecosystem. Do not promote "Lodash",
"NumPy", "RxJS", "pandas", "Tokio", "Rails", or "Guava" because the ecosystem
is popular. Promote only a row such as a specific factory, predicate, adapter,
or law whose proof obligations are explicit.

## Verdicts

Each pricing loop records exactly one of these verdicts:

| Verdict | Meaning |
|---|---|
| `priced-ready` | Corpus signal plus existing evidence vocabulary justify a target packet before implementation. |
| `priced-but-blocked` | Impact exists, but a named proof/evidence substrate blocks exact influence. |
| `unpriced` | The candidate has no corpus presence, no current miss value, or no sound row boundary yet. |

`priced-ready` is not permission to implement a broad pack. It means the next
step is a target packet with required evidence, unsupported cases, hard-negative
siblings, product/runtime measurement, and rollback path.

`priced-but-blocked` is useful output. It names the missing substrate so future
work can decide whether to build that proof fact.

`unpriced` is also useful output. It prevents seed-list churn by recording why a
visible or plausible API surface should not be reassessed without new evidence.

## Corpus Tool

Run the pricing tool with:

```sh
python3 bench/semantic_pack/pricing.py --selftest
python3 bench/semantic_pack/pricing.py --check-artifacts
python3 bench/semantic_pack/pricing.py --nose ./target/release/nose --query-sample-repos 1
```

The `--nose ./target/release/nose --query-sample-repos 1` form is the canonical
command for refreshing the committed pricing JSON and Markdown because it
records the sample product-query overlay. The `--check-artifacts` form verifies
that the committed JSON, Markdown, and review log are internally consistent with
the current tool and corpus digest without requiring a release binary. It also
validates the issue #509 v2 blocker packet and capability matrix and the issue
#511 v3/v4 R1-R3 cycle artifacts plus the R4 authorability matrix for count,
cross-reference, accepted-generalization, transition, and performance-gate
consistency.

The tool emits:

- [`candidate_pricing.v1.json`](../bench/semantic_pack/candidate_pricing.v1.json)
- [`candidate_pricing.md`](../bench/semantic_pack/candidate_pricing.md)
- [`loop_reviews.v1.json`](../bench/semantic_pack/loop_reviews.v1.json)

The follow-up primitive census, blocker taxonomy, and accept/reject matrix for
issue #507 are recorded in
[semantic-kernel-capability-minimization](semantic-kernel-capability-minimization.md)
and [`kernel_capability_matrix.v1.json`](../bench/semantic_pack/kernel_capability_matrix.v1.json).
The larger issue #509 expansion uses the same pricing discipline with a second
blocker packet and capability matrix:
[`blocker_packet.v2.json`](../bench/semantic_pack/blocker_packet.v2.json),
[`kernel_capability_matrix.v2.json`](../bench/semantic_pack/kernel_capability_matrix.v2.json),
and
[semantic-kernel-builtin-expansion-509](semantic-kernel-builtin-expansion-509.md).
Issue #511 continues the same loop through repeated R1-R3 cycles before external
pack influence is opened. Its current artifacts are
[`blocker_packet.v3.json`](../bench/semantic_pack/blocker_packet.v3.json),
[`kernel_capability_matrix.v3.json`](../bench/semantic_pack/kernel_capability_matrix.v3.json),
[`blocker_packet.v4.json`](../bench/semantic_pack/blocker_packet.v4.json), and
[`kernel_capability_matrix.v4.json`](../bench/semantic_pack/kernel_capability_matrix.v4.json).
The R4 external authorability pass is recorded in
[`external_authorability_matrix.v1.json`](../bench/semantic_pack/external_authorability_matrix.v1.json).

The current artifact records 20 candidate iterations. It starts from a curated
seed list instead of attempting automatic API discovery. The scanner uses
language-specific regexes plus package/import context where practical. Its
matches are intentionally labeled as pricing evidence, not semantic proof.
When `--nose` and `--query-sample-repos` are provided, the artifact also records
sample product-query summaries, whether the proposed pack id is already
observed in sampled semantic-pack inventory output, and whether current semantic
query families cover the sampled candidate lines.

## Required Fields

Every iteration must record:

- candidate id;
- proposed pack id, when there is a plausible builtin row;
- corpus signal: repo breadth, dev/held-out breadth, primary-language breadth,
  raw occurrences, pattern counts, and sample locations;
- current detector/query result summary;
- impact price;
- safety price;
- required evidence;
- hard-negative siblings;
- unsupported cases;
- verdict and next action.

For `priced-ready`, the iteration must also include a target packet. For
`priced-but-blocked`, it must name the exact missing proof or evidence substrate.
For `unpriced`, it must include enough rejection context to avoid repeated
reassessment without new evidence.

## Review Rule

Issue #505 requires two independent subagent reviews for each pricing loop. The
durable review record is
[`loop_reviews.v1.json`](../bench/semantic_pack/loop_reviews.v1.json). It
preserves, for every iteration:

- two independent reviewer entries with reviewer identifiers;
- per-reviewer challenged categories for verdict, evidence, hard negatives, or
  next action;
- accepted changes;
- rejected feedback with reason.

The final PR should receive three independent whole-work reviews after it is
opened. Valid findings from those reviews should be applied in follow-up commits
before the gates are rerun and the PR is merged.

## Implementation Rule

Only `priced-ready` target packets can feed implementation PRs. A row
implementation still needs the normal semantic-pack adoption gates:

- builtin descriptor and stable pack id;
- dependency-backed evidence;
- positive fixtures and hard negatives;
- `nose semantic-pack inventory`, `adoption-gates`, and `compatibility` checks;
- product query-regression output and runtime notes;
- docs and rollback path.

## Related

- [semantic-pack-ecosystem-candidates](semantic-pack-ecosystem-candidates.md)
- [semantic-pack-architecture](semantic-pack-architecture.md)
- [semantic-pack-adoption](semantic-pack-adoption.md)
- [frontier-platform](frontier-platform.md)
- [adversarial-coevolution](adversarial-coevolution.md)
