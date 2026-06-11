# `nose review` fire-precision benchmark — results (2026-06-11, #243)

The consumer-2 measurement [design](../../docs/design.md) §3 called for: when `nose review`
is used as a PR gate, how often does it fire, and how often is the fire right? Protocol and
numbers below; the experiment narrative lives in
[docs/experiments.md §BR](../../docs/experiments.md).

## Protocol

- **Replay**: for each of 14 pinned corpus repos (7 languages × dev/heldout), sample 25
  first-parent commits whose diff touches ≥ 1 supported-language file with 3–600 changed
  source lines (evenly spaced over the newest ≤ 800 first-parent commits, pool capped at
  200). Check each commit out in a throwaway git worktree and run
  `nose review --base <parent> --format json --top 0` — exactly the PR-gate situation.
- **Arms**: `default` (review's conservative `syntax,semantic` mix) and `near`
  (`--mode syntax,semantic,near`).
- **Labeling unit**: a fired change's **top-ranked finding** (`--fail` is a per-change
  decision and review orders most-likely-unpropagated first). 120 findings stratified
  round-robin by (arm, repo). Lower-ranked findings are unlabeled — a stated limit.
- **Judge**: §BG-gold method — independent judge labels, then **two adversarial refuters
  on every positive**; a positive survives only if both sustain it. Verdict classes:
  `should_propagate` (the gate-positive), `intentional_divergence`, `not_a_clone`,
  `no_propagation_needed` (real clones, but the diff does not touch the shared logic),
  `unclear`.

Reproduce: `cargo build --release` then

```sh
python3 eval/review_fire/replay.py replay --per-repo 25 --out /tmp/rf-replay.jsonl
python3 eval/review_fire/replay.py summarize --records /tmp/rf-replay.jsonl
python3 eval/review_fire/replay.py sample --records /tmp/rf-replay.jsonl --n 120 --out sample.jsonl
```

## Fire rate (change level; 347 replayed changes per arm)

| arm | fire rate | findings/fire p50 | p90 | max | review s p50 | p90 |
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
  equivalent fix (rxjs #7444) citing the same root cause** — review would have caught
  it at the original PR.
- **tokio** `UdpSocket` Debug impl — PR #7675 fixed five identical socket Debug bodies
  and missed the sixth (udp.rs).

## False-fire taxonomy (the #245 gap list)

| class | n | share | read |
|---|---:|---:|---|
| `no_propagation_needed` | 61 | 51% | the diff overlaps the member's **span** but not the **shared logic** — review's overlap test is span-level; requiring overlap with the family's shared/invariant lines targets exactly this bucket |
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
