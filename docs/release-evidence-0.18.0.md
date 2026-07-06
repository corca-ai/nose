# 0.18.0 release evidence

Generated on 2026-07-06 for the `nose 0.18.0` release candidate.

See the machine-readable [release-0.18.0-evidence-2026-07-06.v1.json](../bench/recall_loss/release-0.18.0-evidence-2026-07-06.v1.json)
artifact for exact commands and measurements.

## Summary

- The pre-release performance pass kept product output identical across all `120`
  pinned corpus repos compared with the immediate pre-pass baseline: `0` hash,
  family-count, or byte-count drifts.
- The retained code change is deliberately bounded: inline `nose-ignore` suppression
  now prescreens candidate `n`/`N` bytes with `memchr` before confirming the
  ASCII-case-insensitive marker. Suppression behavior is unchanged.
- The same-binary all-corpus control measured `54,751.18ms -> 55,825.94ms`
  (`+1.96%`). The retained patch's all-corpus run measured `58,555.18ms ->
  57,323.32ms` (`-2.10%`) with unchanged output, so the runtime effect is small
  and within known run-to-run noise.
- The `v0.17.0` to 0.18.0 release-candidate query regression across all `120`
  repos measured `60,162.97ms -> 56,068.77ms` (`-6.81%`). Four repos changed JSON
  hashes, one changed family count, and two changed byte counts.
- The `crates` recall-loss gate passed with `0` false merges and `0`
  canon-preservation violations.
- Full local CI passed before tagging; see [continuous integration](continuous-integration.md)
  and the release steps in [contributing](contributing.md#releasing).

## Performance Pass

The profiling pass rejected broader changes when their selected-corpus medians failed
to justify the extra code or state:

- value-graph reachable/output sharing preserved output but regressed selected runtime;
- binding-symbol evidence caches preserved output but did not beat noise;
- `named_children` preallocation preserved output but regressed selected runtime;
- value-dag allocation cleanup and borrowed line-diff output stayed neutral or regressed.

The only retained patch is the inline suppression marker prescreen. It is not a broad
algorithmic speedup, but it removes unnecessary whole-marker comparisons on the common
no-marker path and preserved product output exactly in the all-repo regression.

## Release Candidate

The release-candidate all-corpus query regression compared `v0.17.0` with the 0.18.0
candidate before the mechanical version bump:

| Gate | Result |
| --- | ---: |
| repos | `120` |
| iterations / warmups | `3 / 1` |
| aggregate runtime | `60,162.97ms -> 56,068.77ms` |
| aggregate delta | `-6.81%` |
| hash-drift repos | `4` |
| family-count drift repos | `1` |
| byte-count drift repos | `2` |

The hash-drift repos were `date-fns`, `nushell`, `prometheus`, and `redis`.
Only `prometheus` changed family count (`67 -> 66`). `prometheus` and `redis`
changed output bytes. The immediate performance patch, measured separately against
`a7b3dfbe`, had no output drift across the full corpus.

## Recall-Loss Gate

The release candidate passed:

```sh
target/release/nose verify crates --max-violations 0 \
  --recall-loss-report target/release-0.18.0/recall-loss.current.crates.json
```

Summary:

| Metric | Value |
| --- | ---: |
| total units | `7,232` |
| interpretable units | `1,052` |
| canon checked | `104` |
| false merges | `0` |
| canon-preservation violations | `0` |
| admission rejections | `876` |
| completeness | `39/95 = 41.05%` |
