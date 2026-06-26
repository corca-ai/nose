# JS/TS string affix hardening closeout (#550)

Status: #550 hardens JavaScript/TypeScript `startsWith` and `endsWith`
admission so the string-affix protocol admits proven primitive string receivers,
not arbitrary same-named methods or patched `String.prototype` methods.

## Scope

The PR changes receiver proof boundaries, not the supported affix operation:

- TypeScript primitive `string` annotations still prove exact string receivers;
- TypeScript `String` object-wrapper annotations no longer prove primitive
  string receivers;
- TypeScript nullable unions such as `string | null` and optional parameters
  such as `value?: string` remain closed;
- JavaScript without a dependency-backed string receiver proof remains closed;
- module-scope `String.prototype.startsWith` and
  `String.prototype.endsWith` writes close JS/TS string-affix admission for that
  file, including writes inside top-level control flow and
  `Object.defineProperty(String.prototype, "...", ...)`;
- syntactic local shadows of `String`/`Object` do not suppress unrelated
  primitive string receiver proof;
- optional offset/position arguments, borrowed prototype calls, custom
  same-name methods, prefix/suffix direction swaps, and receiver/affix
  coordinate swaps remain closed.

## Product Comparison

Baseline ref: `origin/main@7bb480d617f7b9b1317d4bf02e4da2a072dbf69d`.
Current product build ref: `9e3047f2057e9c9dd98e3c2a289d1cfc6023eb05`.

Binary hashes:

- baseline: `94b1169ea766bf04d1d43d2696d9cb3d8b11a2850c8f22f139685085cbc87c61`
- current: `3184f8020ad8354d8adde15ca18f734033259f89febfd0beb3ca1a8bd5518b7b`

Focused corpus:

- durable fixture: `crates/nose-cli/tests/fixtures/string_affix_550`;
- positives: Python, TypeScript, Go, Rust, and Java `startsWith`/`HasPrefix`
  equivalents; a TypeScript file with a locally shadowed `String` constructor
  patch that does not affect primitive strings; Python and TypeScript
  `endsWith` equivalents;
- hard negatives: untyped JavaScript receiver, borrowed
  `String.prototype.startsWith.call`, custom same-name method, TypeScript offset
  argument, `String` object wrapper, nullable receiver, optional receiver,
  prototype patch before and after the function, conditional prototype patch,
  `Object.defineProperty` prototype patch, wrong affix literal, and wrong
  receiver.

Command:

```sh
nose query crates/nose-cli/tests/fixtures/string_affix_550 all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 3 | 3 |
| semantic pack count | 49 | 49 |
| investigation triggers | 0 | 0 |
| prefix positive family members | 12 | 6 |
| false-open members in prefix family | 6 | 0 |
| suffix positive family members | 2 | 2 |

The false-open members removed from the prefix family are the TypeScript
`String` object wrapper, optional receiver, direct prototype patch before and
after the function, conditional prototype patch, and
`Object.defineProperty(String.prototype, "startsWith", ...)` patch. Untyped
JavaScript and nullable receivers already stayed out of the proved affix family;
#550 records them as explicit hard negatives. The locally shadowed `String`
constructor patch remains in the proved prefix family because it does not mutate
the global string prototype.

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
| hard negatives | 148 | 157 |
| conformance refs | 336 | 345 |
| unsupported refs | 20 | 20 |
| string-affix positives | 14 | 14 |
| string-affix hard negatives | 9 | 18 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
10.371, 9.924, 11.118, 11.349, 9.752, 10.065, 11.407, 9.290, 11.753
```

Current times in milliseconds:

```text
10.675, 8.974, 9.439, 10.492, 9.059, 9.024, 11.143, 10.419, 10.492
```

Median: `10.371 ms -> 10.419 ms` (`+0.048 ms`).

## Review Evidence

- Gibbs semantic soundness review, PR #564 at `c4cb3339`, read-only prompt
  bounded to JS/TS string-affix receiver proof. Blocking findings: module-scope
  control-flow and `Object.defineProperty` prototype patches were missed;
  `value?: string` was admitted as primitive `string`. Non-blocking finding:
  local `String` shadows over-closed unrelated primitive string calls. Accepted
  changes in `9e3047f2`: recursive module-scope mutation scan that stops at
  function/lambda boundaries, unshadowed `String`/`Object` checks, exact
  `Object.defineProperty(String.prototype, "startsWith"|"endsWith", ...)`
  suppression, optional TypeScript annotation fail-closed behavior, and durable
  fixture coverage. Rejected feedback: none.
- Kepler evidence/process review, PR #564 at `c4cb3339`, read-only prompt
  bounded to done criteria, conformance counts, docs, and measurement evidence.
  Blocking finding: review artifacts were not durable yet. Non-blocking
  finding: product regression evidence pointed only at `/tmp` scratch state.
  Accepted changes: this committed review-evidence section and the durable
  `crates/nose-cli/tests/fixtures/string_affix_550` product fixture used by both
  the CLI regression and closeout command. Rejected feedback: none.

## Rollback

Revert the #550 PR. That restores the previous TypeScript annotation parser and
JS/TS string-affix prototype behavior, including the known false-open cases for
`String` object wrappers, optional receivers, and module-scope
`String.prototype` patches.
