# Ruby string affix closeout (#551)

Status: #551 adds the Ruby `String#start_with?` and `String#end_with?`
proof slice to the existing `nose.protocols.string_affix_predicates`
capability without adding a Ruby-specific API surface.

## Scope

The PR changes receiver proof boundaries:

- Ruby literal string receivers now emit explicit `Domain(String)` evidence,
  which lets the existing exact-string receiver contract admit
  `start_with?`/`end_with?` calls under string-affix protocol provenance;
- untyped Ruby receivers and custom same-name methods remain closed without
  exact string receiver proof;
- Ruby multi-affix calls stay closed until disjunction semantics are
  represented;
- prefix/suffix direction and receiver-vs-affix coordinates remain distinct;
- same-file `class String; def start_with?` / `def end_with?` and
  `String.class_eval` redefinitions, including `define_method` forms, close the
  corresponding Ruby affix admission for that file.

## Product Comparison

Baseline ref: `origin/main@f03019b0`.
Current product build: `issue-551-ruby-string-affix-proof` working tree after
the focused Ruby hard negative fixtures and before review-evidence updates.

Binary hashes:

- baseline: `f1641955710eea8195fb4b114b7f3800b3d293dfd1aff998640dcef9250bf386`
- current: `10189f62d583c3325f81952bf674a10bc2f1703ecccb0ff3fff59e1885bd2738`

Focused corpus:

- durable fixture: `crates/nose-cli/tests/fixtures/string_affix_551`;
- positives: two Ruby literal-string `start_with?` units and two Ruby
  literal-string `end_with?` units;
- hard negatives: untyped receiver, custom same-name receiver,
  multi-affix call, wrong receiver, direction mismatch, same-file
  `String#start_with?` monkey patch, and same-file `String.class_eval`
  / `define_method` monkey patches.

Command:

```sh
nose query crates/nose-cli/tests/fixtures/string_affix_551 all top=0 --mode semantic --format json
```

Result:

| Metric | Baseline | Current |
| --- | ---: | ---: |
| family count | 1 | 3 |
| semantic pack count | 49 | 49 |
| investigation triggers | 0 | 0 |
| proved Ruby prefix family members | 0 | 2 |
| proved Ruby suffix family members | 0 | 2 |
| hard negatives in proved prefix family | 0 | 0 |

The baseline only reported the existing shape-level `custom_same_name.rb` /
`untyped_receiver.rb` pair. The current build adds the two intended Ruby affix
families and keeps every adjacent hard negative out of the proved prefix family.

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
| positive fixtures | 188 | 190 |
| hard negatives | 161 | 169 |
| conformance refs | 349 | 359 |
| unsupported refs | 20 | 20 |
| string-affix positives | 14 | 16 |
| string-affix hard negatives | 22 | 30 |

## Runtime

Method: 2 warmups, then 9 alternating measured repeats over the focused corpus.

Baseline times in milliseconds:

```text
11.586, 10.971, 8.881, 8.971, 9.175, 8.940, 8.982, 10.658, 7.305
```

Current times in milliseconds:

```text
12.135, 9.848, 9.655, 9.766, 9.698, 8.059, 8.189, 8.277, 9.804
```

Median: `8.982 ms -> 9.698 ms` (`+0.716 ms`).

## Review Evidence

- Evidence/process review:
  <https://github.com/corca-ai/nose/pull/565#issuecomment-4811371553>
  found no blocking issues in conformance refs/counts, product-output evidence,
  runtime evidence, docs links, or #551 done-criteria coverage.
- Semantic soundness review:
  <https://github.com/corca-ai/nose/pull/565#issuecomment-4811372589>
  found a blocking `define_method` monkey-patch false-open. The follow-up review
  <https://github.com/corca-ai/nose/pull/565#issuecomment-4811493744>
  verified the blocker is resolved for `class String`, `String.class_eval`,
  direct `String.define_method`, and string-name `define_method` forms.

## Rollback

Revert the #551 PR. That removes the Ruby literal string receiver proof seed,
the same-file Ruby `String#start_with?`/`String#end_with?` redefinition
suppression, the Ruby focused fixture, and the new string-affix conformance
refs.
