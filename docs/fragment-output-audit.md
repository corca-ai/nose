# Exact fragment output audit

This audit records how current exact semantic fragment output behaves from a
user's point of view. It is a read-only product audit: no detector semantics,
ranking implementation, or fragment families changed. Back to
[clone-types](clone-types.md), [scan-json](scan-json.md), and
[review](review.md).

The work was prompted by
[issue #35](https://github.com/corca-ai/nose/issues/35) and feeds three follow-up
contracts:

- [#33](https://github.com/corca-ai/nose/issues/33) should expose stable fragment
  kind and diagnostic proof metadata.
- [#11](https://github.com/corca-ai/nose/issues/11) should define stable
  refactorability and actionability reason codes.
- [#23](https://github.com/corca-ai/nose/issues/23) should consume review-hazard
  metadata without treating every exact fragment as a direct extraction.

## Pre-#33 Observable Audit

Baseline command shape:

```sh
./target/release/nose scan bench/repos/<repo> \
  --mode semantic \
  --format json \
  --top 0 \
  --min-size 1
```

The committed corpus checkouts are not part of the repository, so the scan used
the local `bench/repos` corpus from the main worktree. The branch under audit was
`main` at `0b57dd2`.

The audit intentionally used the current pre-#33 JSON contract. That means the
observable facts are limited to `locations[].kind`, source span, language,
family size, scope, and the source shape visible at the reported span. Current
JSON does not say whether a `Block` is a return fragment, guarded exit, ordered
effect sequence, foreach effect, Java self-field body, or proof-only fragment.
Because true semantic fragment shape is not observable before #33, this audit
uses source shape plus location-kind distribution as the numeric proxy for shape.

## Corpus Sweep

The sample covered 28 repositories and 7,607 supported files.

| language | files |
|---|---:|
| c | 2102 |
| ruby | 2033 |
| java | 1136 |
| python | 584 |
| typescript | 522 |
| javascript | 457 |
| rust | 397 |
| go | 376 |

The scan emitted 1,116 semantic families and 3,692 locations. `Block` dominated
the observable output:

| location kind | locations |
|---|---:|
| Block | 2743 |
| Method | 474 |
| Function | 443 |
| Class | 32 |

815 of 1,116 families were all-`Block` families. 709 families had mean span
length at or below three source lines. This reproduces the product concern: the
exact fragment substrate is real and broad, but most observable fragment output
is too small to be treated as a top-level refactoring candidate by default.

| repo | families | all-Block families | mean span <= 3 |
|---|---:|---:|---:|
| arrow | 184 | 173 | 181 |
| commons-lang | 125 | 86 | 78 |
| gson | 27 | 19 | 13 |
| jsoup | 22 | 13 | 15 |
| chi | 4 | 4 | 4 |
| cobra | 8 | 7 | 6 |
| gin | 4 | 3 | 2 |
| gorm | 31 | 30 | 3 |
| click | 22 | 11 | 20 |
| flask | 47 | 34 | 47 |
| requests | 9 | 5 | 8 |
| black | 51 | 27 | 27 |
| axios | 26 | 16 | 10 |
| ky | 9 | 9 | 1 |
| zod | 19 | 13 | 4 |
| zustand | 4 | 1 | 3 |
| bat | 7 | 3 | 6 |
| ripgrep | 6 | 2 | 5 |
| regex | 33 | 10 | 13 |
| fd | 1 | 1 | 0 |
| faraday | 1 | 1 | 1 |
| jekyll | 6 | 6 | 6 |
| asciidoctor | 6 | 5 | 4 |
| rubocop | 117 | 90 | 116 |
| libuv | 30 | 24 | 19 |
| curl | 93 | 54 | 49 |
| nginx | 95 | 88 | 10 |
| sqlite | 129 | 80 | 58 |

## Taxonomy

### Refactor-worthy

The family is likely useful as a direct extraction, shared helper, codegen
target, or intentionally shared implementation. These should be eligible for
default output when they are not generated, vendored, fixture-only, or fully
contained by a better parent finding.

### Review-hazard

The family is useful because repeated logic in larger units can drift, even when
it is not a clean extraction. These should feed `review output`: `nose review`,
hazard-oriented ranking from #23, PR comments, and grouped context around changed
lines.

### Proof-only/noise

The family is exact, but a human is unlikely to act on it as a top-level finding:
one-line guards, one-line asserts, fixture input/output pairs, tiny boilerplate,
or ordinary test setup. These should be hidden by default, grouped under a larger
finding, or shown only in diagnostic/audit views.

### Unsupported/ambiguous

The family looks interesting, but current JSON cannot explain the proof boundary
well enough for product output. These need #33 metadata before they are trusted
outside debug or audit views, especially when the source includes receiver
effects, pointer/index mutation, Java implicit field writes, string-heavy
locale bodies, temp chains, or effect ordering.

## Representative Examples

The table records representative families. Paths are relative to the scanned
repo. `context/action` states what a maintainer would likely do. Whole
`Function`, `Method`, and `Class` rows use the reported span as the enclosing
unit span. For important `Block` rows, the surrounding source was inspected
manually because current JSON does not serialize parent units:

| fragment evidence | inspected enclosing context |
|---|---|
| `gorm:tests/migrate_test.go:250-269` and `337-356` | `TestSmartMigrateColumn` around `185-270`; `TestSmartMigrateColumnGaussDB` around `272-357` |
| `gorm:callbacks/create.go:18-23` and `callbacks/update.go:36-41` | `BeforeCreate` `15-34`; `BeforeUpdate` `33-53` |
| `black:src/black/nodes.py:634-635` and `645-646` | `is_tuple` `632-640`; `is_tuple_containing_walrus` `643-651` |
| `nginx:src/http/modules/ngx_http_log_module.c:1646-1664` and `src/stream/ngx_stream_log_module.c:1375-1393` | `ngx_http_log_compile_format` `1580-1756`; `ngx_stream_log_compile_format` `1310-1469` |
| `nginx:src/http/modules/ngx_http_upstream_hash_module.c:398-412` and `src/stream/ngx_stream_upstream_hash_module.c:380-394` | `ngx_http_upstream_update_chash` `336-466`; `ngx_stream_upstream_update_chash` `317-448` |
| `sqlite:ext/fts3/fts3_hash.c:173-184` and `src/hash.c:92-103` | hash insertion helpers around `166-187` and `79-104` |
| `curl:tests/server/sws.c:312-323` and `tests/server/tftpd.c:578-589` | `sws_parse_servercmd` `225-329`; `tftpd_parse_servercmd` `535-595` |
| `zod:packages/zod/src/v4/classic/schemas.ts:2677-2685` and `mini/schemas.ts:1829-1837` | `instanceof` schema factory around `2665-2688`; mini factory around `1823-1840` |
| `axios:tests/unit/prototypePollution.test.js:882-887` and `896-901` | adjacent test callbacks inside `describe('Prototype Pollution Protection')` `13-1217` |
| `flask:examples/tutorial/tests/test_auth.py:65` and `test_blog.py:11` | `test_logout` `64-69`; `test_index` `6-17` |
| `zod:packages/zod/src/v4/locales/fr-CA.ts:57-108` and `fr.ts:77-124` | locale `error` closures `5-110` and `5-126` |
| `libuv:src/win/fs.c:894-898` and `1102-1106` | `fs__read` `846-929`; `fs__write` `1056-1131` |
| `sqlite:ext/fts3/fts3_hash.c:254-258` and `src/hash.c:193-197` | hash removal helpers around `249-280` and `188-216` |

One-line proof-only rows that are not listed here intentionally omit enclosing
spans: their actionability decision comes from their one-line guard/assert/setup
shape plus path role, not from a specific parent size. They are exactly the cases
that need future `enclosing_unit` metadata if they ever move out of debug output.

| bucket | evidence | language/kind | context/action | recommended surface |
|---|---|---|---|---|
| refactor-worthy | `libuv:src/unix/core.c:859-868` and `src/unix/os390-syscalls.c:94-103` | C `Function`, 10 lines | Duplicate `next_power_of_two`; extract or share a small utility. | default |
| refactor-worthy | `sqlite:ext/rbu/sqlite3rbu.c:537-567` and `tool/sqldiff.c:857-887` | C `Function`, 31 lines | Same checksum implementation under different names; strong direct helper candidate. | default |
| refactor-worthy | `zod:packages/zod/src/v4/locales/be.ts:5-23` and `ru.ts:5-23` | TypeScript `Function`, 19 lines | Same plural-selection algorithm with locale-specific names; extraction or generator candidate. | default |
| refactor-worthy | `curl:tests/http/testenv/curl.py:855-882` and `947-974` | Python `Method`, 28 lines | `ftp_get` and `ssh_download` build the same command options; extract a testenv helper. | default, test scope |
| refactor-worthy | `axios:tests/browser/basicAuth.browser.test.js:74-77` and `defaults.browser.test.js:77-80` | JavaScript `Function`, 4 lines, 6 members | Repeated browser test flush helper; direct shared test helper. | default, test scope |
| refactor-worthy | `gorm:tests/migrate_test.go:250-269` and `337-356` | Go `Block`, 20 lines | Repeated migration column assertions; extract a test assertion helper. | default, test scope |
| refactor-worthy | `nginx:src/http/modules/ngx_http_log_module.c:1646-1664` and `src/stream/ngx_stream_log_module.c:1375-1393` | C `Block`, 19 lines | Same variable-name scanner in HTTP and stream log modules; helper or macro candidate. | default |
| review-hazard | `commons-lang:src/main/java/org/apache/commons/lang3/ArrayUtils.java:6693-6707` and `6735-6749` | Java `Method`, 15 lines, 9 members | Primitive overload family for `reverse`; extraction is awkward, but edits must stay synchronized. | review |
| review-hazard | `gorm:callbacks/create.go:18-23` and `callbacks/update.go:36-41` | Go `Block`, 6 lines | Shared `BeforeSave` hook logic inside parallel create/update callbacks; flag when either side changes. | review |
| review-hazard | `black:src/black/nodes.py:634-635` and `645-646` | Python `Block`, 2 lines, 4 members | Same early guard in related node predicates; too small for default, useful near a changed predicate. | review |
| review-hazard | `nginx:src/http/modules/ngx_http_upstream_hash_module.c:398-412` and `src/stream/ngx_stream_upstream_hash_module.c:380-394` | C `Block`, 15 lines | Same host/port reverse scan in HTTP and stream upstream hashing; divergent-edit risk. | review |
| review-hazard | `sqlite:ext/fts3/fts3_hash.c:173-184` and `src/hash.c:92-103` | C `Block`, 12 lines | Linked-list insertion duplicated across hash implementations; review if either structure changes. | review |
| review-hazard | `curl:tests/server/sws.c:312-323` and `tests/server/tftpd.c:578-589` | C `Block`, 12 lines | Same command-line continuation parser in two test servers; useful synchronization hazard. | review |
| review-hazard | `libuv:src/unix/fs.c:1637-1649` and `test/test-fs.c:4069-4081` | C `Function`, 13 lines | Production/test copy of buffer-offset logic; review when production behavior changes. | review |
| review-hazard | `zod:packages/zod/src/v4/classic/schemas.ts:2677-2685` and `mini/schemas.ts:1829-1837` | TypeScript `Block`, 9 lines | Same `invalid_type` issue construction across classic/mini variants; keep behavior aligned. | review |
| review-hazard | `axios:tests/unit/prototypePollution.test.js:882-887` and `896-901` | JavaScript `Block`, 6 lines, 6 members | Repeated `try/finally` request and server cleanup pattern in adjacent tests. | review |
| review-hazard | `sqlite:ext/jni/src/org/sqlite/jni/capi/SQLTester.java:757-768` and `ext/wasm/SQLTester/SQLTester.mjs:722-733` | Java/JavaScript `Method`, 12 lines | Cross-language argument validation; not extractable, but a strong sync hazard. | review |
| proof-only/noise | `flask:examples/tutorial/tests/test_auth.py:65` and `test_blog.py:11` | Python `Block`, 1 line, 7 members | Repeated `auth.login()` test setup; exact but not a top-level finding. | hidden/debug |
| proof-only/noise | `flask:examples/javascript/js_example/templates/fetch.html:12` and `jquery.html:13` | JavaScript `Block`, 1 line | Embedded script `ev.preventDefault()` across examples; common local event boilerplate. | hidden/debug |
| proof-only/noise | `faraday:lib/faraday/encoders/flat_params_encoder.rb:25` and `76` | Ruby `Block`, 1 line | Symmetric nil guards in encode/decode; group under enclosing methods if shown. | hidden/debug |
| proof-only/noise | `jekyll:lib/jekyll/filters/url_filters.rb:12` and `33` | Ruby `Block`, 1 line | `return if input.nil?` in URL filters; exact proof-only guard. | hidden/debug |
| proof-only/noise | `rubocop` multiple cops, for example `class_structure.rb:323` and `redundant_line_break.rb:128` | Ruby `Block`, 1 line, 15 members | Common cop guard shape across unrelated cops; high-cardinality noise. | hidden/debug |
| proof-only/noise | `arrow:tests/test_locales.py:191` and `1999` | Python `Block`, 1 line | Locale assertion rows repeat expected strings; useful as test data, not refactoring output. | hidden/debug |
| proof-only/noise | `black:tests/data/cases/keep_newline_after_match.py:2-20` and `23-41` | Python `Function`, 19 lines | Formatter golden input/output intentionally duplicates code; report only in fixture/debug contexts. | hidden/debug |
| unsupported/ambiguous | `zod:packages/zod/src/v4/locales/fr-CA.ts:57-108` and `fr.ts:77-124` | TypeScript `Block`, about 50 lines | Large locale switch with different user-facing strings; current JSON cannot explain why this is exact or actionably refactorable. | hidden until metadata |
| unsupported/ambiguous | `libuv:src/win/fs.c:894-898` and `1102-1106` | C `Block`, 5 lines | Same `overlapped` field write sequence in read/write loops; needs place/effect proof to explain. | review after #33 |
| unsupported/ambiguous | `commons-lang:src/main/java/org/apache/commons/lang3/mutable/MutableByte.java:168-171` and `MutableDouble.java:159-162` | Java `Method`, 4 lines, 6 members | `value--` then return across mutable wrappers; needs Java implicit receiver/self-field proof metadata. | review after #33 |
| unsupported/ambiguous | `sqlite:ext/fts3/fts3_hash.c:254-258` and `src/hash.c:193-197` | C `Block`, 5 lines | Pointer update on `elem->prev` and `pH->first`; needs receiver/place proof before top-level output. | review after #33 |

## Ranking And Grouping Recommendations

1. Separate exactness from actionability.

   The report should explain two different facts: why the fragment is an exact
   semantic match, and why it is worth a human's attention. Exact proof alone
   must not imply refactorability.

2. Prefer enclosing units over child fragments in default output.

   If a method/function/class family contains smaller exact fragments with the
   same member set or overlapping spans, the default list should show the best
   enclosing candidate once. Child fragments can appear as evidence under the
   parent or in a debug view.

3. Default output should favor refactor-worthy families.

   Default `nose scan` human/json/SARIF output should include whole-unit
   families and larger block fragments when they have enough span, spread,
   non-fixture scope, and a stable explanation. Small fragments may still appear
   when they have high family cardinality and clear extraction shape, but the
   burden should be higher than for functions or methods.

4. Review output should favor synchronization hazards.

   `nose review` and #23-style ranking should show smaller guards, effect
   fragments, overload siblings, cross-language ports, production/test copies,
   and parallel module copies when the changed lines overlap or neighbor the
   fragment. The output must show enclosing context because a one- or two-line
   fragment only becomes meaningful inside the surrounding function.

5. Hidden/debug output should keep proof-only evidence available.

   One-line guards/asserts, formatter fixture input/output, generated-looking
   locale data, and extremely common boilerplate should be hidden by default or
   grouped below an enclosing finding. They remain valuable for validating #33
   fragment extraction and for audits.

6. Effect-bearing fragments need stronger explanation before promotion.

   Blocks with field writes, pointer writes, index assignment, receiver method
   calls, temp chains, and ordered effect sequences should not be promoted from
   debug to default output unless #33 can expose a stable `FragmentKind` and a
   clear diagnostic explanation of receiver/place/effect identity.

7. Group by role, not only by fingerprint.

   The product grouping key should consider enclosing unit, path role, scope,
   language, and source shape. Examples: HTTP/stream module siblings, primitive
   overload families, classic/mini package variants, prod/test copies, and
   fixture input/output pairs should be explainable as distinct roles even when
   the exact fingerprint matches.

## Post-#33 Metadata Requirements

These requirements are split between stable public fields and diagnostic facts.
Per the issue decision, `proof_facts` should not become a public contract until
#33 explicitly stabilizes and documents it.

### Stable Public Fields

- `is_fragment`: whether the location is a sub-function fragment rather than a
  whole function/method/class.
- `FragmentKind`: stable coarse kind, for example whole unit, control block,
  guard return, guard throw, ordered effect sequence, foreach effect, index
  assignment effect, Java self-field assignment, or pure expression return.
- `reason_code`: stable actionability vocabulary coordinated with #11. It should
  describe product placement, such as direct extraction candidate, overload
  synchronization hazard, cross-language port hazard, test fixture/golden,
  proof-only guard, or requires diagnostic proof.
- `enclosing_unit`: kind, name when available, file, start line, end line, and a
  stable unit key. This is required for review-hazard output and for grouping
  child fragments under parent findings.
- `span_lines` and `span_tokens`: explicit span size without consumers
  recalculating from locations.
- `family_size`, `scope`, and path role: production, test, fixture/golden,
  generated/vendor when known, mixed prod/test, and module spread.
- `recommended_surface`: a ranking-time result, not detector semantics:
  default, review, hidden, or debug.

### Diagnostic Fields

- `proof_facts`: internal explanation ingredients, such as exact-safe subtree
  containment, receiver identity, place identity, effect sink, effect order,
  callee identity, branch-local temporary chain, static import/projection,
  byte/index contract, and Java `this` receiver proof.
- `proof_fact_version`: only needed if diagnostic fields are serialized for
  tooling experiments.
- `fragment_parent`: whether this fragment is contained by another reported
  semantic family, so default output can suppress duplicate child findings.
- `source_shape`: a best-effort observed shape for audits, not a semantic
  contract.

## Output Surface Recommendation

| bucket | default output | review output | hidden/debug output |
|---|---|---|---|
| refactor-worthy | yes, unless contained by a better parent or fixture-only | yes | optional detail |
| review-hazard | only when span/spread is strong enough for standalone attention | yes | optional detail |
| proof-only/noise | no | only when directly changed and grouped under context | yes |
| unsupported/ambiguous | no until #33 metadata explains the proof | yes after #33 for changed regions | yes |

The ranking implementation can remain unchanged until #33 and #11 provide the
fields above. The important product decision is that exact semantic fragments
are evidence, not automatically refactoring candidates.
