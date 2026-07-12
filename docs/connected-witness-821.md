# Connected mapped witnesses (#821)

Issue #821 closes the first bounded tranche selected by the post-#817 missed-worthy
frontier. It adds a pair-local proof for existing `near` candidates whose useful common
computation is connected in source order but is not accepted by the ordinary alignment score.

## What shipped

- Each eligible unit keeps a compact normalized-IL preorder only when connected witnesses are
  enabled. A token records its valued tag, kind, arity, subtree length, and source bounds.
- A witness must be either a whole mapped block or one contiguous statement window below a block
  on both endpoints. It cannot skip an unrelated middle statement or infer A-C from A-B/B-C.
- Control, observable calls/effects, tree arity, order, and direct free callees remain exact. At
  most eight consistently mapped value leaves may vary. Inverse cycles and return-vs-mutation
  mismatches fail closed.
- The ordinary accepted-pair scorer is unchanged. Connected families cannot suppress an existing
  family during ranking or opportunity folding.
- Raw `detect --candidates` evaluates every seed. Product queries bound work to 2,048 ordinary
  misses, eight cross/same-file misses per file, 512 nested seeds globally, and 64 nested seeds
  per file. Output is independently capped at 32 mapped, 32 complete-exit, and 32 nested routes.
- Java retains substantive locally-bound anonymous behavior objects as enclosing-unit evidence,
  without standalone anonymous methods. Inline callbacks and throw-only guards stay opaque.

## Quality result

The checked dev audit promotes all six intended IDs to `accepted-pair`:
`curl:2a436119a08187ba`, `delve:de84b1952aa09a8e`,
`graphhopper:2acc71582f85cc79`, `mockito:d82f6e75097748da`,
`ripgrep:519afdcaed73af0d`, and `thor:07e233ddffad07f0`.

The no-go controls remain `candidate-only`: `gson:4014d594ab6a8e54`,
`scrapy:be72d6b46ad8eaf1`, and `serde_json:946bfa61cb71d562`.

The final combined v6 comparison records **4,684 -> 4,711 worthy hits (+27)** and
**zero regressions**. Held-out source was never inspected; only the evaluator's mechanical
ID/span comparison was used. On the fixed #809 slice, the default surface moves **6,663 ->
6,782 (+119, +1.79%)**. All-family output moves 12,312 -> 12,504 (+1.56%).

## Runtime price

The primary baseline is the official v0.18.0 aarch64 release binary. Five alternating
measurements report **3,612.60 ms -> 3,749.02 ms (+3.78%)**. The HEAD/HEAD control reports
**+0.12%**, about **+3.65% control-adjusted**. A nine-iteration focused rerun of axios, curl,
and nushell reports **+2.26%** primary and **-0.25%** control.

The budget was reached by isolating the ordinary scorer, bounding product work before matching,
preserving exhaustive raw audit mode, and removing per-node child-vector allocations.

## Evidence

- `bench/labels/missed_worthy_stage_audit_issue_821_2026_07_13.dev.v1.json`
- `bench/labels/product_quality_evaluation_issue_821_2026_07_13.v1.json`
- `bench/labels/connected_witness_pricing_2026_07_13.release-primary.v1.json`
- `bench/labels/connected_witness_pricing_2026_07_13.control.v1.json`
- `bench/labels/connected_witness_pricing_2026_07_13.release-focused.v1.json`
- `bench/labels/connected_witness_pricing_2026_07_13.focused-control.v1.json`

## Next decision

After #821 merges, rerun the missed-worthy frontier from the merged tree. Choose the next tranche
from the refreshed measured cohort rather than extending connected witnesses opportunistically.
The expected alternatives remain bounded same-unit fragments and extraction gaps.
