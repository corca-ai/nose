# etcd Go frontend attribution for #907

Generated on 2026-07-19. The [durable attribution
artifact](../bench/recall_loss/issue-907-etcd-frontend-attribution-2026-07-19.v1.json)
binds the frozen measurements, executable identities, disassembly hashes, and sampling
summary below.

## Outcome

The inherited etcd signal is not a proven structural Go frontend regression, so #907
does not change product code. The frozen #892 r40 run measured raw increases of only
`2.50 ms` in `lower` and `2.55 ms` in `parse+lower`, both below the unchanged `5 ms`
absolute gate. Negative same-binary controls of `-3.05 ms` and `-3.35 ms` inflated the
control-adjusted rows to `5.55 ms` and `5.90 ms`.

Source and executable inspection found no new Go lowering work. The official v0.19.0
binary and the #892 candidate were built with different macOS SDKs, however, and their
source-identical native parser code and whole-binary layout differ. The residual is
therefore attributed to build provenance plus signed control subtraction, not to a
product path that warrants speculative optimization.

## Frozen reproduction

No timing run was repeated to seek a different verdict. The source of truth remains the
#892 r40 checker status (`SHA-256 021b949b…8b8f2`) and its durable performance artifact
(`SHA-256 e603d720…f6b04`).

| Stage | v0.19.0 | Candidate | Raw delta | Control | Adjusted delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| `lower` | 85.15 ms | 87.65 ms | +2.50 ms | -3.05 ms | +5.55 ms / +6.52% |
| `parse+lower` | 55.90 ms | 58.45 ms | +2.55 ms | -3.35 ms | +5.90 ms / +10.55% |

The unchanged checker contract is `5%` and `5 ms`. The raw rows cross only the relative
gate; applying the signed negative control makes them cross the absolute gate too.

## Source and build identity

The seven Go frontend files have the same tree digest at v0.19.0, the #892 candidate,
and current main. `tree-sitter-go 0.25.0`, `tree-sitter 0.25.10`,
`tree-sitter-language 0.1.7`, and the release profile are also unchanged.

The two exact binaries do not share a build environment:

| Identity | Official v0.19.0 | #892 candidate |
| --- | ---: | ---: |
| Source | `0985e696…ef9` | `8c37b8f9…59` |
| macOS minimum | 11.0 | 11.0 |
| macOS SDK | 14.5 | 26.2 |
| `__text` bytes | 7,045,316 | 7,289,744 |
| `__TEXT` bytes | 19,693,568 | 19,972,096 |
| Go symbols | 41 | 41 |
| Go module span | 55,344 bytes | 55,344 bytes |

Normalized LLVM disassembly is identical for the hot Rust Go functions
`lower_stmt`, `lower_expr_with_iota`, and `lower_block`. The
`ts_parser_parse_with_options` wrapper is also identical. `ts_parser_parse` differs even
though its dependency source version is the same, consistent with the SDK and
whole-program code-generation difference rather than a Go frontend source delta.

## Diagnostic profile

A single sequential `/usr/bin/sample` profile per exact binary used the frozen etcd
command with `RAYON_NUM_THREADS=1`. This was call-path attribution, not a new timing
verdict.

| Inclusive samples | Official v0.19.0 | #892 candidate |
| --- | ---: | ---: |
| Go `lower_source` | 410 | 395 |
| parser | 251 | 231 |
| derived post-parser lower | 159 | 164 |
| `ts_parser_parse` on top | 58 | 52 |

The candidate exposes no added call path: total frontend and parser samples are lower,
while post-parser lowering differs by five samples in a one-second diagnostic profile.

## Decision and follow-up

Changing Go lowering in response to this evidence would be speculative and would risk
semantic output for a signal the source does not own. #907 therefore records a no-go
optimization decision and preserves family IDs, ordering, surfaces, metadata, output
bytes, and determinism by making no product change.

[#927](https://github.com/corca-ai/nose/issues/927) separately preregisters an
order-aware same-binary control estimator. It keeps the `5%` / `5 ms` product gate,
must replay the frozen #892 evidence without rerunning it, and may not special-case etcd
or any stage.
