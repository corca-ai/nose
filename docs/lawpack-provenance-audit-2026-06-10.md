# LawPack provenance audit, 2026-06-10

This audit closes [#186](https://github.com/corca-ai/nose/issues/186): run
targeted real-corpus scans for the first-party LawPack pilot and decide whether
`semantic_laws` provenance is showing up as actionable signal.

The checked-in data artifact is
[bench/labels/lawpack_provenance_audit_2026_06_10.json](../bench/labels/lawpack_provenance_audit_2026_06_10.json).

## What Was Tested

The pack under test was the compiled first-party LawPack,
`nose.value_graph.laws`, which currently exposes two proof-backed value laws:

| law id | proof obligation |
|---|---|
| `value-graph.factor-distribute.numeric-common-factor` | `normalize.value_graph.factor_distribute` |
| `value-graph.clamp.integer-ordered-minmax` | `normalize.value_graph.clamp` |

The scan command was:

```sh
target/release/nose scan bench/repos/<repo> --mode semantic --format json --top 0
```

The release binary was built from `1b83307085171ca6a6f61efaaa2480fdaead463c`.

## Corpus Result

The full `bench/repos` pass had one scanner failure and zero law-bearing
families:

| measure | value |
|---|---:|
| successful repos | 104 |
| failed repos | 1 (`rxjs`, stack overflow) |
| scanned files | 59,865 |
| reported families | 10,967 |
| repos with active `nose.value_graph.laws` | 104 |
| repos with `semantic_laws` families | 0 |
| `semantic_laws` families | 0 |

Language coverage in the successful scans:

| language | files | repos |
|---|---:|---:|
| Java | 18,837 | 21 |
| JavaScript | 8,386 | 59 |
| TypeScript | 7,562 | 21 |
| C | 6,143 | 28 |
| Go | 5,138 | 18 |
| Ruby | 4,864 | 22 |
| Python | 4,475 | 34 |
| Rust | 4,460 | 16 |

The five largest family-producing repos in this pass were `guava` (1,795),
`libgdx` (931), `netty` (912), `sympy` (902), and `sqlalchemy` (736). None had
law-bearing families.

## Targeted Subset

A second pass selected likely LawPack candidates from the frontier numeric-clamp
signal and targeted source search for clamp/min-max and common-factor arithmetic
surfaces.

| repo | why selected | files | families | law families |
|---|---|---:|---:|---:|
| `fzf` | Go custom clamp helper and many call sites | 88 | 4 | 0 |
| `pixijs` | TypeScript nested `Math.min`/`Math.max` clamp idioms | 1,483 | 38 | 0 |
| `alacritty` | Rust min/max viewport clamps | 88 | 1 | 0 |
| `meilisearch` | Rust `.clamp` plus adjacent min/max bound normalization | 690 | 8 | 0 |
| `h2database` | Java `Math.min`/`Math.max`-heavy corpus | 1,292 | 496 | 0 |
| `netty` | Large Java network corpus with min/max candidates | 3,556 | 912 | 0 |
| `marked` | Small JS/TS parser with arithmetic guards | 215 | 1 | 0 |
| `libgdx` | Large Java/C game corpus with arithmetic helpers | 2,100 | 931 | 0 |
| `libsodium` | C arithmetic-heavy corpus | 418 | 21 | 0 |
| `nushell` | Rust numeric min/max and clamp-like guards | 1,712 | 44 | 0 |

## Qualitative Read

The negative result is not evidence that the pack is disconnected. Every
successful scan reported `nose.value_graph.laws` in `semantic_packs` with two
value laws. `semantic_laws` is deliberately narrower: it appears only when a
proven value-graph law actually participates in a reported family.

The targeted search found real law-shaped code, but not law-bearing families:

| example | location | read |
|---|---|---|
| `fzf` generic clamp helper | `bench/repos/fzf/src/util/util.go:63` | `return max(min(val, maximum), minimum)` is a real clamp helper, but generic `cmp.Ordered` operands and parameter names do not prove integer domain or `minimum <= maximum`. |
| `pixijs` color clamp | `bench/repos/pixijs/src/color/Color.ts:1095` | `Math.min(Math.max(value, min), max)` is a useful idiom candidate, but TypeScript `number` is not integer-only and bound order is not proven. |
| `pixijs` render target clamp | `bench/repos/pixijs/src/rendering/renderers/shared/renderTarget/RenderTargetSystem.ts:409` | Bitwise-rounded dimensions are probably integer-shaped, but the current proof boundary does not connect that evidence to the min/max pair. |
| `meilisearch` facet clamp | `bench/repos/meilisearch/crates/milli/src/update/facet/mod.rs:413` | Rust `.clamp(2, 127)` is exact in isolation, but this scan did not report a duplicate family bridging it to an equivalent min/max or ternary member. |
| `alacritty` scroll clamp | `bench/repos/alacritty/alacritty_terminal/src/grid/mod.rs:166` | The expression is clamp-shaped, but mixed casts/newtypes and dynamic bounds keep it outside the small ordered-integer law surface. |

## Conclusion

This is a useful negative field result. The LawPack pilot is present and
serialized in scan metadata, but the current two laws did not surface in the
real-corpus family layer. The main gap is not pack loading; it is that the
current exact law surface is too narrow for common real-code clamp forms, and
clone-family output is the wrong instrument for singleton law-shaped misses.

Next work should add a small law-provenance corpus fixture, introduce a
miss-mining arm for singleton law candidates and proof blockers, and expand
LawPack-facing laws only where proof obligations and hard negatives are already
clear.
