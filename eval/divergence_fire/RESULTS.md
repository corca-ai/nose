# Divergent-edit fire-precision benchmark — results (2026-06-11 #243, 2026-07-06 #670-#675)

The consumer-2 measurement [design](../../docs/design.md) §3 called for: when query base
is used as a PR gate, how often does it fire, and how often is the fire right? Protocol and
numbers below; the experiment narrative lives in
[docs/experiments.md §BR](../../docs/experiments.md).

## Protocol

- **Replay**: for each of 14 pinned corpus repos (7 languages × dev/heldout), sample 25
  first-parent commits whose diff touches ≥ 1 supported-language file with 3–600 changed
  source lines (evenly spaced over the newest ≤ 800 first-parent commits, pool capped at
  200). Check each commit out in a throwaway git worktree and run
  `nose query . base=<parent> top=0 --format json` — exactly the PR-gate situation.
- **Arms**: `default` (conservative `syntax,semantic` mix) and `near`
  (`--mode syntax,semantic,near`).
- **Labeling unit**: a fired change's **top-ranked finding** (`--fail` is a per-change
  decision and query base ranks most-likely-unpropagated first). 120 findings stratified
  round-robin by (arm, repo). Lower-ranked findings were unlabeled in the 2026-06-11
  baseline — a stated limit that #670 addresses below.
- **Judge**: §BG-gold method — independent judge labels, then **two adversarial refuters
  on every positive**; a positive survives only if both sustain it. Verdict classes:
  `should_propagate` (the gate-positive), `intentional_divergence`, `not_a_clone`,
  `no_propagation_needed` (real clones, but the diff does not touch the shared logic),
  `test_scaffolding`, `unclear`.

Reproduce the checked 2026-06-11 artifacts and current harness invariants:

```sh
python3 eval/divergence_fire/replay.py selftest
python3 eval/divergence_fire/replay.py check-artifacts
```

Re-run the historical rank-prioritized sampling protocol with `cargo build --release` then

```sh
python3 eval/divergence_fire/replay.py replay --per-repo 25 --out /tmp/df-replay.jsonl
python3 eval/divergence_fire/replay.py summarize --records /tmp/df-replay.jsonl
python3 eval/divergence_fire/replay.py sample --records /tmp/df-replay.jsonl --n 120 --findings-per-change 0 --out sample.jsonl
python3 eval/divergence_fire/replay.py policy-eval --samples sample.jsonl --verdicts verdicts.jsonl --out policy_eval.json
```

For the #670 / divergent-edit v2 refresh, also keep a strict top-1 comparison sample:

```sh
python3 eval/divergence_fire/replay.py sample --records /tmp/df-replay.jsonl --n 120 --findings-per-change 1 --sid-prefix rv2t1 --out sample.top1.jsonl
python3 eval/divergence_fire/replay.py policy-eval --samples sample.top1.jsonl --verdicts verdicts.top1.jsonl --out policy_eval.top1.json
```

Then price lower-ranked gate findings from the full all-findings selected pool:

```sh
python3 eval/divergence_fire/replay.py sample --records /tmp/df-replay.jsonl --n 0 --findings-per-change 0 --sid-prefix rv2 --out sample.all-findings.jsonl
python3 eval/divergence_fire/replay.py policy-eval --samples sample.all-findings.jsonl --verdicts verdicts.all-findings.jsonl --out policy_eval.all-findings.json
```

For checked #670 policy reproduction, keep only labeled rows and redact source excerpts:

```sh
jq -cr --slurpfile verdicts <(jq -s 'map(.sid)' eval/divergence_fire/verdicts_2026_07_06.jsonl) 'select(.sid as $sid | $verdicts[0] | index($sid))' sample.top1.jsonl sample.all-findings.jsonl | jq -s -c 'unique_by(.sid)[]' > sample.labeled.jsonl
python3 eval/divergence_fire/replay.py redact-sample --samples sample.labeled.jsonl --out eval/divergence_fire/sampled_findings_2026_07_06.jsonl
python3 eval/divergence_fire/replay.py policy-eval --samples eval/divergence_fire/sampled_findings_2026_07_06.jsonl --verdicts eval/divergence_fire/verdicts_2026_07_06.jsonl --out eval/divergence_fire/policy_eval_2026_07_06.json
```

## Sealed precision-first protocol (2026-07-14, #848)

Further gate changes use the checked
[`precision_protocol_2026_07_14.v2.json`](precision_protocol_2026_07_14.v2.json).
It freezes measurement and population before implementation work can inspect any new
quality result. The 28 repositories and 179 labels from the 2026-07-06 refresh are
**development-only**. They reproduce the v2 baseline at 80 strict findings, 45 true
positives, 35 false positives, precision 45/80 = 0.5625 (reported 0.562), and a
one-sided 95% Wilson lower bound of 0.4707.

The corrected v2 seal supersedes the unused v1 draft after three independent reviews
found a support-unit mismatch, an incomplete temporal rule, lossy Git collection, weak
private-file permissions, and an incorrect release-asset suffix. No v1 quality verdict
was created or revealed. The fresh v2 blind population is repository-disjoint from
development data: four
repositories for each of C, Go, Java, Python, Ruby, Rust, and TypeScript, or 28
repositories total. Up to 40 eligible first-parent changes were frozen per repository,
yielding 1,120 changes. A separate 28-repository temporal-canary reserve, also four per
language, becomes eligible only after the seal. Its 1,000 changes follow a sealed future
selection rule described below. The primary arm remains `syntax,semantic`;
`syntax,semantic,near` is advisory and cannot decide the gate.

The evaluation reports three precision units:

- target precision: `should_propagate` direct changed-member-to-skipped-sibling targets
  divided by all adjudicated strict targets;
- finding precision: positive strict family findings divided by all adjudicated strict
  findings; and
- change precision: changes with at least one `should_propagate` strict target divided
  by changes with at least one strict target.

All three units report one-sided 95% Wilson score bounds (`z = 1.6448536269`).
For the blind policy gate, strict-target precision must be at least 0.95, its Wilson
lower bound at least 0.90, with at least 100 distinct strict findings, at least 100
targets, and 20 complete repositories. A default-on claim further
requires change-block precision at least 0.99, its Wilson lower bound at least 0.95,
at least 20 repositories and 1,000 temporal changes, and zero confirmed false required
blocks. Finding and change precision are always reported.

Blind sampling is repository-atomic. In secret HMAC order, replay and adjudicate every
selected change and every emitted strict finding and target from the next complete
repository. Stop only after a complete repository brings cumulative support to at
least 100 distinct strict findings, 100 targets, and 20 complete repositories. A replay
error remains counted and is never replaced. If all 28 blind repositories are exhausted
before all three support minima are met, the result is
`insufficient-evidence`, not a relaxed sample. The allowed final classifications are
`default-on-ready`, `improved-opt-in-only`, `failed`, and `insufficient-evidence`.

Temporal selection is also fixed before blind results exist. At days 30, 60, 90, 120,
150, and 180 after the seal, a checkpoint must atomically commit all 28 repositories'
advertised default refs and heads, capture times, command provenance, and errors before
any nose replay or verdict. Each head must descend from its sealed head. From that
first-parent range, apply the same supported-path and 3–600 changed-source-line bounds,
then take at most 40 changes per repository in secret HMAC order. The temporal sample is
every selected change from every reserve repository at the earliest checkpoint totaling
at least 1,000; no checkpoint through day 180 means `insufficient-evidence`. Identity,
ancestry, selection, or checkpoint errors invalidate the evaluation; query errors remain
counted and cannot be replaced.

No held-out quality labels exist at this stage. Raw repository/commit identities,
source-bearing diffs, and the HMAC seed remain in an external `0700` directory whose
seed, packet, and manifest are each created as `0600`. Git stores
only opaque repository/change IDs, row commitments, source-free provenance, population
counts, and the private packet's byte length and SHA-256. Two independent reviewers per
opaque target and a resolver for disagreement must seal verdicts before identities can
be revealed. This keeps implementation issues from tuning against blind outcomes.

The public validator freezes the v0.19.0 release binary identity, corpus and prune
manifest, replay harness, collector, exact freeze command, Git version/config/locale,
raw-byte diff encoding, source-redaction boundary, stop rule, temporal sampling, verdict
rubric, thresholds, and opaque population. The `.tar.xz` release archive and extracted
binary have separate SHA-256 identities. Collection disables ambient system/global Git
config and pins rename, diff, text-conversion, locale, and path-byte handling; a Git
selection failure aborts the freeze instead of silently choosing a replacement. The
history-bound receipt additionally pins
the exact artifact commit, parent, tree, Git blobs, file bytes, public checksum, seed
commitment, private-packet commitment, and population counts. Consequently the #848
branch must be integrated with a true Git merge commit; squash or rebase would remove
the frozen artifact commit from ancestry and deliberately fail validation.

Reproduce the public protocol and the development baseline:

```sh
python3 eval/divergence_fire/precision_protocol.py validate
python3 eval/divergence_fire/precision_protocol.py self-test
python3 eval/divergence_fire/precision_protocol_receipt.py validate
python3 eval/divergence_fire/precision_protocol_receipt.py self-test
python3 eval/divergence_fire/replay.py selftest
python3 eval/divergence_fire/replay.py check-artifacts
python3 eval/divergence_fire/replay.py policy-eval \
  --samples eval/divergence_fire/sampled_findings_2026_07_06.jsonl \
  --verdicts eval/divergence_fire/verdicts_2026_07_06.jsonl \
  --out /tmp/divergent-v2-development-policy.json
```

An authorized custodian can also reproduce the private projection without exposing it
in Git:

```sh
python3 eval/divergence_fire/precision_protocol.py validate-private \
  --private-dir <external-private-dir> --repos-root bench/repos
```

## Refresh run (2026-07-06, #670-#675)

The first v2 replay refresh broadened the corpus sample to 28 repos and 10 commits
per repo, keeping both historical arms. The durable summary is
[`replay_summary_2026_07_06.json`](replay_summary_2026_07_06.json). Raw replay JSONL
and sampled judging packets remain scratch artifacts because they embed source excerpts
and diffs. The checked redacted sample
[`sampled_findings_2026_07_06.jsonl`](sampled_findings_2026_07_06.jsonl) contains
the source-free fields required to recompute the policy artifact.

| arm | replays | errors | fire rate | findings | findings/fire p50 | p90 | divergence s p50 | p90 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| default (`syntax,semantic`) | 280 | 0 | **31.8%** | 209 | 1 | 5 | 2.33 | 8.84 |
| near (`syntax,semantic,near`) | 280 | 0 | **39.6%** | 274 | 1 | 6 | 2.45 | 10.31 |

The initial #670 run refreshed the labelset to cover the full strict top-1
sample plus every lower-ranked fire-eligible finding in the all-findings selected
pool. Verdicts are in
[`verdicts_2026_07_06.jsonl`](verdicts_2026_07_06.jsonl), with policy simulation
in [`policy_eval_2026_07_06.json`](policy_eval_2026_07_06.json).

| slice | labeled | should-propagate | precision if all fired |
|---|---:|---:|---:|
| top-1 strict sample | 120 | 21 | 17.5% |
| lower-ranked fire-eligible findings | 59 | 28 | 47.5% |
| combined labeled set | 179 | 49 | 27.4% |

Overall verdict counts are: 49 `should_propagate`, 46 `test_scaffolding`, 40
`no_propagation_needed`, 26 `intentional_divergence`, and 18 `not_a_clone`.
The lower-ranked fire-eligible slice remains materially richer than top-1
(28/59 vs 21/120), so policy work must not optimize only for rank 0. On this
complete labeled set, the serialized `fire_eligible` policy fires on 94 findings
with 45 true positives and 49 false positives (precision 0.479). The #672 v2 strict
policy (`gate.fail_default=true`, equivalent to `tier=strict`) fires on 80 findings
with the same 45 true positives and 35 false positives (precision 0.562), so it
preserves every confirmed v1 missed-propagation catch while cutting labeled strict
fires by 14.9%.

#673 adds a bounded `new-copy` report-only lane for current-tree clone evidence from
small added/copied/renamed source diffs. The lane is capped at two touched source
files so broad PRs do not pay a second full current-tree detection pass. Focused
fixtures cover added-copy and moved-copy positives plus an unrelated-added negative.
For runtime, a same-environment #672 rerun on the 560-record replay measured
default p50/p90 2.53s/9.52s and near p50/p90 2.70s/10.66s. The #673 budget=2
replay measured default 2.38s/9.55s and near 2.43s/10.37s, also with 0 errors.
The fixed replay sample had no budget-eligible real-world `new-copy` row after the
cap; the lane's product contract is pinned by the focused JSON/SARIF fixtures while
the replay checks the no-runtime-regression constraint.

The #675 closeout replay reran the same 28-repo, 560-record replay after the
v2 strict gate, bounded `new-copy` lane, SARIF/output polish, and docs updates.
It completed with 0 errors. The durable closeout summary is
[`replay_summary_final_head_a38ecb8b_2026_07_06.json`](replay_summary_final_head_a38ecb8b_2026_07_06.json),
and the final policy reproduction is
[`policy_eval_final_head_a38ecb8b_2026_07_06.json`](policy_eval_final_head_a38ecb8b_2026_07_06.json).
Scratch raw records are intentionally not checked in because they contain source
excerpts; the closeout summary records the raw path and sha. The closeout
commands were:

```sh
cargo build --release -p nose-cli
python3 eval/divergence_fire/replay.py replay \
  --repos git redis curl hugo minio cobra prometheus netty rxjava guava gson scrapy sympy black requests rubocop sidekiq devise clap tokio regex fd jest rxjs prettier axios date-fns execa \
  --per-repo 10 --jobs 6 --timeout 240 \
  --out /tmp/nose-675/divfire-final-head-a38ecb8b-2026-07-06.raw.jsonl
python3 eval/divergence_fire/replay.py summarize \
  --records /tmp/nose-675/divfire-final-head-a38ecb8b-2026-07-06.raw.jsonl \
  --out /tmp/nose-675/replay_summary_final_head_a38ecb8b_2026_07_06.json
python3 eval/divergence_fire/replay.py policy-eval \
  --samples eval/divergence_fire/sampled_findings_2026_07_06.jsonl \
  --verdicts eval/divergence_fire/verdicts_2026_07_06.jsonl \
  --out /tmp/nose-675/policy_eval_final_head_a38ecb8b_2026_07_06.json
```

The final replay summary records clean source commit `a38ecb8b`.

| arm | replays | errors | fire rate | strict-firing changes | findings | tier counts | divergence s p50 | p90 |
|---|---:|---:|---:|---:|---:|---|---:|---:|
| default (`syntax,semantic`) | 280 | 0 | 31.8% | 32 | 209 | report-only 105, review 62, strict 42 | 2.21 | 8.97 |
| near (`syntax,semantic,near`) | 280 | 0 | 39.6% | 43 | 274 | report-only 118, review 90, strict 66 | 2.40 | 11.00 |

The checked policy inputs still reproduce the v2 strict result: 80 strict fires,
45 true positives, 35 false positives, precision 0.562. That retains all 45/45
confirmed positives from the serialized v1 `fire_eligible` slice while reducing
labeled default-failing findings from 94 to 80.

#684 audited the checked labelset before any further strict-policy tightening.
The policy artifact now records derived v2 `gate.fail_default` evidence, tier and
taxonomy confusion counts, and the strict precision floor. The audit found no
safe no-tradeoff policy cut: simple filters such as dropping structural-similarity,
requiring `similarity == 1.0`, or limiting to copy-paste witnesses all improve
precision but lose confirmed positives. The retained strict false positives are
17 `no_propagation_needed`, 13 `intentional_divergence`, and 5 `not_a_clone`.
Because the checked evidence does not identify a deterministic classifier that
removes those rows while retaining all 45 serialized-fire-eligible positives, the
runtime strict gate remains unchanged.

Runtime did not show a confirmed degradation. Against the same-environment #672
strict-gate replay, default p50/p90 moved 2.53s/9.52s -> 2.21s/8.97s and near
p50/p90 moved 2.70s/10.66s -> 2.40s/11.00s. Against the durable #670 refresh
summary, default p50/p90 moved 2.33s/8.84s -> 2.21s/8.97s and near p50/p90 moved
2.45s/10.31s -> 2.40s/11.00s. A same-binary replay control on the final binary
measured default p50/p90 2.27s/9.00s and near p50/p90 2.32s/10.85s, so the
small p90-tail increases are within replay noise. A separate non-`base=`
product-output regression over 10 corpus repos and 5 iterations compared
`origin/main` to the closeout binary: aggregate median 5957.65ms -> 5930.96ms
(-0.45%), with identical product hashes, family counts, and output byte counts
for all 10 repos. A same-binary control over the same repos and iterations
measured 5943.05ms -> 6012.94ms (+1.18%).

The remaining false-positive buckets under serialized `fire_eligible` are:

| bucket | false positives | v2 implication |
|---|---:|---|
| `no_propagation_needed` | 17 | tighten shared-line/changed-logic overlap beyond span-level contact |
| `intentional_divergence` | 13 | add variant-aware signals or ignore ergonomics |
| `test_scaffolding` | 12 | keep test/scope filtering prominent in default policy |
| `not_a_clone` | 7 | improve family grouping quality for broad low-specificity matches |

## Fire rate (change level; 347 replayed changes per arm)

| arm | fire rate | findings/fire p50 | p90 | max | divergence s p50 | p90 |
|---|---:|---:|---:|---:|---:|---:|
| default (`syntax,semantic`) | **33.1%** | 1 | 4 | 38 | 2.9 | 6.8 |
| near (`syntax,semantic,near`) | **41.2%** | 1 | 5 | 33 | 3.4 | 7.7 |

## Fire precision (top-1 finding, judge-labeled, refuter-confirmed; n=120)

| slice | n | confirmed should-propagate | precision |
|---|---:|---:|---:|
| overall | 120 | 5 | **4.2%** |
| arm = default | 65 | 2 | 3.1% |
| arm = near | 55 | 3 | 5.5% |
| similarity = 1.0 | 99 | 4 | 4.0% |
| similarity < 1.0 | 21 | 1 | 4.8% |

The five confirmed positives are **three unique divergences** (two were sampled by both
arms), each independently validated by the refuters against upstream:

- **rubocop** `DataInheritance#correct_parent` — byte-identical autocorrect helper;
  the parentheses bug fixed in one copy genuinely applies to the other (still latent
  on rubocop master at audit time).
- **rxjs** `AnimationFrameAction` — the `id === scheduler._scheduled` guard added to
  `AsapAction` was missing in the identical sibling; **upstream later merged the
  equivalent fix (rxjs #7444) citing the same root cause** — query base would have caught
  it at the original PR.
- **tokio** `UdpSocket` Debug impl — PR #7675 fixed five identical socket Debug bodies
  and missed the sixth (udp.rs).

## False-fire taxonomy (the #245 gap list)

| class | n | share | read |
|---|---:|---:|---|
| `no_propagation_needed` | 61 | 51% | the diff overlaps the member's **span** but not the **shared logic** — the old overlap test was span-level; requiring overlap with the family's shared/invariant lines targets exactly this bucket |
| `intentional_divergence` | 38 | 32% | async/sync, platform, version variants; per-member specializations — needs structured-ignore ergonomics and/or a variant-awareness signal, not a threshold |
| `not_a_clone` | 15 | 12% | grouping artifacts, scaffolding; concentrated in low-similarity and large-block families |
| `unclear` | 1 | 1% | split refuter panel |

## Honest read

- A **33–41% fire rate on merged PRs with ~4% top-1 precision is not a shippable
  default-on gate** — design §2's "a gate that cries wolf gets disabled" is now a
  measured fact, not a fear, and `--fail` should stay an explicitly-opted, policy-tuned
  gate until #245 lands.
- The signal is real: three genuine un-propagated changes in 350 replayed merged
  changes, one later fixed upstream for exactly the predicted reason. The gate problem
  is **dilution, not absence** — and half the dilution is one mechanical bucket
  (span-level overlap), which is a fixable policy, not a judgment-deep wall.
- Sample limits: top-1 findings only; 14 repos; merged-PR replay measures the
  *surviving* change stream (changes blocked before merge are invisible).
