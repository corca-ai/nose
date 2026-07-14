# #846 default-head blind closeout

Issue #846 is the one-shot held-out and field closeout for the #838 default-head
epic. It measures the proposal frozen by #845. Because #845 found no eligible
ranking proposal, the product order remains unchanged; held-out evidence cannot
start another tuning round.

## Frozen boundary

The #840 held-out seal fixes 1,564 candidate commitments and the exact order of
214 selected, previously unmatched families. It contains repository/query hashes,
selection metadata, and candidate-content digests, but no source locations,
judgments, or worthiness labels. Its checked receipt is commit `f9450535`, tree
`78797dee`, and byte SHA-256 `b99c3965…3004`.

The unseal tool fails closed unless all of the following reproduce:

- the seal and its historical collector blobs from Git;
- the published v0.19.0 binary (`0f73ea…0f3`), corpus manifest, v6 labelset,
  rubric, and every pinned held-out repository revision;
- every repository query hash, all 1,564 candidate commitments, the pool totals,
  and the exact 214-key selection order.

The tool must run from a clean commit. Its output records that commit and tree,
the collector blob, inputs, source inventory, and the original seal receipt.

## Blind judgment protocol

The panel packet exposes each selected family and hash-bound source files under an
opaque ID such as `heldout-0001`. Candidate key, product rank, lane, prior-match
state, selection reason/order, and v6-match fields stay hidden. Validation rebuilds
the hidden candidate from the seal commitment and visible family, so changing a
member, value, language, order, or blind ID breaks the original commitment.

After this packet is separately committed, three personas independently apply
[`RUBRIC.md`](../bench/labels/RUBRIC.md). Exact `(worthy, reason)` disagreements
go to a fresh arbiter. The final component maps blind IDs back to the frozen seal
order by exact key only; no fuzzy overlap propagation is permitted.

## Closeout gates

Only after the judgment component is frozen may the closeout report final dev and
held-out P@10, coverage, full-universe recall, per-language bootstrap intervals,
and the family/surface ledger. It must also record a fresh-repository audit,
all-120 regression and performance against the published v0.19.0 binary,
same-binary controls, `verify --max-violations 0`, thread/run determinism, docs,
and full CI.

If any #838 threshold fails, #846 records the exact shortfall and preserves the
unchanged product. It does not weaken the threshold or reuse held-out evidence for
ranking, surface, or detector tuning.
