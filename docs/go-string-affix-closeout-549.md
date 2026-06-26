# Go string affix closeout (#549)

Status: #549 moves Go `strings.HasPrefix` and `strings.HasSuffix` from the Go
namespace-call pack into the string-affix protocol pack.

## Scope

The PR changes semantic ownership, not the accepted source shape:

- Go `strings.HasPrefix` and `strings.HasSuffix` now use
  `nose.protocols.string_affix_predicates` provenance;
- exact admission still requires imported `strings` namespace proof;
- `strings.Contains` stays in `nose.go.stdlib.namespace_calls` as substring
  containment and does not become collection membership or affix proof;
- wrong namespace proof, missing import proof, wrong old pack provenance,
  unsupported arity, and direction mismatch stay closed.

## Product Comparison

Baseline ref: `origin/main@e249027dc9cb5f0030b59a35f7a2fe5aa037795a`.
Current build ref: `17935f4f271a7ad67f05ab7d68e240f2bb653cd3`.

Binary hashes:

- baseline: `b7f7bd1838df7a32ba700584e95516e561b7d1df38f66b97b7e82bbf868d14d4`
- current: `94b1169ea766bf04d1d43d2696d9cb3d8b11a2850c8f22f139685085cbc87c61`

Focused corpus:

- `prefix.go`: `strings.HasPrefix(value, "pre")`
- `prefix.py`: `value.startswith("pre")`
- `prefix.ts`: `value.startsWith("pre")`
- `suffix.go`: `strings.HasSuffix(value, "pre")`
- `contains.go`: `strings.Contains(value, "pre")`

Command:

```sh
nose query /tmp/nose-549-corpus all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 1 | 1 |
| family array equal | true | true |
| semantic pack count | 49 | 49 |
| investigation triggers | 0 | 0 |

Drift classification: intentional semantic-pack metadata and ownership drift
only. Focused query families are unchanged.

## Inventory Comparison

Command:

```sh
nose semantic-pack inventory --format json
```

| Metric | Baseline | Current |
| --- | ---: | ---: |
| packs | 49 | 49 |
| builtin packs | 49 | 49 |
| exact-capable packs | 39 | 39 |
| packs needing coverage | 0 | 0 |
| positive fixtures | 188 | 188 |
| hard negatives | 146 | 148 |
| conformance refs | 334 | 336 |
| unsupported refs | 20 | 20 |
| string-affix positives | 12 | 14 |
| string-affix hard negatives | 7 | 9 |
| Go namespace positives | 5 | 3 |
| Go namespace hard negatives | 2 | 2 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
7.429, 7.297, 5.575, 7.282, 7.187, 5.560, 7.349, 7.438, 7.349
```

Current times in milliseconds:

```text
7.395, 5.717, 7.575, 7.302, 5.660, 7.278, 7.257, 6.024, 5.479
```

Median: `7.297 ms -> 7.257 ms` (`-0.040 ms`).

## Rollback

Revert the #549 PR. That restores Go `strings.HasPrefix` and
`strings.HasSuffix` provenance to `nose.go.stdlib.namespace_calls` without
changing the source-level imported namespace proof requirement.
