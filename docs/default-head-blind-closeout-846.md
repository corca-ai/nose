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

The post-result reveal replayed the official binary and all 214 sealed candidates,
then reproduced the three 214-case packets and the 90-case arbitration packet byte
for byte. It compiled 147 worthy and 67 not-worthy final decisions; 124 cases were
exact panel agreements and 90 used the frozen arbiter result. The seven plaintext,
mapping, decision, and precision-only component artifacts were added together by
commit `e1bafefe` / tree `cdce9053`, whose exact parent is the independently reviewed
collector `3558f6f6`. The reveal receipt checks the exact commit, parent, tree, path
set, Git modes, blob hashes and lengths, current bytes, full seed/HMAC mapping, all
three raw-vote alignments, arbitration coverage, and final decision/component replay.

## Closeout gates

Only after the judgment component is frozen may the closeout report final dev and
held-out P@10, coverage, full-universe recall, per-language bootstrap intervals,
and the family/surface ledger. It must also record a fresh-repository audit,
all-120 regression and performance against the published v0.19.0 binary,
same-binary controls, `verify --max-violations 0`, thread/run determinism, docs,
and full CI.

Before either field result or all-corpus runtime is observed, two additional inputs
are frozen. The [all-corpus contract](../bench/default_head_closeout_corpus.v1.json) binds
all 120 repository ids and their checked post-prune content digest. The
[fresh-repository selection](../bench/labels/default_head_fresh_repository_selection_2026_07_14.v1.json) binds
one repository and commit for each evaluated language, all outside the pinned corpus.
The field audit reviews the first ten bare-default families in product order;
its discoveries may open follow-up issues but cannot alter this closeout's product or
evidence.

The [v0.19.0 output-drift ledger](../bench/labels/default_head_closeout_v0_19_0.expected-drift.v1.json)
authorizes only the exact all-120 serialization and surface changes already justified
by #842 and #843. Family counts are unchanged. The query-regression checker rejects an
extra, missing, or byte-different declaration, and evaluates runtime independently of
that authorization.

If any #838 threshold fails, #846 records the exact shortfall and preserves the
unchanged product. It does not weaken the threshold or reuse held-out evidence for
ranking, surface, or detector tuning.

## Final result: no-go

The frozen product did **not** satisfy #838. The checked
[closeout artifact](../bench/labels/default_head_closeout_2026_07_14.v1.json) binds
the final evaluation, exact dev overlay, field audit, all-corpus output and runtime
reports, soundness report, and determinism evidence. Its validator recomputes the
exact dev head, replays both query-regression failures, checks every evidence hash,
and rejects a mutated go/no-go conclusion.

| split | bare-default P@10 | label coverage | worthy recall | precision gate |
| --- | ---: | ---: | ---: | --- |
| dev | **387/658 = 58.81%** | 658/658 = 100% | 2716/2849 = 95.33% | fail: 74 hits short |
| held-out | **334/526 = 63.50%** | 526/538 = 97.77% | 2005/2091 = 95.89% | fail: 35 hits short |

The dev point estimate above is the authority: it applies #845's frozen exact-key
top-up to all 658 reported positions. The independently rerun standard evaluator
reports 382/647 = 59.04% and eleven unmatched positions. That row remains a useful
reproducibility cross-check, but it is not silently substituted for the completely
judged dev result. Both dev and held-out intervals use 2,000 deterministic bootstrap
resamples; the aggregate intervals are 55.02–62.46% and 59.32–67.49%, respectively.

| language | dev P@10 (95% CI) | held-out P@10 (95% CI) |
| --- | ---: | ---: |
| C | 40/90 = 44.44% (34.44–55.56) | 28/58 = 48.28% (36.21–60.34) |
| Go | 53/80 = 66.25% (55.00–76.25) | 60/70 = 85.71% (77.14–92.86) |
| Java | 30/90 = 33.33% (24.44–43.33) | 37/60 = 61.67% (50.00–73.33) |
| Python | 49/70 = 70.00% (58.57–80.00) | 49/80 = 61.25% (50.00–72.50) |
| Ruby | 73/90 = 81.11% (73.33–88.89) | 46/60 = 76.67% (65.00–86.67) |
| Rust | 48/78 = 61.54% (51.28–71.79) | 44/70 = 62.86% (51.43–74.29) |
| Swift | 39/80 = 48.75% (37.50–61.25) | 38/58 = 65.52% (53.45–77.59) |
| TypeScript | 55/80 = 68.75% (58.75–78.75) | 32/70 = 45.71% (34.29–57.14) |

The 50% language floor therefore fails for dev C, Java, and Swift, and for
held-out C and TypeScript. Coverage and worthy recall pass. The official v0.19.0
comparison finds 4,721 worthy families in both binaries, with zero recovered and
zero regressed families. Every #842/#843 default-output change is one of 26 exact
ledger entries; there are zero unexpected drifts and family counts are unchanged.

## Fresh-repository audit

The preregistered eight-repository audit reviewed all 80 first-page families and
found 40 worthy: **50.0%**, with repository results ranging from 0/10 to 10/10.
The complete decisions, notes, pinned commits, ordered family IDs, and raw-query
hashes are in the checked [field audit artifact](../bench/labels/default_head_fresh_repository_audit_2026_07_14.v1.json).

Two discoveries are especially actionable but were not allowed to change the
frozen product:

- Picocli was 0/10 because generated static documentation HTML occupied every
  first-page position; Pydantic was 1/10 with eight generated mypy output-snapshot
  families. [#891](https://github.com/corca-ai/nose/issues/891) tracks a
  provenance-based, hard-negative-gated classifier.
- New-repository experience varies much more than the aggregate corpus value.
  Future closeouts must retain field sampling rather than treating aggregate
  precision as a universal first-run claim.

## Soundness, determinism, and performance

`verify crates --max-violations 0` passes: 7,869 units, 1,128 interpretable,
zero false merges, zero canon-preservation violations across 117 canon checks,
and completeness 63/180. Six trace disagreements remain advisory. All 120
repositories produce one repeated-run hash; the 66 dev and 54 held-out repositories
are byte-identical at `RAYON_NUM_THREADS=1` and `4`.

The checked [measurement provenance
manifest](../bench/labels/default_head_measurement_provenance_2026_07_14.v1.json) binds
the raw soundness report, held-out thread matrix, and Ruby scaling report to
the frozen product source and binary, a clean measurement tree, the exact `crates`
Git tree, and the frozen corpus/prune/state digests. The closeout validator resolves
the recorded commits, requires them to be ancestors of the reviewed checkout,
re-hashes every bound artifact and input, and derives the 54 held-out repository IDs
from the corpus manifest rather than trusting the TSV alone.

Performance uses the published v0.19.0 binary and an official/official same-binary
control. The all-120 primary aggregate is safe: 41,819.31 ms → 42,221.93 ms,
raw +0.96%, control-adjusted +515.13 ms / +1.23%. The exact output ledger reports
26 authorized and zero unexpected drifts. After the established 3 → 6 → 9 → 21 →
40 escalation, however, nine material stage signals remain: Alamofire `lower`,
`parse+lower`, `query_gate`, and `query_opp`, plus `query_opp` in Guava, Netty,
RxJava, SQLAlchemy, and SymPy. The r40 aggregate is still safe at +0.46%; the
stage gate is not.

The required seven-repository semantic smoke independently fails its focused run:
its aggregate is safe at +2.23%, but Prettier `discover` is +8.80 ms / +6.71%
and `parse+lower` is +20.55 ms / +12.04% after control. Ruby redefinition scaling
passes at exponent 0.68. [#892](https://github.com/corca-ai/nose/issues/892)
tracks profiling and output-preserving remediation; this closeout does not hide
the regression inside an already-failed quality epic.

The bounded #838 experiment therefore closes as an honest no-go: the unchanged
product preserves recall, soundness, output traceability, and determinism, but misses
both aggregate precision gates, five language-floor cells, and the performance
stage gate. Revealed held-out evidence is not a new tuning set.

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
python3 bench/labels/default_head_heldout_reveal.py freeze \
  --private-panel-dir <outside-repository> \
  --private-arbiter-packet <outside-repository>/arbiter.json
python3 bench/labels/default_head_heldout_reveal.py validate
python3 bench/labels/default_head_heldout_reveal.py self-test
python3 bench/labels/default_head_heldout_reveal_receipt.py validate
python3 bench/labels/default_head_heldout_reveal_receipt.py self-test
python3 bench/labels/default_head_fresh_repository_audit.py
python3 bench/labels/default_head_fresh_repository_audit.py --self-test
python3 bench/labels/default_head_closeout.py
python3 bench/labels/default_head_closeout.py --self-test
```

Ordinary CI validates only public commitments and receipts until the post-arbiter
reveal. The reveal copies the exact precommitted packet bytes, publishes the root
seed and opaque-ID mapping, and reconstructs exact-key decisions without rerunning
judgment. Its public validator binds those bytes to the pre-judgment commitments,
replays every HMAC nonce, permutation, ID, and anonymous vote order, and requires
all three panels and the arbiter to have seen the same source excerpts. The
local publication stages all seven outputs and uses a checked transaction marker
to roll back ordinary failures or recover an interrupted partial write before a
retry. Promotion is no-clobber, rollback never deletes a byte-mismatched path, and
all reserved entries must be regular files rather than symlinks. Public validation
rejects an outstanding marker. Live rollback additionally requires the original
device/inode identity, so an equal-byte replacement is never mistaken for an owned
entry. Git publication remains one atomic artifact commit. The plaintext packets
and root seed remain deliberately unavailable to CI during the blind phase and
become public only after the blind arbitration result is merged.
