# Semantic-pack candidate pricing

This directory contains the issue #505 pricing artifacts for narrow
semantic-pack candidate rows.

Run:

```sh
python3 bench/semantic_pack/pricing.py --selftest
python3 bench/semantic_pack/pricing.py --check-artifacts
python3 bench/semantic_pack/pricing.py --nose ./target/release/nose --query-sample-repos 1
```

Use the `--nose ./target/release/nose --query-sample-repos 1` command when
refreshing the committed JSON/Markdown artifacts. Use `--check-artifacts` to
verify committed artifacts, the two-reviewer log, and the issue #509 v2
blocker/matrix artifacts without regenerating them.

Outputs:

- `candidate_pricing.v1.json` — machine-readable 20-iteration pricing record.
- `candidate_pricing.md` — human-readable summary of the same record.
- `loop_reviews.v1.json` — durable two-reviewer resolution record for the 20
  pricing iterations.
- `kernel_capability_matrix.v1.json` — issue #507 primitive census, blocker
  taxonomy, and accept/reject matrix derived from the pricing record.
- `blocker_packet.v2.json` — issue #509 20-probe blocker packet for the larger
  kernel primitive and builtin expansion wave.
- `kernel_capability_matrix.v2.json` — issue #509 accepted primitive,
  still-blocked proof shapes, and rejected unsafe broadening matrix.

The scanner reports corpus queue signals. It does not prove semantic
correctness and does not authorize broad ecosystem packs. Only `priced-ready`
rows may move to target packets, and those still need normal semantic-pack
fixtures, hard negatives, product-output measurement, runtime notes, and
rollback evidence.

See [semantic-pack-candidate-pricing](../../docs/semantic-pack-candidate-pricing.md).
