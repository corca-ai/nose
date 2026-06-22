# Semantic-pack candidate pricing

This directory contains the issue #505 pricing artifacts for narrow
semantic-pack candidate rows.

Run:

```sh
python3 bench/semantic_pack/pricing.py --selftest
python3 bench/semantic_pack/pricing.py
python3 bench/semantic_pack/pricing.py --nose ./target/release/nose --query-sample-repos 1
```

Outputs:

- `candidate_pricing.v1.json` — machine-readable 20-iteration pricing record.
- `candidate_pricing.md` — human-readable summary of the same record.
- `loop_reviews.v1.json` — durable two-reviewer resolution record for the 20
  pricing iterations.

The scanner reports corpus queue signals. It does not prove semantic
correctness and does not authorize broad ecosystem packs. Only `priced-ready`
rows may move to target packets, and those still need normal semantic-pack
fixtures, hard negatives, product-output measurement, runtime notes, and
rollback evidence.

See [semantic-pack-candidate-pricing](../../docs/semantic-pack-candidate-pricing.md).
