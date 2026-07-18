# Divergent-edit 0.20 closeout

Issue #854 closes the #847 precision-first gate cycle. The result is deliberately
fail-closed: nose ships better review evidence, but the v2 divergent-edit gate remains
explicitly opt-in and is not recommended as a default required check.

## Decision

The #852 development qualification found no runnable v3 policy. Every admissible hard-block
target required a complete, caveat-free semantic witness, but none of the 168 direct targets
in the 80-finding development slice had one. #853 therefore stopped before unsealing the
held-out population. No private repository, change, or quality label was opened, and the seal
remains available for a future independently qualified policy.

The active machine contract is unchanged:

- schema v8 and the existing capability flags remain current;
- `divergent-edit-v2-strict` remains the runtime policy;
- `items[].gate.fail_default` is the only CI failure authority; and
- `review`, `report-only`, suppressed, mixed/test, and `new-copy` findings cannot fail the
  default gate through human, JSON, SARIF, or exit-status paths.

Users still gain concrete review context. Findings now identify detector-accepted direct
changed-to-skipped targets, attach bounded base-to-current semantic-change evidence, and name
pair-local variant or role mismatches. These fields make a finding easier to inspect without
claiming that incomplete evidence proves a missed propagation.

## Performance and compatibility

The release comparison uses the published v0.19.0 macOS arm64 binary (SHA-256
`0f73ea544da06cc175e01c31c383cc4cb86daf3d37a49d74de61dea3724fe0f3`) and one frozen
strict default-arm query in each of 17 development repositories, with three measured runs and
one warm-up. The final candidate measured 10,101.61 ms -> 11,017.30 ms in aggregate. After
subtracting the 32.59 ms same-binary control movement, the regression is 883.10 ms / 8.74%.
That fails #847's 5% budget.

The nearest-rank repository p90 is 58.60%, above the 20% investigation threshold. Profiling
the largest movements showed candidate-local semantic evidence re-lowering and normalizing
selected base/current files: axios spent 128.7 ms in target evidence, while regex spent
240.9 ms in family evidence. Sharing cached projections removes avoidable deep clones, but
does not remove this dominant compatibility cost. Retaining the whole base corpus merely to
avoid selected-file re-lowering was rejected because it couples detection to evidence lifetime
and raises peak memory without a demonstrated budget pass.

All 17 base-query outputs are equal to the official release after removing the reviewed
additive `semantic_change` and `targets` fields. A direct pre-#847-to-closeout comparison also
keeps ordinary non-`base=` `all top=0` JSON byte-identical; this isolates #847 from intentional
v0.19.0-to-main product changes already priced by the 0.20 release evidence.

The [`issue-854 official-v0.19.0 closeout receipt`](../bench/recall_loss/issue-854-official-v0.19.0-closeout-2026-07-18.v1.json)
binds the durable numbers and input hashes.

## Operational checks

The checked CI examples keep capability and base-ref preflights, `top=0`, fork-safe SARIF
upload, upload-before-fail ordering, and a final wrapper decision based only on
`gate.fail_default`. Focused CLI tests cover human/JSON/SARIF agreement, strict exit status,
mixed/test/report-only/new-copy non-failure, suppression, and complete versus truncated
uploads.

The 179-finding development replay completed without errors. Repeating it with one worker in a
fresh set of temporary worktrees produced the same evidence bytes as the parallel run after
removing timing fields. Target IDs are also fixture-tested across reruns and temporary worktree
changes. The closeout does not open a new performance follow-up: a future v3 effort should
start only after target-adjudicated development evidence can qualify a non-degenerate policy,
then price its simpler authoritative path before touching the still-sealed population.
