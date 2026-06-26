# JS/TS string affix hardening closeout (#550)

Status: #550 hardens JavaScript/TypeScript `startsWith` and `endsWith`
admission so the string-affix protocol admits proven primitive string receivers,
not arbitrary same-named methods or patched `String.prototype` methods.

## Scope

The PR changes receiver proof boundaries, not the supported affix operation:

- TypeScript primitive `string` annotations still prove exact string receivers;
- TypeScript `String` object-wrapper annotations no longer prove primitive
  string receivers;
- TypeScript nullable unions such as `string | null` remain closed;
- JavaScript without a dependency-backed string receiver proof remains closed;
- direct top-level `String.prototype.startsWith` and
  `String.prototype.endsWith` writes close JS/TS string-affix admission for that
  file;
- optional offset/position arguments, borrowed prototype calls, custom
  same-name methods, prefix/suffix direction swaps, and receiver/affix
  coordinate swaps remain closed.

## Product Comparison

Baseline ref: `origin/main@7bb480d617f7b9b1317d4bf02e4da2a072dbf69d`.
Current product build ref: `02021e1bc2ed56fb6071584ad633482372067c94`.

Binary hashes:

- baseline: `94b1169ea766bf04d1d43d2696d9cb3d8b11a2850c8f22f139685085cbc87c61`
- current: `b7b18246b8b2cb2cd294df070036aea6d360df65e251f491a74f894c6b248bbf`

Focused corpus:

- positives: Python, TypeScript, Go, Rust, and Java `startsWith`/`HasPrefix`
  equivalents; Python and TypeScript `endsWith` equivalents;
- hard negatives: untyped JavaScript receiver, borrowed
  `String.prototype.startsWith.call`, custom same-name method, TypeScript offset
  argument, `String` object wrapper, nullable receiver, top-level prototype
  patch before and after the function, wrong affix literal, and wrong receiver.

Command:

```sh
nose query /tmp/nose-550-corpus all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 3 | 3 |
| semantic pack count | 49 | 49 |
| investigation triggers | 0 | 0 |
| prefix positive family members | 8 | 5 |
| false-open members in prefix family | 3 | 0 |
| suffix positive family members | 2 | 2 |

The false-open members removed from the prefix family are the TypeScript
`String` object wrapper and the two top-level `String.prototype.startsWith`
patch cases. Untyped JavaScript and nullable receivers already stayed out of the
proved affix family; #550 records them as explicit hard negatives.

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
| hard negatives | 148 | 154 |
| conformance refs | 336 | 342 |
| unsupported refs | 20 | 20 |
| string-affix positives | 14 | 14 |
| string-affix hard negatives | 9 | 15 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
9.227, 9.393, 9.161, 8.300, 8.247, 8.996, 8.913, 9.254, 9.390
```

Current times in milliseconds:

```text
7.510, 8.680, 8.149, 10.715, 7.505, 7.622, 7.290, 8.867, 8.750
```

Median: `9.161 ms -> 8.149 ms` (`-1.012 ms`).

## Rollback

Revert the #550 PR. That restores the previous TypeScript annotation parser and
JS/TS string-affix prototype behavior, including the known false-open cases for
`String` object wrappers and top-level `String.prototype` patches.
