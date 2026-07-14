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

No partial vote enters Git while another reviewer is working. Mapping release is
after arbitration—not merely after panel voting—so the arbiter also remains blind
to product rank and provenance. Reveal validation must reproduce packet hashes,
persona seeds and permutations, the complete official-binary replay, all 1,564
commitments, exact 214-key selection, and exact-key mapping. Fuzzy overlap
propagation is forbidden.

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
```

Ordinary CI validates only public commitments and receipts until the post-arbiter
reveal. The plaintext packet and root seed remain deliberately unavailable to CI
during the blind phase.
