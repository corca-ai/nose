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
  primitive string receiver proof, while nested function parameters named
  `String`/`Object` and standalone block-scoped `String`/`Object` bindings do
  not hide real module-scope global prototype mutations;
- optional offset/position arguments, borrowed prototype calls, custom
  same-name methods, prefix/suffix direction swaps, and receiver/affix
  coordinate swaps remain closed.

## Product Comparison

Baseline ref: `origin/main@7bb480d617f7b9b1317d4bf02e4da2a072dbf69d`.
Current product build ref: `b6045135ce7eea373e39eeee1a32abbf87a964ec`.

Binary hashes:

- baseline: `94b1169ea766bf04d1d43d2696d9cb3d8b11a2850c8f22f139685085cbc87c61`
- current: `107909898de25aa960ced7434f2b4cd6ef6fcf36be225d7833baa5dc9921fc0e`

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
  `Object.defineProperty` prototype patch, nested-parameter `String`/`Object`
  shadows and standalone block-scoped `String`/`Object` shadows adjacent to real
  global prototype patches, wrong affix literal, and wrong receiver.

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
| prefix positive family members | 16 | 6 |
| false-open members in prefix family | 10 | 0 |
| suffix positive family members | 2 | 2 |

The false-open members removed from the prefix family are the TypeScript
`String` object wrapper, optional receiver, direct prototype patch before and
after the function, conditional prototype patch, and
`Object.defineProperty(String.prototype, "startsWith", ...)` patch, including
the two nested-parameter shadow variants and the two block-scoped shadow
variants. Untyped JavaScript and nullable receivers already stayed out of the
proved affix family; #550 records them as explicit hard negatives. The locally
shadowed `String` constructor patch remains in the proved prefix family because
it does not mutate the global string prototype.

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
| hard negatives | 148 | 161 |
| conformance refs | 336 | 349 |
| unsupported refs | 20 | 20 |
| string-affix positives | 14 | 14 |
| string-affix hard negatives | 9 | 22 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
12.622, 12.342, 11.356, 12.811, 13.635, 11.645, 12.002, 12.556, 11.457
```

Current times in milliseconds:

```text
13.827, 10.784, 13.354, 11.405, 11.657, 10.965, 12.308, 11.863, 9.876
```

Median: `12.342 ms -> 11.657 ms` (`-0.685 ms`).

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
- Gibbs re-review, PR #564 at `b89476c9`, found one remaining blocking
  false-open: nested function parameters named `String`/`Object` were counted as
  file-wide shadows and could hide real module-scope global prototype
  mutations. Accepted change in `d52b2faa`: prototype mutation suppression now
  uses a module-scope-only shadow check, and the durable fixture includes both
  nested-parameter regressions. Rejected feedback: none.
- Gibbs re-review, PR #564 at `62bec114`, found one remaining blocking
  false-open: standalone block-scoped `String`/`Object` bindings were counted as
  module-scope shadows and could hide later real global prototype mutations.
  Accepted change in `b6045135`: module-scope shadow detection no longer treats
  top-level `Block` contents as module bindings, and the durable fixture includes
  both block-scoped-shadow regressions. Rejected feedback: none.
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
