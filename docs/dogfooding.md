# Dogfooding nose on nose — a critical review

Goal: honestly assess whether `nose query crates all top=0 --mode near --min-value 40` produces *real* design-level
refactoring opportunities on its own codebase, act on the genuine ones, and record
where the tool is weak. The third-party counterpart is [field evaluation](field-evaluation.md);
the duplication gate that grew out of this lives in [CONTRIBUTING](../CONTRIBUTING.md).

Original review scope: the then-production crates only (6 Rust crates, 8-language
frontends). Result at the start of this review: 34 candidate families, ~662 duplicated
lines (the arc below took it to 23 / ~411). The numbers are a dated snapshot of *this*
review pass; re-running on today's larger codebase reports more, since the crates have
since grown.

The current CI gate is [scripts/check-duplication.sh](../scripts/check-duplication.sh):
it runs `nose query crates all top=0 --mode near --min-value 40 --format json` and
compares the default-surface family IDs with
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json). Tests are
included in the current ratchet so fixture/scaffolding copy-paste stays visible instead
of being policed only by the file-length gate. A family disappearing also requires a
baseline/docs update, so an unrelated removal cannot mask a newly introduced duplicate.

## 2026-06-19 tests-included ratchet baseline snapshot

Reviewed on 2026-06-19 with the current binary and current tree. The production-only
default surface reports 24 substantial families; the tests-included default surface
reports 36. The reviewed default-surface families below are accepted as pre-existing
debt, not as permission to add more. Update the baseline only when the corresponding
family delta is reviewed here.

This is a historical snapshot of the first tests-included ratchet baseline. The exact
current machine baseline is [`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json);
later sections record reviewed deltas from this 36-family state.

`0a5ac734c56c9f54`, `0a5cdb261739af70`, `1267c115f7832175`, `1639812e75927a23`,
`18b10c46c5eef924`, `1dfaba2582163d7c`, `1fc08105c8b5d5c0`, `209fdc39157ececd`,
`28594d5cfe2a2c75`, `3e76e062e630928d`, `4890b7227d416249`, `49cf43940d7ba72c`,
`4ac4a88371e43e72`, `4fcb322e2465279d`, `60806a4da1fcff4f`, `6a34db62d843f27d`,
`6e37683225332c86`, `77d8e8012b2ac08a`, `7f4ff361137cc14a`, `84df147de864f719`,
`8d3e36bdd11cf2c0`, `8f9c8cadbe769f47`, `90809d0e27461ac4`, `936f238ab2e0d6b2`,
`98f5617cbcf09658`, `ab38dd94000926e1`, `b527e97155167c1b`, `bf4255f2994b1d65`,
`c5f1969d0a866135`, `c9fe4dc9d9cd14f5`, `d7dea9009200ed08`, `e479623ccf355d32`,
`e633f3912604730d`, `f010e9908081b902`, `f380654d807c1e90`, `f57a5ee0ebbdf114`.

| family | scope | judgment | action |
|---|---|---|---|
| `49cf43940d7ba72c` | mixed | `evidence_with_dependencies` / `evidence` test-support builders repeat across semantics, detect, and normalize; real shared fixture shape, but crossing crate support boundaries. | Track as visible fixture debt; extract only with a deliberate shared evidence-fixture boundary. |
| `7afae0406480a99e` | mixed | `evidence_anchor_span` appears in JS/TS test support and production evidence helpers; tiny same-purpose accessor. | Candidate for a small helper when the evidence APIs are next touched. |
| `9dfc900a8a39f8c9` | production | `source_*_at_node` evidence accessors repeat the same `evidence_at_span` wrapper; this whole-accessor representative leaves the old `c5f1969d0a` inner slice below the default surface. | Accepted as existing local helper debt; not introduced by the query multi-root PR. |
| `1267c115f7832175` | test | method-call IL fixture builders differ by receiver/argument shape but share a large construction skeleton. | Candidate for a fixture builder; keep until it improves readability. |
| `d7dea9009200ed08` | test | receiver-domain fail-closed tests share setup for three distinct evidence-break cases. | Accepted test scaffold; consolidate only around named receiver-domain scenarios. |
| `248e283bde49aaf6` | test | strict-exact receiver/binding-domain tests share evidence setup across detect unit surfaces. | Visible cross-test debt; extract if a common strict-exact receiver fixture emerges. |
| `90809d0e27461ac4` | test | interpreter field-state tests repeat state construction across read/write scenarios. | Candidate for a state-fixture builder when interpreter tests are next reorganized. |
| `8f9c8cadbe769f47` | test | HOF demand and strict-exact lazy receiver tests share library-HOF fixture setup. | Accepted cross-boundary test scaffold; extract only if it names the HOF demand scenario. |
| `f380654d807c1e90` | test | typed/free call IL fixture builders share a construction skeleton. | Candidate for a small fixture builder. |
| `0a5cdb261739af70` | test | library API admission resolver tests share resolver/evidence setup for node and call paths. | Accepted as paired behavior tests; extract if resolver fixture setup grows again. |
| `e633f3912604730d` | production | `UnionFind` exists independently in detect clustering and markdown detection. | Real shared utility candidate, but cross-crate extraction is out of scope for the CLI prelude isolation pass; keep visible under the ratchet. |
| `4890b7227d416249` | test | ordered loop-conditional exact-fragment tests share the same branch fixture skeleton with the ordered conditional tests. | Accepted as visible test-fixture debt; extract only if a named ordered-branch fixture reduces the per-case signal. |
| `77d8e8012b2ac08a` | production | query origin-hint helpers share scoring/reason collection shape inside `query_opportunities`. | Small real local helper candidate; keep visible until that module is next simplified. |
| `84df147de864f719` | test | guard/static-import/library-API semantic tests share the same query fixture harness. | Accepted named behavior tests; table only if it keeps failure messages specific. |
| `8d3e36bdd11cf2c0` | test | list/map/module literal convergence tests repeat cross-language fixture structure. | Candidate for a literal-fixture table; keep visible until it improves readability. |
| `98f5617cbcf09658` | test | JS/Python/Ruby literal-preservation and `typeof` guard tests share the same semantic query scaffold. | Accepted test scaffold across boundary cases. |
| `b527e97155167c1b` | test | ordered conditional/effect exact-fragment tests still share a large branch fixture shape. | Real refactor candidate; keep visible under the ratchet. |

The 2026-06-19 query-only/prelude refresh removed nine stale IDs from the old
baseline and added the six reviewed rows above. Net current count: 39 → 36. Later
query-surface and multi-root PRs refreshed the evidence-accessor representative without
changing the count: `9dfc900a8a39f8c9` is the current default-surface whole-accessor
family, while `c5f1969d0a866135` remains the shallow inner slice.

The 2026-06-20 CI repair split overlong tests and production diagnostic/lowering
dispatch functions to satisfy the file-length and clippy ratchets. The substantial
default-surface count stayed at 36, but 11 family IDs changed because helper extraction
and line-span movement shifted the representative spans: strict-exact fixture setup,
async/protocol-boundary tests, evidence accessors, evidence-anchor helpers, switch-label
folding, frontend call/dispatch parallelism, raw-name test helpers, and the coverage /
gap-impact census loop. These are the same reviewed debt classes above or
parallel-by-design helper boundaries; no budget increase was accepted.

## Verdict by candidate (critically)

| family | what it is | judgment | action |
|---|---|---|---|
| `lang_str` (detect + coverage) | an **exact** `Lang→&str` duplicate | real, clear | ✅ unified into `Lang::name()` |
| `*_bin_op` (js/go/rust) | near-identical operator tables | real, low-risk | ✅ extracted `lower::common_bin_op` |
| `lower` entry points (all 8 frontends, sim 1.00) | parse → lower-root → build FileMeta → finish | reconsidered → **real** | ✅ extracted `lower::lower_file(…, key, lang_fn, lang, lower_root)`; each frontend passes only its grammar, `Lang` tag, and root-lowering closure (~80 lines removed). Earlier judged "leave it" at 3 sites; at 8 the boilerplate clearly dominated the 3 specific lines. |
| `lower_while` (c/java/python/ruby/rust, 5 sites) | "extract cond + body, build `While` Loop" | real, clean | ✅ extracted `lower::while_loop(node, cond_fn, body_fn)` — closures supply the per-language cond/body lowering |
| `lower_block`/`lower_source`/`lower_items` (11 sites, value 227 — top family) | per-frontend "iterate children → Block/Module" | reconsidered → **real** | ✅ extracted `lower::collect_into(node, kind, lower_one)`; the fn-arg is a closure, not a pointer, and it removed ~110 lines across 7 builders with byte-identical IL. (Earlier judged "leave it"; at 11 sites the duplication clearly outweighed the one line of indirection.) |
| `lower_binary`/`lower_binop` (6 frontends) | extract left/op/right → `BinOp` | reconsidered → **real** | ✅ extracted `lower::binary(node, op_of, lower_operand)`; standardized the name (Python's was `lower_binop`) and the fallback (go/py/js wrongly defaulted unknown ops to `Add`; now all use the correct `Raw` fallback). The fields (`left`/`operator`/`right`) are *shared* across grammars, so no quirk leaks. |
| `lower_func` (python/go/js/rust, sim 0.86) | name + params + body → `Func` unit | reconsidered → **real** | ✅ extracted `lower::function_unit(node, method, lower_params, lower_body)`; the per-grammar param/body lowering are closures. c/java/ruby keep bespoke versions (genuinely divergent param handling). |
| `lower_switch` (c/java, sim 0.89) | switch → if/else-if chain | real, clean | ✅ extracted `lower::switch_to_if_chain(node, is_case, …)`; the case-node predicate is a clean parameter, not a leaked quirk. Centralizing it also documents the "case values not matched yet" limitation in one place. |
| `lower_if` (c/java/js_ts, 3 sites) | cond + then + optional else → `If` | real, clean | ✅ extracted `lower::if_stmt(node, cond_fn, then_fn, else_fn)`. The `condition`/`consequence`/`alternative` field names are *shared* across the three grammars (no wart leaks, like `binary`); only the else-branch resolution (bare block vs else-if recursion) varies, supplied as a closure. go/rust/python/ruby keep bespoke `if` lowering — init-prefix, if-let, elif chains: genuinely divergent shape. |
| `stmt_as_block` (c/java/js_ts, 3 sites) | "already a block? lower it : wrap the single statement" | real, clean | ✅ extracted `lower::stmt_as_block(node, block_kind, lower_block, lower_stmt)`; only the grammar's block-node name (`compound_statement`/`block`/`statement_block`) and the two lowerings vary. The c/java `lower_for`+`stmt_as_block` copy-paste was the single cleanest family (32 lines) on the re-run that drove this pass. |
| `lower_for` C-style (c/java/js_ts, 3 sites) | init/cond/update/body → `CStyle` Loop | real, clean | ✅ extracted `lower::c_style_for(node, init_field, update_field, …closures)`; the two clause field names that differ (`initializer`/`init`, `update`/`increment`) are params, the four sub-lowerings are closures. c↔java differed by a *single* field name over a ~30-line body — boilerplate dominates (cf. `lower_file`). go's `range_clause`/init-prefix `for` stays bespoke. |
| `lower_while`/`lower_do` (js_ts, 2 sites) | identical `condition`/`body` While Loop | real, clean | ✅ routed both through the existing `lower::while_loop` and collapsed the dispatch to `while_statement \| do_statement`, deleting the byte-identical `lower_do` (do-while's run-body-first semantics stay unmodelled, as before). The other frontends already used `while_loop`; js_ts was the last inlined copy. |
| `mark`/`mark_defs` (dce/dataflow, sim 1.00) | collect scope params/defs/nested-fns | real, clean | ✅ extracted `normalize::collect_scope` (free fns, no borrow obstacle). |
| `generic` node-copy (cfg_norm/dataflow/dce/desugar, 4 sites, sim 1.00) | recurse over children via `self.go`, then `rebuild_like` | real dup, extractable via **macro** | ✅ extracted into a `rebuild_generic!()` macro — re-opened when a later re-run pushed the duplication gate to 5 > 4. The earlier verdict (below) correctly ruled out a *trait* (a default method routes the disjoint `&mut self.b` + `&self.old` field borrows through `&self`/`&mut self` accessors, which the borrow checker can't see as disjoint), but didn't consider a **`macro_rules!`** — it expands in-place, preserving the disjoint field access. The right tool for "identical method body, sibling structs." |
| `lower_unary` (go/js, sim 0.82) | unary op → `UnOp` or strip | real but **left** | ⚠️ the operand field name differs per grammar (`operand` vs `argument`); a shared helper would take that name as a parameter, leaking a grammar wart into the abstraction for only two callers — the duplication is the lesser evil. |
| `lower_call` / `lower_new` (go/js/python) | per-grammar shapes | parallel-by-design | ⚠️ left: callee/arguments node shapes differ per grammar; coupling would leak quirks across frontends |
| `NodeKind`/`UnitFeat`/`DetectOptions`/`ValOp`/`Payload` | enum/struct **type definitions** with similar field-count shape | **false positive for refactoring** | ❌ distinct domain types; no shared logic to extract |

## What this says about the tool (honest)

**Genuinely useful.** Acting on its own findings drove a real consistency pass over the
frontends — every cross-frontend "parallel" shape now routes through one shared helper in
`lower.rs`: `lang_str`→`Lang::name()`, operator tables→`common_bin_op`, `lower_while`→
`while_loop`, the module/block builders→`collect_into`, the `lower` entry points→`lower_file`,
binary expressions→`binary`, `Func` units→`function_unit`, `switch`→`switch_to_if_chain`,
`if`→`if_stmt`, the block-or-wrap helper→`stmt_as_block`, the C-style `for`→`c_style_for`, and
in `normalize` the dce/dataflow scope walk→`collect_scope`. The control-flow set
(`if_stmt`/`stmt_as_block`/`c_style_for`) and the js_ts `while`/`do` merge landed in a later pass
once the crates had grown enough to re-surface them; each left the IL byte-identical (verified by
re-running `nose query` on the C/Java/JS/Rust/Python corpus and diffing the family output). The dogfood report shrank from 34
families / ~662 duplicated lines to 23 / ~411, and two latent inconsistencies were fixed in the
process (Python's `lower_binop` name; go/py/js's wrong `Add` fallback for unknown operators).
Each was reviewer-confirmed and left the IL byte-identical. Alongside them are families where
the right answer is "leave it" — for example, `lower_unary`, where a per-grammar field-name
would leak into the abstraction, and the remaining per-grammar frontend parallelism that is
clearer than a forced shared helper. That human-in-the-loop judgment is the point: surfacing
candidates is the tool's job, deciding is the reviewer's.

**Known weakness — type-definition false positives.** The top family by value was a
cluster of unrelated `enum`/`struct` definitions that merely share a "block of field
declarations" shape. These are *not* refactoring candidates (they're distinct types
with no shared behavior). A ranking-time discount for computation-poor type-definition
families has landed; a future `--kind fn|type|all` filter would make this easier to
control explicitly when a user is hunting only behavioral duplication.

## Conclusion

On its own codebase nose behaves as intended: high-recall surfacing of similar code,
ranked so the genuine wins (exact dups, operator tables) rise, with the reviewer
dismissing parallel-by-design families. The one clear gap — type-definition shape
false positives — is logged as a future candidate-mode improvement.

## Re-run (a later pass, after the IL-convergence work)

Re-running the duplication gate found it over budget (5 > 4). Triage held up the original
verdicts: the top families were the per-language frontend lowering arms — each mapping a
*grammar-specific* node-kind string to an *already-shared* `lower.rs` helper, so the residual
similarity is the parallel match structure, not extractable logic (parallel-by-design, the
[experiments](experiments.md) §AV "judgment-deep precision" thesis confirmed on our own code). The one
genuinely actionable family was the `generic` wrapper — promoted from "kept" to ✅ via the
`rebuild_generic!` macro (see the table), which restored the gate to 4/4. Net lesson: on a
well-factored codebase the gate's job is mostly to catch *new* avoidable duplication (here, a
pre-existing 4-copy wrapper a `macro_rules!` cleanly removes), while the standing top families
are correctly-dismissed intentional parallelism.

## Re-run (2026-06-05, while versioning scan JSON)

The duplication gate reported 6 substantial near-duplicate families against a budget of 4.
The two additional families were not introduced by the scan JSON schema work: the PR changed
CLI JSON wrapping, tests, and docs, while the findings are all in existing frontend lowering
code (`lower_call`/`lower_new`, map/object/hash lowering, per-language parse roots, module
lowering arms, and small C/Java/Rust root wrappers). They are the same class of residual
per-grammar parallelism described above: reviewed design debt, not accidental new duplication
from the JSON contract change.

The gate budget is therefore refreshed to 6 for the current accepted state. Future PRs should
still treat any count above 6 as a ratchet failure: either remove the new duplication or record
why it is intentionally accepted.

## PR #82 — budget re-baselined 6 → 20 (stronger `near` detection)

PR #82 added value-fingerprint candidate generation + high-`vj` acceptance for impure code
(async/IO/opaque-call) and sub-DAG anchor pairing to the `near` channel. The detector therefore
now surfaces 14 additional **pre-existing** substantial near-duplicate families in nose's own
source — chiefly the per-grammar frontend helpers and the `proven_*` value-graph collection/map
factories (genuinely parallel functions, like the frontend parallelism already accepted here).
These are dedup candidates, not duplication introduced by the PR. The gate budget is re-baselined
to 20 so it keeps ratcheting against NEW duplication on top of the stronger detector.

## Budget 20 → 21 and sub-DAG anchors with line ranges

A later PR weight-grades the sub-DAG score (a larger shared computation scores higher), which
lifts one pre-existing partial-clone family past the substantial threshold — budget re-baselined
20 → 21. Each sub-DAG anchor also now carries the **source line range** of the shared computation
(stamped from the enclosing expression during value-graph evaluation), exposed per unit in
`nose features` (which emits JSON by default), as `anchors[].line_start` / `line_end`.

Those line ranges are now surfaced at the **family** level too: when every member of a clone family
shares a heavy sub-DAG, each site in the report carries `locations[].shared_subdag = [start, end]`
— its OWN source range for the shared computation — and `nose query`'s text output appends
`(shared computation: lines X-Y)` to each site. So a partial / sub-DAG clone points at *where* the
shared logic lives in every copy, not just that one exists.

## Budget 21 → 22 and receiver-method LibraryApi occurrence evidence

Moving receiver-method APIs onto admitted `LibraryApi` occurrence evidence briefly raised the
dogfooding count to 25. The new occurrence-producer and strict-exact gate duplication was real and
was deduped: receiver-method contract selection now lives in the semantic kernel, shadow checks use
one semantic helper, bulk dependency lookup reuses a cache, and strict-exact call evidence gates
share admitted-contract helpers.

The remaining 22nd family was pre-existing domain/binding helper similarity
(`domain_evidence_for_var_reference` and `binding_lhs_name` after the receiver-domain cache moved
behind `nose-semantics`). It crosses the substantial threshold because receiver-method occurrence
evidence lets the near channel recognize more of the existing semantic proof plumbing, not because
this PR added another copy. The gate budget is therefore re-baselined to 22 while continuing to
ratchet against new avoidable duplication.

## Budget 22 → 23 and the Java collection constructor exact-safe recognizer

Restoring exact-safety for wildcard-imported Java empty collection constructors (PR #141) adds one
`strict_exact_java_collection_constructor_safe` recognizer and wires it into the
`strict_exact_safe_call` dispatch chain with a single `if recognizer { return true }` line. That
one line lengthens `strict_exact_safe_call` just enough to lift a **pre-existing** near-family —
`strict_exact_safe_call` ↔ `strict_exact_in_membership_safe` — past the substantial (value ≥ 40)
line. The overlap is incidental (~4 removable lines between a recognizer dispatch and a membership
checker that merely share the early-return / `strict_exact_safe_tree`-recursion shape); it is not
extractable duplication, and merging the two would conflate unrelated responsibilities. No new copy
of the recognizer logic was introduced — the new recognizer mirrors the value graph's existing
admission check rather than re-implementing it. The gate budget is therefore re-baselined to 23
while continuing to ratchet against new avoidable duplication.

## Budget 24 → 25 and the effect-free-reorder soundness guard

The #283-A fix (coevo §CE) stops the value-graph canonicalizer from sorting the operands of a
commutative/AC operator when any operand carries an observable effect — `print(a) + print(b)` must
not converge with `print(b) + print(a)`. Holding effectful operands in source order shifts a few of
nose's own value-graph fingerprints, which nudges one **pre-existing** large-span dispatch
near-family — `interp.rs` ↔ `value_graph/eval/*` ↔ `value_graph/control/*`, sharing ~12 of ~1082
lines — past the substantial (value ≥ 40) line. That is a spurious whole-function-span match (three
big match-dispatch bodies that share a sliver of control shape), not extractable duplication and not
new code. The gate budget is re-baselined to 25 while continuing to ratchet against new avoidable
duplication.

## Budget 25 → 26 and the graded-witness module

The [graded-witness](graded-witness.md) PR (#315) adds `value_graph/value_dag.rs`. Its
`impl<'a> FileReferents<'a>` block (~270 lines) incidentally shares ~7 boilerplate lines — the
`impl<'a>` header plus a `for u in &il.units { def_*.entry(..).or_insert(..) }` skeleton — with the
`impl<'a> Builder<'a>` block in `value_graph/builders.rs`. nose's own near channel matches them at
the whole-impl span (8 varying spots, ~7 of ~270 lines shared), but the two impls do unrelated work
(value-DAG referent resolution vs the value-graph builder's dict-entry/index-write methods) — there
is nothing extractable. Another spurious whole-impl-span match, not new avoidable duplication; the
budget is re-baselined to 26.

## Budget 26 → 27 and the series-9 dataflow fix

The series-9 dataflow inline-soundness fix (oracle-value-model §7.2 — `collect_writes` records
indexed/field-store base mutations, and the inliner skips uses in conditional/repeated positions) is
fingerprint-neutral on the corpus (family delta ≈ 0), but the small structural shift pushed one
**pre-existing** test near-family over the substantial line: the two table-driven decidability-filter
tests in nose-cli's inline `tests` module, `declaration_spans_fail_open_per_language` ↔
`declaration_spans_classify_per_language`. They are near-identical by construction — a
`&[(&str, &str)]` case table plus an `assert!(…ast_classifies…)` loop, differing only in the asserted
direction — benign test scaffolding with nothing extractable. The budget is re-baselined to 27.

## Budget 27 → 28 and semantic false-merge boundaries

The semantic false-merge boundary fix moves order-comparison orientation behind integer-domain
evidence and keeps NaN/signed-zero-sensitive APIs fail-closed. That changes canonicalized value
fingerprints enough that this branch's release binary reports the same 28 substantial families even
when pointed at an unmodified `origin/main` worktree: the count increase is detector behavior, not
new copy-paste in the PR tree.

The extra counted family is the pre-existing overlap slice
`body_depends_on_iter` ↔ `foreach_effect_body_depends_on_iter` ↔ `single_branch_statement`, folded
under the broader loop-effect family in the human report. It shares the recursive "recognized
statement body" skeleton, but the two loop-effect paths deliberately differ in their effect-site
recording and recognizer contracts while `single_branch_statement` belongs to conditional-guard
summarization. Extracting it would be a high-parameter helper that couples separate detector
responsibilities, so the family is recorded as design debt and the budget is re-baselined to 28.

## Budget 36 → 55 and builtin semantic-pack migration

The builtin semantic-pack migration splits a large amount of semantic evidence coverage into
pack-owned producer/provenance tests and smaller file-length-compliant modules. Re-running the
dogfooding gate reports 55 default-surface substantial near families against the prior 36-family
baseline: 28 current IDs are newly visible and 9 old baseline IDs are no longer reported.

The new families were reviewed during the migration. Most are test scaffolding around pack-owned
`LibraryApi` evidence records, resolver hard negatives, and generated-style builtin-pack report
assertions. A few production families are known semantic-kernel plumbing that this PR intentionally
made more explicit rather than abstracting away: language-core provenance helpers, sequence-surface
provenance checks, span callee-dependency matchers, builtin evidence upsert helpers, and the
pre-existing `Builder`/`FileReferents` whole-impl span. They are candidates for later cleanup, but
deduping them inside this migration would couple unrelated pack slices and slow the safer
architecture move. The baseline is therefore refreshed to 55 while keeping the gate as a ratchet:
future increases still need dedupe or a fresh documented acceptance.

The builtin inventory report PR kept the count at 55 but refreshed two representative IDs:
`b12e6a4ee3b107b6`/`b4311277f23891dd` disappeared and
`641c27f8c0ae37ed`/`758beda0d0ed65da` appeared. The current representatives are still
test-scope semantic-pack migration debt: pack-owned `LibraryApi` record builders and
Rust map-get canonical builtin dependency/hard-negative fixtures. No new budget is accepted.

The #509 admitted API result-domain PR also keeps the count at 55 while refreshing one
representative ID: `39a46b1fa7e4804c` disappears and `0dd2be502b5af83e` appears. Both IDs
refer to the same production-scope helper family,
`call_target_evidence.rs::upsert` and
`library_api_evidence/recording.rs::upsert_builtin_evidence_with_pack_id`; adding receiver-method
result-domain emission in `recording.rs` shifts that function's source span and therefore the
family ID. This is still the reviewed builtin evidence upsert-helper debt from the migration, not
new avoidable duplication, so no new budget is accepted.

The #511 admitted API result-domain materializer PR keeps the count at 55 while refreshing two
representative IDs. `0dd2be502b5af83e` changes back to `39a46b1fa7e4804c` for the same
`call_target_evidence.rs::upsert` / `library_api_evidence/recording.rs::upsert_builtin_evidence_with_pack_id`
family after the result-domain emission path is centralized. `caf459299b305432` changes to
`be538d60b289f5ba` for the same language-core provenance helper family involving
`sequence_surface_record_has_language_core_provenance` and
`language_core_sequence_surface_record`. A new receiver-method test helper initially surfaced as
avoidable test duplication and was removed by reusing the existing receiver-method IL fixture
helper. No new budget is accepted.

The #516 CPD blind-spot recall PR first kept the count below the 55-family budget after deduping
avoidable Guava positive-fixture helper repetition. The review-hardening pass then added required
Guava hard negatives for unsupported `ImmutableMap.of` arity, static null elements/key-values, and
duplicate static map keys across frontend/result-domain, value-graph, strict-exact, and export
surfaces. The current release binary reports 56 default-surface families. The two new accepted
families are test-scope Guava hard-negative IL fixture builders repeated across the three crates
that own those independent gates:
`crates/nose-detect/src/units/tests/strict_exact_factories.rs`,
`crates/nose-normalize/src/value_graph/tests/factories/guava_factories.rs`, and
`crates/nose-semantics/src/tests/semantic_evidence/sequence_surfaces.rs`. The smaller family
(`84edbf7d317212c7`) is the shared `eleven_entry_payloads` fixture; the larger family
(`99408319bd080594`) is the Java `ImmutableMap.of` IL/evidence builder. Extracting them into
production code would couple unrelated crate test surfaces, and there is no shared test-support
crate for this boundary. The budget is therefore re-baselined to 56 while preserving the gate as a
ratchet for future production or avoidable test duplication.

The #521 Java Collections stdlib factory PR keeps the count at 56 while refreshing the same two
Guava hard-negative fixture IDs after the file-length ratchet split tests into child modules:
`84edbf7d317212c7` changes to `46c7ab6a624ab637` for the shared `eleven_entry_payloads` helper,
and `99408319bd080594` changes to `0ca8c1c2117a5fa4` for the Java `ImmutableMap.of` IL/evidence
builder family. A temporary production near-family between Java collection and map value-graph
recognizers was removed by sharing the internal Java static-member call-shape helper, so no new
budget is accepted.

The #522 Swift stdlib collection factory PR also keeps the count at 56. The Swift
`Array`/`Set`/`Dictionary(uniqueKeysWithValues:)` slice moved new tests into child modules to keep
the file-length ratchet green, and it added the general `LabeledFreeName` callee capability for
first-argument-label proof. That shifted eight representative family IDs:
`0ca8c1c2117a5fa4`, `13835f6b499ba385`, `1b239d6003d12d2f`, `26775d07eef0a114`,
`2c454f3fdff599c8`, `3ff060916c96600f`, `46c7ab6a624ab637`, and `b5c1ae278fc77802`
disappeared; `04d39fd18168311f`, `070c8818af8421e9`, `072b0b3003cf2698`,
`3280184026a6a7c9`, `4f5e190b35a2dac2`, `a72c9bc5138a4045`, `d984ca7d5210611e`, and
`dbbb03b3c0fa93e8` appeared. The new test-scope IDs are the same evidence-builder,
method-call `LibraryApi`, and Guava hard-negative fixture debt already reviewed above. The two
production-scope IDs are the existing node/span callee-dependency matcher parallelism now including
labeled free-name checks; unifying those paths would be a separate dependency-matcher abstraction,
not part of the Swift stdlib capability slice. Two avoidable draft families were removed before
acceptance by sharing the post-lower `LibraryApi` emission helper and the strict-exact
collection-factory recognizer helper. No new budget is accepted.

The #523 Go `strings.Contains` stdlib helper PR keeps the count at 56. Supporting the `Contains`
selector for both `slices` and `strings` first surfaced an avoidable production family between
post-lower and normalize receiver-method `LibraryApi` recorders; that was removed by centralizing
the receiver-method candidate/dependency-proof selection in the semantic kernel while leaving each
caller to seed and record its own evidence. The remaining drift is representative-ID churn:
`072b0b3003cf2698`, `3280184026a6a7c9`, `39a46b1fa7e4804c`, `758beda0d0ed65da`, and
`be538d60b289f5ba` disappear; `0715a8712c2fdb76`, `0a126db1cbf0faa6`,
`6faabbec4e234610`, `85074f64d038d1a0`, and `b1570372c0d34139` appear. They cover the
same reviewed evidence test helpers, canonical builtin evidence fixtures, language-core provenance
helpers, and builtin evidence upsert-helper debt from the semantic-pack migration. No new budget is
accepted.

The #525 Rust `Result` channel capability PR keeps the count at 56. Supporting `Ok`/`Err`
constructors and `is_ok`/`is_err` predicates first surfaced production family `275bb8c2e5e605a0`
between the Option and Result post-lower sum-type pattern recorders; that was removed by sharing
the free-name variable `LibraryApi` recorder while leaving each capability slice to select its own
contracts and evidence domains. The remaining drift is representative-ID churn:
`641c27f8c0ae37ed`, `6faabbec4e234610`, and `8aefdf6c558af0bc` disappear;
`78872d78308c99fd`, `868d099f88f94cfa`, and `b981263fc2a3f950` appear. The two test-scope IDs
cover the same semantic-pack migration fixture debt in `LibraryApi` record builders and stdlib
receiver/API record builders already accepted above. The production-scope ID covers the existing
language-core provenance helper family that keeps sequence-surface and import-fact records tied to
the semantic kernel. No new budget is accepted.

The #532 Rust `Result` API-evidence runtime follow-up keeps the count at 56. Caching constructor
shadow-root visibility for the Rust `Some`/`Ok`/`Err` recorder and preserving fail-closed
result-domain materialization moves line spans in `library_api_evidence`, so the already reviewed
production-scope language-core provenance helper family changes representative ID:
`868d099f88f94cfa` disappears and `cf86f9ad6c5a533a` appears. The new representative covers the
same semantic-kernel provenance records in `binding_evidence`, `library_api_evidence`,
`import_facts`, and `sequence_surface`. No new budget is accepted.

The #534 Rust iterator sequence-HOF capability PR keeps the count at 56. Moving Rust iterator
HOFs into `nose.protocols.sequence_hof_adapters`, splitting the value-graph `LibraryApi` test
helper to satisfy the file-length ratchet, and adding explicit custom-`map`/`collect_vec`
hard-negative tests shifts representative line spans without adding a new substantial family:
`0715a8712c2fdb76`, `b1570372c0d34139`, `cd016e6bfca96acb`, and `eddf659f3c346592` disappear;
`03a902cddc7077f2`, `32b92ef22cfabecd`, `8cfb7e836850848f`, and `d9278c329fce1b6b` appear. The
new IDs cover the same reviewed test/helper debt: cross-crate `evidence_with_dependencies`
fixtures, language-core evidence fixtures, Python collection factory evidence fixtures, and HOF
demand/materialization negative tests. No new budget is accepted.

The #535 Python iterator builtin capability PR also keeps the count at 56. A draft-only frontend
test helper family between `call_span_with_callee_named`, `call_span_with_field_callee_named`, and
the new Python iterator tests was removed by sharing the call-node lookup helper in the lowerer
test module. The remaining drift is representative-ID churn:
`03a902cddc7077f2`, `32b92ef22cfabecd`, `78872d78308c99fd`, and `b981263fc2a3f950` disappear;
`4184990de7be5a2e`, `48b9d4234768340d`, `8a741b956dc35bad`, and `a14558ef919c3e76` appear. The
new IDs cover the same reviewed fixture debt: cross-crate `evidence_with_dependencies` builders,
language-core evidence helpers, and semantic-pack `LibraryApi` record builders now including the
Python iterator builtin protocol record. No new budget is accepted.
