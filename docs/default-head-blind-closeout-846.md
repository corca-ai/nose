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

Repository-query replay is repeatable mechanical commitment verification; the
one-shot rule applies to judgments and the final decision.

## Rejected packet designs

Two drafts were rejected before voting and were never pushed or merged. They
received no worthiness judgment and produced no metric or product decision.

The first draft exposed a candidate digest, family ID, and seal order that mapped
all 214 rows directly back to the seal. Its self-referential validator also
accepted coordinated source-root and commit/tree rewrites.

The second draft removed those joins, used secret HMAC identifiers and a secret
permutation, and bound the packet to an external Git receipt. Three independent
reviews confirmed that the permutation and receipt worked, then independently
replayed the public candidate universe and joined the exact source excerpts back
to **214/214** candidate keys, repositories, ranks, seal positions, and all 584
source paths without the secret. Exact public source is itself an identifier;
cryptographic IDs cannot make it anonymous.

## Procedurally blind protocol

The accepted design makes a narrower, honest guarantee: reviewers are kept blind
to undisclosed product metadata by procedure. It does **not** claim to hide source
identity from a reviewer who searches remembered or public source. Strong identity
blinding would require a tool-disabled, corpus-inaccessible, network-isolated
review environment that the current shared agent workspace does not provide.

Before any judgment, the coordinator replays the seal once and creates three
persona-specific packets outside both Git and the project workspace. Each persona
gets a separately derived seed, secret permutation, case IDs, source IDs, and
packet nonce. A packet contains only:

- its persona and bound rubric digest;
- language, member count, bounded exact source, and bounded surrounding context;
- an explicit no-lookup/no-contact protocol and required attestation.

It omits candidate keys, repository/path/line, rank, lane, seal order, prior-match
state, selection reason, detector value, witness, surface, scope, extraction shape,
family ID, and all other product-derived signals. Before votes, Git receives only
the seal/input receipts, root-seed commitment, and salted whole-packet byte hashes,
lengths, schemas, and counts. It never receives plaintext packet bytes, per-source
digests, IDs, seeds, or mappings.

Fresh `fork_turns=none` reviewers may read only their assigned packet and the bound
rubric. They attest that they did not inspect Git, corpus repositories, unassigned
files, the network, source identity, another packet, or another vote. This is a
trusted-reviewer policy, not a technical sandbox guarantee.

The checked state transition is:

```text
held-out seal
→ three private packet commitments
→ three raw persona votes frozen together
→ blind-ID arbitration frozen
→ packet, seeds, and exact-key mapping revealed
→ exact-key decisions, metrics, and closeout
```

The frozen v3 commitment is commit `6ec0e95f`, tree `2a7887ae`, and byte
SHA-256 `747b1049…57d6`; its exact parent is the clean collector commit
`37319a18` / tree `03ee6e9c`. The three committed private-packet receipts are
`57d4ddf5…3de3` (dedupe), `5375c63a…6e92` (pragmatic), and
`32db34ec…d3b1` (skeptic). Each binds 214 candidates while revealing no packet
plaintext or opaque IDs.

No partial vote enters Git while another reviewer is working. Mapping release is
after arbitration—not merely after panel voting—so the arbiter also remains blind
to product rank and provenance. Reveal validation must reproduce packet hashes,
persona seeds and permutations, the complete official-binary replay, all 1,564
commitments, exact 214-key selection, and exact-key mapping. Fuzzy overlap
propagation is forbidden.

All three fresh reviewers completed all 214 cases under those attestations before
any vote was opened or copied into the repository. Their raw votes were added
together—and were the only paths added—by commit `1d3add50` / tree `f675ed94`.
The byte receipts are `c3159dd5…97fc` (dedupe), `9abe8c3f…b62`
(pragmatic), and `cfdd7da6…e2d` (skeptic). The panels marked respectively
159, 138, and 143 cases worthy; these are independent raw votes, not final labels.
Exact source identity and candidate mapping remain private until the separate
blind-ID arbitration result is frozen.

The external vote receipt checks the exact commit parent and tree, that exactly
the three vote paths were newly added in the same commit, every frozen and current
byte hash and length, all schemas and packet receipts, four true attestations per
reviewer, 214 unique cases per persona, valid worthy/reason pairs, non-empty
rationales, and zero cross-persona blind-ID overlap. Public CI still cannot prove
the secret permutation; exact ID order remains a private-packet check until reveal.
The arbitration builder consumes the three JSON blobs from that exact atomic Git
commit—not from the working tree or its current `HEAD`—then privately rechecks
their ID order against the original packets. Its public commitment validator binds
the panel commitment, atomic vote commit/tree/parent and file receipts, root-seed
commitment, collector commit/tree/blob, clean freeze state, and private arbitration
packet hash, length, schema, and disagreement count. It also requires the vote
commit to precede the collector commit and the frozen collector bytes to equal the
reviewed current tool, so a self-consistent pre-vote collector cannot assert the
opposite chronology.

The blind arbitration packet was then frozen from clean collector commit
`30aa2d86` and added alone by commit `bf4b54f5` / tree `f7811e8d`. Exactly 90 of
214 cases disagree on `(worthy, reason)`. The private packet is 599,244 bytes with
SHA-256 `b0426488…4005`; the public commitment is 2,175 bytes with SHA-256
`cc891ab8…3b8c`. A full private replay reproduced the official 54-repository
queries, 1,564 candidate commitments, exact 214 selection, all three packet
permutations and votes, and the same 90-case arbiter packet. Source, opaque IDs,
root seed, and mappings remain outside Git until the arbiter result is frozen.

A fresh arbiter then resolved all 90 cases using only that packet and the bound
rubric, with all four required attestations true. The blind result records 55
worthy and 35 not-worthy decisions. It was added alone by commit `e419e48b` /
tree `bdb531d6`; its 17,700 bytes have SHA-256 `1458a26f…7865`. The result remains
keyed only by arbiter blind IDs. Source, root seed, persona identities, candidate
keys, and mappings remain private until this result and its external receipt are
merged, after which reveal is a mechanical replay rather than a new judgment.

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

## Reproduction

The private directory must already exist, be empty, and be outside the repository.
The root seed is read without echo from a terminal.

```sh
python3 bench/labels/default_head_heldout.py freeze \
  --private-dir <outside-repository>
python3 bench/labels/default_head_heldout.py validate
python3 bench/labels/default_head_heldout.py self-test
python3 bench/labels/default_head_heldout.py validate-private \
  --private-dir <outside-repository>
python3 bench/labels/default_head_heldout_panel.py self-test
python3 bench/labels/default_head_heldout_vote_receipt.py validate
python3 bench/labels/default_head_heldout_vote_receipt.py self-test
python3 bench/labels/default_head_heldout_arbitration.py self-test
python3 bench/labels/default_head_heldout_arbitration.py validate
python3 bench/labels/default_head_heldout_arbitration_receipt.py validate
python3 bench/labels/default_head_heldout_arbitration_receipt.py self-test
python3 bench/labels/default_head_heldout_arbitration.py validate-private \
  --private-panel-dir <outside-repository> \
  --private-packet <outside-repository>/arbiter.json
python3 bench/labels/default_head_heldout_arbitration_result.py self-test
python3 bench/labels/default_head_heldout_arbitration_result.py validate-public \
  bench/labels/default_head_heldout_arbitration_result_2026_07_14.heldout.v3.json
python3 bench/labels/default_head_heldout_arbitration_result_receipt.py validate
python3 bench/labels/default_head_heldout_arbitration_result_receipt.py self-test
```

Ordinary CI validates only public commitments and receipts until the post-arbiter
reveal. The plaintext packet and root seed remain deliberately unavailable to CI
during the blind phase.
