# String affix protocol closeout (#548)

Status: #548 is complete through #557 and #558. This page records the aggregate
closeout evidence for the case-sensitive string prefix/suffix predicate protocol
extraction under #547 and #519.

The tranche strengthened the semantic kernel by moving receiver-method string
affix predicates behind pack-owned capability evidence instead of broad
method-call provenance. It did not add selector-name-only admission and did not
widen the runtime exact surface beyond rows that already had exact string
receiver proof.

## Scope

| issue | PR | merge commit | outcome |
|---|---|---|---|
| #557 | [#560](https://github.com/corca-ai/nose/pull/560) | `def272b3` | Added builtin protocol pack `nose.protocols.string_affix_predicates`, producer `protocols.string-affix-predicate-api`, and contract `string_affix.predicate`; routed exact receiver-method prefix/suffix rows through the new pack. |
| #558 | [#561](https://github.com/corca-ai/nose/pull/561) | `b2403ec9` | Expanded conformance refs across Python, Java, Rust, Swift, JavaScript, and TypeScript; added hard negatives for non-string receiver proof, wrong producer provenance, and unsupported offset forms. |

#559 tracks this aggregate closeout artifact and the final #548 closure.

Go `strings.HasPrefix` and `strings.HasSuffix` intentionally remain in
`nose.go.stdlib.namespace_calls` until the namespace-proof slice moves in #549.
Case-insensitive, locale-sensitive, regex/glob/path, multi-affix, and offset
forms remain outside this tranche unless their proof obligations become explicit
capabilities.

## Aggregate Inventory Movement

Baseline ref: `v0.16.0` / `0f40969d356c7d7fedf1eb5158939e9908009bab`.
Final ref: `main` after #561 / `b2403ec91445cffd3525566e0062b0800bb243a0`.

Command:

```sh
nose semantic-pack inventory --format json
```

| metric | before #548 | after #557 | after #558 | total delta |
|---|---:|---:|---:|---:|
| packs | 48 | 49 | 49 | +1 |
| builtin packs | 48 | 49 | 49 | +1 |
| exact-capable packs | 38 | 39 | 39 | +1 |
| packs needing coverage | 0 | 0 | 0 | 0 |
| positive fixtures | 177 | 178 | 188 | +11 |
| hard negatives | 139 | 143 | 146 | +7 |
| conformance refs | 316 | 321 | 334 | +18 |
| unsupported refs | 18 | 19 | 20 | +2 |

String-affix pack-specific movement:

| metric | before #548 | after #557 | after #558 |
|---|---:|---:|---:|
| string-affix protocol pack | absent | present | present |
| string-affix positive refs | 0 | 2 | 12 |
| string-affix hard negatives | 0 | 4 | 7 |

## Product Evidence

The two behavior-adjacent PRs each recorded focused query-regression evidence.
The corpora differ, so the timing rows should not be composed into a single
aggregate runtime claim.

| PR | baseline -> current | product output | metadata drift | median wall time |
|---|---|---|---|---:|
| #560 | `0f40969d` -> `2037a164` | family count `1 -> 1`; families equal excluding `semantic_packs`; investigation triggers `0` | semantic pack count `48 -> 49`; new `nose.protocols.string_affix_predicates` metadata | `19.689 ms -> 17.451 ms` |
| #561 | `def272b3` -> `66120cd5` measured build, docs head `858d65a7` | family count `1 -> 1`; families equal; investigation triggers `0` | string-affix positives `2 -> 12`; hard negatives `4 -> 7`; semantic pack count `49 -> 49` | `9.225 ms -> 8.954 ms` |

Drift classification: intentional provenance, conformance, and inventory
metadata drift only. The focused product families stayed unchanged, and no
runtime degradation was observed in the recorded focused comparisons.

## Done Criteria

| #548 criterion | evidence |
|---|---|
| Pack/protocol descriptor, contract rows, producer ids, and inventory refs updated. | #560 added the new protocol pack, producer id, contract id, descriptor counts, inventory/report exposure, and docs. |
| Focused positives and hard negatives cover receiver proof, direction, arity, and wrong-pack boundaries. | #560 added initial prefix/suffix, direction, missing receiver, wrong pack, and unsupported arity refs plus admission tests; #561 expanded language positives and added non-string receiver, wrong producer, and unsupported offset boundaries. |
| Product query-regression representative recorded. | #560 records baseline/current refs, binary hashes, focused corpus, command shape, trigger count, output drift, and timing; #561 records the same in [string-affix-conformance-closeout-558](string-affix-conformance-closeout-558.md). |
| Two independent review links are attached before merge. | #560 and #561 both link durable Huygens and Carson review artifacts before merge. |

## Review Evidence

| PR | Huygens | Carson |
|---|---|---|
| #560 | [final soundness/provenance review](https://github.com/corca-ai/nose/pull/560#issuecomment-4808338756) | [final process/tests/evidence review](https://github.com/corca-ai/nose/pull/560#issuecomment-4808338749) |
| #561 | [final soundness/provenance review](https://github.com/corca-ai/nose/pull/561#issuecomment-4808642427) | [final process/tests/evidence review](https://github.com/corca-ai/nose/pull/561#issuecomment-4808642446) |

## Closeout Decision

#548 can close after the #559 closeout PR merges. The tranche is complete as a
semantic-kernel capability improvement: common receiver-method prefix/suffix
predicates now have a narrow protocol pack, exact receiver proof remains
mandatory, unsupported forms fail closed, and users can audit the support through
the builtin inventory.

The remaining #547 work should proceed through later leaf issues rather than
expanding this pack by raw API breadth. In particular, #549 is the next natural
step for Go namespace-call migration once namespace proof can be represented as
the same string-affix capability instead of a selector list.
