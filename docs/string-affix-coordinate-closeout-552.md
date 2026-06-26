# String affix coordinate closeout (#552)

Status: #552 pins the coordinate boundaries for the existing
`nose.protocols.string_affix_predicates` capability. It does not add new public
semantic-pack APIs; it records which affix value coordinates are safe today and
which multi/offset forms remain deferred.

## Scope

The PR records these boundaries:

- same-role parameter affix coordinates converge across supported languages;
- immutable literal local/module binding affix coordinates converge with direct
  literal affixes;
- wrong parameter coordinates, dynamic affix expressions, and mutated affix
  bindings stay out of the proved coordinate families;
- Python tuple affixes and Ruby multi-affix forms stay closed until explicit
  disjunction semantics exist;
- JS/Java offset forms stay out of whole-string prefix/suffix proof.

## Product Comparison

Baseline ref: `origin/main@995e1ca4`.
Current product build: `issue-552-string-affix-coordinate-boundaries` working
tree before review-evidence updates.

Binary hashes:

- baseline: `10189f62d583c3325f81952bf674a10bc2f1703ecccb0ff3fff59e1885bd2738`
- current: `1c666d0ffc4d5e8100f9168ec5022dbab7aad3b526fe63973a7994f680119e26`

Focused corpus:

- durable fixture: `crates/nose-cli/tests/fixtures/string_affix_552`;
- positives: TypeScript/Swift/Python same-role parameter affixes and
  Python/TypeScript immutable literal binding affixes;
- hard negatives: wrong parameter, dynamic parameter-derived affix, mutated
  binding, Python tuple affix, Ruby multi-affix, and JS/Java offset forms.

Command:

```sh
nose query crates/nose-cli/tests/fixtures/string_affix_552 all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 3 | 3 |
| semantic pack count | 49 | 49 |
| investigation triggers | 0 | 0 |
| parameter coordinate family members | 3 | 3 |
| immutable binding coordinate family members | 3 | 3 |
| offset-form family members | 2 | 2 |
| hard negatives in parameter family | 0 | 0 |
| hard negatives in binding family | 0 | 0 |

The current build changes pack conformance metadata, not the runtime query
families. The focused corpus proves the supported parameter/binding coordinate
families stay open while unsupported multi/offset forms do not collapse into
whole-string prefix proof.

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
| positive fixtures | 190 | 192 |
| hard negatives | 169 | 175 |
| conformance refs | 359 | 367 |
| unsupported refs | 20 | 23 |
| string-affix positives | 16 | 18 |
| string-affix hard negatives | 30 | 36 |
| string-affix unsupported refs | 2 | 5 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
7.106, 7.475, 8.193, 6.835, 6.562, 7.298, 8.289, 8.477, 8.697
```

Current times in milliseconds:

```text
8.537, 6.943, 6.763, 8.075, 6.763, 8.797, 6.400, 7.941, 7.309
```

Median: `7.475 ms -> 7.309 ms` (`-0.166 ms`).

## Review Evidence

- Semantic-boundary review:
  <https://github.com/corca-ai/nose/pull/566#issuecomment-4811704675>
  found no blocking issues in same-role parameter coordinates, immutable literal
  binding coordinates, wrong/dynamic/mutated coordinate hard negatives, or
  deferred tuple/multi/offset forms.
- Evidence/process review:
  <https://github.com/corca-ai/nose/pull/566#issuecomment-4811730827>
  found no blocking issues in conformance refs/counts, unsupported refs,
  focused product output, inventory/runtime evidence, docs/changelog links, or
  the duplication-ratchet tightening.

## Rollback

Revert the #552 PR. That removes the coordinate-boundary fixture, focused
equivalence/query regressions, string-affix coordinate conformance refs, and the
closeout documentation.
