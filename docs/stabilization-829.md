# Stabilization #829 Closeout

Issue [#829](https://github.com/corca-ai/nose/issues/829) is the bounded
documentation, test, code-quality, and performance pass completed before
resuming #821. It does not change detector thresholds, ranking, classification,
or candidate admission.

## Outcome

- The official v0.18.0 Darwin arm64 release asset is the performance baseline.
- Current head is 4.61% faster in aggregate on the checked seven-repository
  product slice while reporting 464 additional families.
- Release and head same-binary controls moved -0.74% and +0.71% respectively.
- The stabilization change preserves all seven product-output hashes.
- The substantial self-duplication ratchet tightened from 30 families to 29.
- No production optimization was accepted because no reproducible product
  regression exists.

## Release Baseline

The baseline is the published `nose-cli-aarch64-apple-darwin.tar.xz` asset from
v0.18.0, not a local rebuild of the tag. Its archive SHA-256 is
`e4b1d073...d01a46`; the extracted binary SHA-256 is `aa22823c...03a040`.
The
[`issue_829_stabilization_closeout.v1.json`](../bench/recall_loss/issue_829_stabilization_closeout.v1.json) closeout
artifact records the complete identities and commands.

The alternating five-iteration comparison covers axios, curl, netty, nushell,
prometheus, rich, and rubocop:

| measurement | baseline | current | delta |
| --- | ---: | ---: | ---: |
| official v0.18.0 → head | 3,710.35ms | 3,539.32ms | -4.61% |
| v0.18.0 same-binary control | 3,755.84ms | 3,728.00ms | -0.74% |
| head same-binary control | 3,576.26ms | 3,601.49ms | +0.71% |

The release reports 11,848 families and head reports 12,312. The larger product
surface therefore does not hide a release-relative slowdown in this slice.

## Binary Identity Finding

The first pre/post check reported a stage-level runtime signal even though the
only Rust source edit was inside a `cfg(test)` module. The two release binaries
had equal size, equal `__TEXT` and `__DATA`, and only 47 differing bytes: the
16-byte Mach-O `LC_UUID` plus bytes in the derived ad-hoc code signature.

`scripts/binary_identity.py` now records both identities:

- full-file SHA-256 for exact artifact provenance;
- code SHA-256 with Mach-O UUID/signature bytes zeroed for regression identity.

The pre-stabilization and final binaries have different full-file hashes but the
same normalized code hash, `7eec9818...538a8`. The query-regression checker now
correctly treats them as code-identical; all seven output hashes match and the
gate passes. Non-Mach-O binaries retain their full-file hash as the code hash.

## Test and Code Cleanup

The warm workspace run executed 1,953 tests successfully. Test bodies account
for roughly 10.42 seconds; the 84.41-second command wall time is dominated by
build and link work. Tests were therefore not combined, weakened, or replaced
with mocks for timing.

One real fixture boundary was improved. Three Rust
`Map.get(...).unwrap_or(...)` evidence tests now share only their IL/evidence
construction while keeping separate observable assertions for:

- valid receiver dependency and builtin kind;
- nested map-get arity drift;
- map proof attached to an unrelated receiver.

This removes 34 lines and dogfood family `85074f64d038d1a0`, tightening the
duplication budget to 29.

## Rejected Cleanup

- Code and Markdown Union-Find implementations remain separate because their
  policies and dependency directions differ.
- Files were not split merely because they approach the 599-line ratchet.
- Criterion's cached local change percentages were not treated as product
  evidence; final diagnostics are about 1.72ms for 200 units and 4.9–5.0ms for
  1,000 units with no statistically significant change.
- No product hot-path patch was made. The official release comparison is
  already faster and the stabilization binary is code-identical to its base.

## Decision

#821 may resume. It now has a verified release-based performance baseline, a
less noisy binary-identity gate, unchanged product behavior, and a tighter
duplication ratchet. No additional cleanup issue is needed.
