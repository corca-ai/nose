# Dogfooding history

This page preserves the detailed nose-on-nose duplication review trail. The
current gate, current baseline source of truth, and baseline-update workflow
live in [dogfooding](dogfooding.md).

## Original critical review

Goal: honestly assess whether `nose query crates all top=0 --mode near --min-value 40` produces *real* design-level
refactoring opportunities on its own codebase, act on the genuine ones, and record
where the tool is weak. The third-party counterpart is [field evaluation](field-evaluation.md);
the duplication gate that grew out of this lives in [contributing](contributing.md#the-duplication-gate-dogfooding).

Original review scope: the then-production crates only (6 Rust crates, 8-language
frontends). Result at the start of this review: 34 candidate families, ~662 duplicated
lines (the arc below took it to 23 / ~411). The numbers are a dated snapshot of *this*
review pass; re-running on today's larger codebase reports more, since the crates have
since grown.

The duplication gate that grew out of this review is the
[check-duplication script](../scripts/check-duplication.sh): it runs
`nose query crates all top=0 --mode near --min-value 40 --format json` and
compares the default-surface family IDs with
[`scripts/duplication-baseline.json`](../scripts/duplication-baseline.json). Tests are
included in the ratchet so fixture/scaffolding copy-paste stays visible instead of
being policed only by the file-length gate. A family disappearing also requires a
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
`6e37683225332c86`, `77d8e8012b2ac08a`, `d723f2396fdd67de`, `84df147de864f719`,
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

The 2026-06-26 JS/TS string-affix receiver hardening (#550) inserted stricter
case-preserving and optional-parameter TypeScript annotation helpers in
`type_domain.rs`. That moved the `rust_integer_type` span inside the
already-reviewed small predicate/helper family with `css::is_selector_kind` and
`python::expressions::is_op_tok`; the family ID changed from `7f4ff361137cc14a`
to `d723f2396fdd67de`, then to `1e76918c4878bab0`, but members, value, and
scope were unchanged.

The same #550 PR added the durable
`crates/nose-cli/tests/fixtures/string_affix_550` product-regression corpus so
closeout evidence no longer depends on `/tmp` scratch files. That fixture is
intentionally repetitive: `340cd841f428840f` is the cross-language proved
prefix family, and `f4bd533cf627ba92` is the matching JS/TS hard-negative
scaffold for optional/nullable/prototype-patched, nested-shadow, and
block-scoped-shadow receiver shapes. These are accepted test-evidence families,
not production refactor opportunities; the reviewed default-surface budget moves
from 52 to 54.

The 2026-06-26 string-affix coordinate-boundary pass (#552) adds a more focused
`string_affix_552` product-regression corpus for parameter, immutable binding,
multi-affix, and offset boundaries. With the current clustering, the earlier
cross-language proved-prefix fixture family `340cd841f428840f` is no longer
reported as a substantial default-surface near-duplicate. This tightens the
reviewed default-surface budget from 54 to 53; no new family is accepted.

The 2026-06-27 recall-loss attribution pass (#570) adds a local JSON
`soundness_gate` beside the existing human `report_falsify` path. Both walk
fingerprint groups and count hard-gate/advisory outcomes, so the current binary
now reports `6f1baed465ffcde9` for that accepted reporting/oracle grouping
overlap while the old accepted representative `e88e8f81d527af19` is no longer
reported. The reviewed default-surface count remains 53; no budget increase is
accepted.

The 2026-06-27 Rust brace-import evidence slice (#576) adds a focused Rust
frontend import-test module. That line movement changes the accepted
async/yield/try protocol-boundary scaffold family ID from `60806a4da1fcff4f` to
`55390d59f97e804b`; members and judgment stay the same (`python/tests.rs`
await/yield protocol tests plus the Rust try-expression protocol test). The
reviewed default-surface count remains 53; no budget increase is accepted.

The 2026-06-27 import-backed Rust scoped call-target slice (#578) adds focused
call-target proof and hard-negative tests. The current detector no longer
reports two accepted test-scaffold representatives (`0b353c6f21118d2d`,
`522c5d5dc73163e7`) and instead reports `cc9936001342542f`, a test-only
construction scaffold shared by direct/scoped call-target evidence fixtures,
plus `016becf550d84d34`, the tiny shared `sp` helper repeated across test
support and local fixture modules. These are accepted as fixture debt for named
fail-closed scenarios, not production refactoring debt. The reviewed
default-surface count remains 53; no budget increase is accepted.

The 2026-06-27 Rust struct-expression surface slice (#580) adds a
`rust_struct_expression` sequence-surface contract row and moves Rust
struct-literal surface assertions into a focused module. This shifts two
already accepted representative IDs without increasing the reviewed
default-surface count: `55390d59f97e804b` becomes `57be5bd4067b5967` for the
source-backed async/yield/try protocol-boundary fixture family, and
`cf86f9ad6c5a533a` becomes `d48c1b96caba9588` for the semantic-kernel
language-core provenance helper family covering binding, library API, import
facts, and sequence-surface records. The reviewed default-surface count remains
53; no budget increase is accepted.

The 2026-06-27 receiver-domain recovery slice (#582) changes value-graph
module/static binding seeding enough to move the already accepted
context/export assignment-counting family from `209fdc39157ececd` to
`d0198581ac228459`; the members and judgment remain the same
(`value_graph/context.rs` module binding seeding and
`module_imports/exports.rs` imported literal export collection). The same
self-query now reports `318c7eb92b77189f`, a production value-graph whole-impl
span between `field_state.rs` and `stdlib/bindings.rs` with one shared line and
one removable line. That is detector span noise over small helper impl blocks,
not extractable duplication. The reviewed default-surface count moves from
53 to 54; the budget increase accepts this surfacing without accepting new
avoidable duplication.

The 2026-06-28 Rust module-resolution slice (#587 1-3) moves imported literal
export collection into a richer context-aware shape. That shifts the already
accepted context/export assignment-counting family ID from `d0198581ac228459`
to `5d2ee58ae63af599`; the members and judgment remain the same
(`value_graph/context.rs` module binding seeding and
`module_imports/exports.rs` imported literal export collection). The reviewed
default-surface count remains 54; no budget increase is accepted.

The 2026-06-28 Rust direct re-export slice (#587) adds one-hop public `use`
alias evidence to imported literal export collection. This shifts three already
reviewed production representatives without increasing the default-surface
count: the context/export assignment-counting family moves from
`5d2ee58ae63af599` to `2a3ff0019f8a1765`; the value-graph whole-impl span
moves from `61c01561e227df11` to `2a5aa3db45d33592` after `value_dag.rs`
learns to ignore re-export alias proof as a value referent; and the
semantic-kernel language-core provenance helper family moves from
`d48c1b96caba9588` to `40de0ff958ad1b55` as
`module_imports/exports.rs::trusted_language_core_record` joins the existing
binding/library/import/sequence provenance shape. These remain reviewed
plumbing/span-noise families, not new avoidable duplication. The reviewed
default-surface count remains 54; no budget increase is accepted.

The 2026-06-29 CI repair splits overlong recall-loss reporting,
exact-admission attribution, library-API idiom tests, strict-exact factory
tests, and post-lower LibraryApi dispatch into file-length-compliant modules.
That moves five already reviewed representatives without increasing the
default-surface count: the reporting/oracle overlap moves from
`190f0a721624d635` to `1f785cf0498fe78d`; the semantic-kernel
language-core provenance helper family moves from `40de0ff958ad1b55` to
`475fa037b992d31d`; the guard/static-import/library-API semantic query
harness moves from `84df147de864f719` to `e91ec2b8c9d99c30`; the
`eleven_entry_payloads` fixture family moves from `678befa6db9e5c5d` to
`f2646ca6f31a0c0b`; and the Guava map factory IL builder family moves from
`8a02be14d3980cd3` to `6b3b0c88a12efe80`. A newly surfaced
`verify_admission` attribution-helper family was deduped by extracting the
shared `visit_subtree` traversal, so the reviewed default-surface count
remains 54; no budget increase is accepted.

The 2026-06-29 Promise local-continuation recovery slice moves the expanded
Promise value-graph fixtures into `value_graph/tests/support/promise.rs` to
stay under the file-length ratchet. That changes three already reviewed
fixture-helper representatives without increasing the default-surface count:
the tiny shared `sp` helper moves from `016becf550d84d34` to
`17b5d7672d8502a0`; the language-core evidence helper family moves from
`1bcf5beffb5c2932` to `60083cfa5d4da06d`; and the cross-crate
`evidence_with_dependencies` builder family moves from `42cc257ba613ae19`
to `dec85ed3ec0be74e`. These remain accepted test/helper debt, not new
production refactoring debt. The reviewed default-surface count remains 54;
no budget increase is accepted.

The 2026-06-29 Promise `.finally` settlement recovery slice first surfaced two
avoidable test-helper families: a copied `promise_static_call` fixture builder
and parallel Promise continuation admission checks. Both were deduped in the
same change by sharing the Promise static-call helper and extracting
`push_promise_receiver_api_evidence`. The remaining drift is representative
churn for the same reviewed helper families: `sp` moves from
`17b5d7672d8502a0` to `314d2b14120bd2a4`, language-core evidence helpers move
from `60083cfa5d4da06d` to `fbd1cc4abe27b98d`, and
`evidence_with_dependencies` moves from `dec85ed3ec0be74e` to
`ac51c88919bc3122`. The reviewed default-surface count remains 53; no budget
increase is accepted.

The 2026-06-29 Promise imported settled-value contract slice first surfaced
avoidable duplication in the new builtin settled-value evidence scanner and
Promise settled-value tests. The scanner now reuses the semantic facade's
unique asserted record resolver, and the tests share the imported-function
target fixture. The remaining drift is representative churn for reviewed
families: the `sp` helper moves from `314d2b14120bd2a4` to
`99f8c8a9192a0930`; language-core evidence helpers move from
`fbd1cc4abe27b98d` to `8fba27133717de21`; `evidence_with_dependencies`
moves from `ac51c88919bc3122` to `52cb1ae313158c0c`; the production
`source_*_at_node` accessor family moves from `56b37252f08a8696` to
`1b952c1370c4f637` after evidence resolver line movement; and the
direct-call-target negative fixture family moves from `1af33fd980c0e8b9`
to `5e2b99978272a129`. The reviewed default-surface count remains 53; no
budget increase is accepted.

The 2026-06-29 Node timers Promise-domain slice adds the dependency-backed
`JsImportedPromiseFactory` contract and splits LibraryApi occurrence recording
into a focused helper module. That moves the already reviewed semantic-kernel
language-core provenance helper family from `475fa037b992d31d` to
`c67046264486c003`; the members and judgment remain the same
(`trusted_language_core_record`, sequence-surface provenance helpers, and
language-core record constructors across frontend, normalize, and semantics).
The reviewed default-surface count remains 53; no budget increase is accepted.

The 2026-06-29 Node timers safe-payload slice reuses the imported Promise
settled-value kernel for the no-options `setTimeout(delay, value)` and
`setImmediate(value)` arities. It only moves the same reviewed
semantic-kernel language-core provenance helper family from
`c67046264486c003` to `46eafe785a6f3517`; the members and judgment remain the
same, and no new substantial default-surface duplicate family is accepted. The
reviewed default-surface count remains 53; no budget increase is accepted.

The 2026-06-29 `Promise.all` literal aggregate slice first surfaced one
avoidable production helper family: the new qualified-global symbol evidence
lookup repeated the existing asserted-evidence lookup shape. Rewriting that
lookup as an explicit proof check removed the new family. The remaining drift
only moves the same reviewed semantic-kernel language-core provenance helper
family from `46eafe785a6f3517` to `551e7992e1632597`; the members and judgment
remain the same (`trusted_language_core_record`, sequence-surface provenance
helpers, and language-core record constructors across frontend, normalize, and
semantics). The reviewed default-surface count remains 53; no budget increase
is accepted.

The 2026-06-30 oracle-exclusion obligation reporting slice adds diagnostics-only
rollups for runtime/protocol units that fail closed before oracle
interpretation. That shifts the already reviewed reporting/oracle overlap
family from `1f785cf0498fe78d` to `cdfa7334daa1135b`. The 2026-07-05
`recall_loss_report` module split moves the same reviewed family again to
`41586dd06dff63b5`; the members remain `verify_report.rs::report_falsify` and
`recall_loss_report.rs::soundness_gate`. This is representative-ID churn from
nearby report-shape movement, not new extractable duplication. The reviewed
default-surface count remains unchanged; no budget increase is accepted.

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

## Re-run (2026-06-05, while versioning machine JSON)

The duplication gate reported 6 substantial near-duplicate families against a budget of 4.
The two additional families were not introduced by the machine-JSON schema work: the PR changed
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

The #536 JS/TS Array HOF capability PR first keeps the count at 56. Adding exact Array receiver
obligations and JS Array-pack HOF provenance shifts representative spans in the already reviewed
semantic-evidence and HOF-demand test scaffolding:
`4184990de7be5a2e`, `48b9d4234768340d`, `8cfb7e836850848f`, `d7dea9009200ed08`, and
`d9278c329fce1b6b` disappear; `1096a4a828c21a80`, `1a260c845757db00`,
`1bcf5beffb5c2932`, `42cc257ba613ae19`, and `4e655a7c9a3d22dd` appear. The new IDs cover
the same accepted fixture debt: cross-crate `evidence_with_dependencies` builders, language-core
evidence helpers, Python collection-factory `LibraryApi` record helpers, receiver-domain
fail-closed tests, and HOF demand/materialization negatives now carrying JS Array HOF pack
evidence. The sparse-array hard-negative then moves two more existing representatives:
`3e76e062e630928d` and `7df8f46c267d1092` disappear; `2b26aa8a17d81eae` and
`7b134f23e922f405` appear. Those cover the already reviewed `evidence_anchor_span` helper
family and per-frontend call/constructor lowering parallelism. The callback-obligation review fix
then drops the count from 56 to 54: `1267c115f7832175` and `f380654d807c1e90` disappear because the
method-call and typed/free call IL fixtures now share the inline callback shape required by JS Array
HOF admission. A final release-binary rebuild after the nested normalized-HOF callback fix moves one
value-graph collection representative: `1639812e75927a23` disappears and `44bfd76822ddbe95`
appears. The new representative is a whole-impl-span `cardinality`/small `reductions` match with
only 2 shared lines, not new extractable duplication. No new family appears, so the baseline budget
is tightened to 54.

The #537 Swift Sequence HOF capability PR tightens the count from 54 to 53. The first draft
surfaced two avoidable test-scope families while adding Swift `map`/`filter`/`flatMap` admission:
JS/Swift callback fixture builders and receiver-domain fail-closed IL setup. Both were deduped by
sharing a callback fixture node helper in the admission resolver support module and a named
cid-param/receiver fixture in receiver-domain tests. After that cleanup, accepted representative
`1096a4a828c21a80` no longer reports and no new family appears, so the baseline budget is tightened
again.

The #538 Ruby Enumerable HOF capability PR keeps the count at 53. The first draft surfaced
avoidable test-scope families while adding Ruby `map`/`collect`/`select`/`filter`/`reject`
admission: Ruby/Swift sequence-HOF pack requirement tests and HOF demand fixture setup. Those were
deduped by sharing the ordered sequence-HOF pack requirement helper and the map/predicate HOF
fixture builders. The remaining drift is representative-ID churn: `4e655a7c9a3d22dd` disappears
and `d836cac640ba27ba` appears. The new representative covers the same reviewed HOF
demand/materialization negative scaffolding, now anchored on the shared `map_len_il_with_lambda`
helper plus the strict-exact pull-lazy `len` boundary test. No new budget is accepted.

The #557 string affix protocol-pack extraction tightens the count from 53 to 52. Adding
`string_affix_call_il` first joined the existing receiver-method `LibraryApi` fixture family
covering `map_get_default_call_il`, `map_key_view_call_il`, and
`receiver_membership_call_il`. That was avoidable test scaffolding, so the resolver tests now share
one named `receiver_method_call_il` helper. Accepted representative `b7a3fa1f37880138` no longer
reports and no new default-surface family appears.

The #567 import-snapshot recall-loss census keeps the count at 54 after the
earlier phase 1/2 imported-provider snapshot fixtures expanded the baseline.
Adding `import_snapshot_census` to the local recall-loss report shifts the
already reviewed reporting/oracle representative from `6f1baed465ffcde9` to
`190f0a721624d635`. The reported locations remain the same
`report_falsify`/`soundness_gate` production overlap recorded for #570, so this
is representative-ID churn rather than new extractable duplication. No new
budget is accepted.

The #567 aggregate-boundary triage keeps the count at 54. Adding focused
semantic evidence tests shifts two already reviewed Guava/collection-factory
test helper representatives: `070c8818af8421e9` becomes `678befa6db9e5c5d`
for the repeated `eleven_entry_payloads` hard-negative fixture, and
`d984ca7d5210611e` becomes `8a02be14d3980cd3` for the Guava map factory IL
builder family across detect, normalize, and semantics tests. These remain
test-scope fixture debt already accepted in the collection-factory capability
work; no new budget is accepted.

The Promise async-function return recovery slice tightens the count from 54 to
53. The first draft surfaced an avoidable production family between the new
call-target evidence `upsert` helper and the existing `LibraryApi` evidence
recorder; splitting the call-target match lookup removed that production
family. Adding focused direct-call evidence tests also shifted representative
spans for an already reviewed direct-call-target fixture family: `0a126db1cbf0faa6`
and `cc9936001342542f` no longer report, while `727e41b9e3e96f1e` reports the
same test-scope call-target negative scaffolding. No new budget is accepted, so
the baseline budget is tightened to 53.

The Promise direct-function return recovery slice keeps the count at 53. The
first draft surfaced avoidable duplication across the new Promise call-target
evidence tests; extracting a shared direct-return fixture removed that family.
The only remaining delta is representative-ID churn for the same reviewed
direct-call-target negative fixture family: `727e41b9e3e96f1e` no longer
reports, and `32ed015840375d04` reports the same three test-scope locations
(`does_not_emit_*` direct-call-target negatives plus the semantics selector
shape guard). No new budget is accepted.

The Promise branch-return producer recovery slice also keeps the count at 53.
The DirectMethod branch-return test initially surfaced a small five-line
test-fixture family against the semantics DirectMethod selector guard; extracting
shared Promise-like and DirectMethod fixture helpers removed that new family.
The remaining delta is the same reviewed direct-call-target negative fixture
family moving from `32ed015840375d04` to `1af33fd980c0e8b9` after new branch
fixtures shifted line spans. No new budget is accepted.

The cross-language async-function obligation reporting slice tightens the count
from 53 to 52. The first draft added a third Python protocol-boundary lowering
test and surfaced avoidable test scaffolding duplication against the existing
await/yield protocol tests; extracting `expect_python_protocol_boundary` removed
that new family. After cleanup, accepted representative `57be5bd4067b5967` no
longer reports and no new default-surface family appears, so the baseline budget
is tightened.

The follow-up review fix for that slice keeps the count at 52. Gating JS
`PromiseLike` producer evidence away from Python/Rust/Swift async functions adds
one focused negative test, which shifts the already reviewed direct-call-target
negative fixture representative from `5e2b99978272a129` to
`f6a2c8af9c3fd791`. The reported locations are still the same test-scope
`does_not_emit_*` direct-call-target negatives plus the semantics selector shape
guard. No new budget is accepted.

The async protocol near-channel mirror slice keeps the count at 52. Extending
the value-graph dual-view handling from `await` to supported async protocol
boundaries moves a large evaluator whole-impl span: accepted representative
`c9fe4dc9d9cd14f5` no longer reports, and `149bb759833d2d51` now covers
`interp/eval.rs`'s oracle evaluator impl and `value_graph/eval/core.rs`'s
value-graph evaluator impl. The family is a `shared-sub-dag` whole-impl match
with 7 shared/removable lines and 8 parameter spots across two different
execution models. Extracting it would couple oracle interpretation to
fingerprint construction, so this is recorded as evaluator span noise rather
than avoidable duplication. No new budget is accepted.

The Java Future/Executor local and `this`-field receiver provenance slice keeps
the count at 52. Recording exact Java receiver-domain evidence shifts line
spans in `java/expressions.rs`, so two already reviewed frontend-lowering
representatives move without changing members or values:
`7b134f23e922f405` becomes `596f602568ace201` for the per-frontend
call/constructor/enum-constant lowering family, and `6e37683225332c86`
becomes `a54e8f6b173a160a` for the Java/C expression-dispatch family. Both are
the same language-frontend parallelism recorded above; no new budget is
accepted.

The Java `CompletableFuture` constructor/package-shadow reporting slice keeps
the count at 52. The first duplication-gate run surfaced a real, avoidable
near-family between the Java collection constructor lowerer and the new
`CompletableFuture` constructor lowerer; extracting the shared construct-call
builder removed it. The remaining delta is representative churn for the same
reviewed frontend-lowering families: `596f602568ace201` becomes
`ac31c3c9bc390d55` for the per-frontend call/constructor/enum-constant lowering
family, and `a54e8f6b173a160a` becomes `ebf5e40476ceff32` for the Java/C
expression-dispatch family. No new budget is accepted.

The 0.17.0 release-prep performance slice keeps the count at 52. Making
import-use indexes lazy and tightening release-runtime hot paths shifts two
already reviewed representatives: `2a3ff0019f8a1765` becomes
`1d565f1e57ac5d8b` for the context/export assignment-counting family
(`value_graph/context.rs` module binding seeding plus
`module_imports/exports.rs` literal export collection), and
`551e7992e1632597` becomes `7c8432da3fcb2c67` for the semantic-kernel
language-core provenance helper family (`trusted_language_core_record`,
sequence-surface provenance helpers, and language-core record constructors).
The first family is still domain-parallel top-level binding triage, not a
shared abstraction worth extracting; the second remains kernel provenance
plumbing. No new budget is accepted.

The post-0.17.0 runtime pass moves the reviewed default-surface count from 52
to 53. Two IDs are representative churn from profile-guided candidate/indexing
changes: `1d565f1e57ac5d8b` becomes `c967b3bcff5a2b58` for the same
context/export assignment-counting family, and `77d8e8012b2ac08a` becomes
`a7f4d8398c1920e6` for the same `query_opportunities` origin-hint/reason
family (`origin_extract_hint` and `hint_reasons`). The new family,
`cc48beefc6a85976`, is a real production cross-crate similarity between
Markdown fingerprint candidate-pair generation and semantic anchor candidate
generation. Both enumerate bounded pairs from bucketed members, but they live
in different engines with different bucket semantics, stop rules, and output
policies. Pulling that into a lower shared crate would add an abstraction for
loop shape rather than domain meaning, so it is accepted as visible algorithm
debt rather than deduped in this CI repair.

The query-opportunities dogfood cleanup tightens the count from 53 to 52.
The self-query report flagged `a7f4d8398c1920e6`, the local production overlap
between `origin_extract_hint` and `hint_reasons`, and correctly warned that a
large extraction would need too many parameters. Extracting the smaller
invariant `OriginFactSummary` keeps the public query API unchanged while sharing
the domain/body/subkind/name facts used by both decisions. The family no longer
reports, so the baseline removes `a7f4d8398c1920e6` and tightens the ratchet.

The switch-label dogfood cleanup tightens the count from 52 to 51. The
self-query report flagged `f57a5ee0ebbdf114`, the identical production
OR-chain fold used by Java switch expressions, JS/TS switch cases, and the
shared C-family `switch_to_if_chain` helper. Unlike the cross-engine candidate
pair loop above, this is one frontend-domain operation: turn case labels into
`scrutinee == label` conditions joined by `Or`. Moving it to
`lower::fold_switch_labels` removes the family while preserving lowering order.
The paired product query-regression on `axios`, `date-fns`, `pixijs`, `jsoup`,
and `guava` reported byte-identical JSON hashes for every repo, and the
5-iteration runtime triage was neutral at aggregate scale (+0.6%, with only
small/noisy repo-level deltas).

The fragment block-shape dogfood cleanup tightens the count from 51 to 50.
The self-query report flagged `4ac4a88371e43e72`, the repeated production
pattern where conditional/direct-effect and self-field recognizers accept an
empty branch block, accept exactly one child, and reject multi-statement blocks.
Extracting the crate-internal `empty_or_single_block_child` helper keeps the
effect-specific recognizers local while single-sourcing the shared IL block
shape. The cleanup also shifts the already reviewed fragment span-noise family
from `bf4255f2994b1d65` to `9a228db20ad1a68b`; members, value, and judgment
remain the same. Product query-regression on `guava`, `jsoup`, `axios`,
`requests`, `hugo`, and `alacritty` reported byte-identical JSON hashes for
every repo. The 5-iteration runtime triage was neutral at aggregate scale
(+0.3%), with only `small-or-noisy` positive deltas.

The strict-exact HOF dogfood cleanup tightens the count from 50 to 49. The
self-query report flagged `f010e9908081b902`, the local production overlap
between `strict_exact_safe_hof`, `strict_exact_terminal_reduction_arg_safe`,
and `strict_exact_len_arg_safe`. Extracting `StrictExactHofUse` keeps the three
admission policies explicit: general tree HOFs, terminal reduction arguments,
and `len` arguments still allow different comprehension and demand profiles
while sharing the common source-comprehension / HOF payload / children-safe
flow. Product query-regression on `axios`, `date-fns`, `pixijs`, `requests`,
`boltons`, and `guava` reported byte-identical JSON hashes for every repo. The
first 5-iteration runtime triage was neutral at aggregate scale (+1.8%) but
flagged `pixijs` once as a value hot-path candidate; a focused 9-iteration
rerun on `pixijs` classified the delta as `small-or-noisy` (+1.9%,
hash-identical), so no performance regression is accepted.

The documentation/design cleanup keeps the reviewed default-surface count at
49 but fixes a scope attribution error surfaced by the same self-query loop.
The `6a34db62d8` family in `crates/nose-frontend/src/go/tests.rs` is modular
Rust test code (`#[cfg(test)] mod tests;` lowered through `src/*/tests.rs`),
not production frontend code. Moving the path convention into one shared
`nose-detect` helper makes report scoping and unit-extraction gates agree:
`test.rs`/`tests.rs`, root `spec/`, and root `__tests__/` paths now classify as
test scope everywhere. Re-running
`nose query crates all top=30 --mode near --min-value 40 group=dir` reports
the Go frontend helper family as `test`, and `scripts/check-duplication.sh`
still reports `49` default-surface families against budget `49`; no budget
change is accepted.

The callee-dependency matcher cleanup tightens the count from 49 to 48. The
self-query report flagged `04d39fd18168311f`, the static-import half of the
node/span LibraryApi callee-dependency matcher parallelism first accepted during
the Swift stdlib capability slice. The node-backed call matcher now extracts
call/callee/receiver spans, checks the ordinary callee shape, and delegates to
the span-backed dependency matcher for non-method callees. Method-family callees
still use call-aware dependency extraction so argument-sensitive contracts such
as Rust `Iterator::zip` keep requiring both receiver and pair-argument protocol
proofs. This removed the static-import parallel family and the now-dead
node-specific dependency helper wrappers while preserving the node-anchored
`FreeName`/`Property` admission path. The remaining `dbbb03b3c0fa93e8` family is
no longer node/span drift; it is same-file similarity between named-callee and
static-import span matchers with nine varying spots, so extracting it would
create a high-parameter helper rather than a clearer policy owner. No new budget
is accepted.

The frontend test-helper cleanup tightens the count from 48 to 47. The
self-query report flagged `6a34db62d843f27d`, repeated `raw_names`/`seq_names`
and adjacent payload-name collectors across Go, Java, Ruby, Python, Rust, and
Swift frontend tests. Moving the `NodeKind` + `Payload::Name` extraction into
the crate-local `test_helpers::payload_names_for_kind` keeps per-language
lowering fixtures local while removing the repeated name-collection loop. No
new family appears, so the baseline budget is tightened to 47.

The language-core provenance cleanup tightens the count from 47 to 46. The
self-query report flagged `7c8432da3fcb2c67`, the repeated builtin
language-core provenance checks across frontend module exports, normalize
sequence-surface consumers, and semantic import/sequence-surface admission.
Moving the provenance-only and asserted/dependency-closed checks into
`nose-semantics` removes that production helper family. Two accepted production
families only changed representative IDs: `1b952c1370c4f637` becomes
`7a67923c4ee93aca` for the `source_*_at_node` accessor family after evidence
facade line movement, and `c967b3bcff5a2b58` becomes `fc4edd8f87a8a8f0` for the
same context/export assignment-counting family. The reviewer-driven follow-up
that routed symbol and iterator admission through the same helper also moves the
already reviewed `evidence_anchor_span` helper representative from
`2b26aa8a17d81eae` to `734ee4c50e4d001e`. No new family appears, so the baseline
budget is tightened to 46.

The multiset-similarity cleanup tightens the count from 46 to 45. The self-query
report flagged `1fc08105c8b5d5c0`, the repeated sorted multiset/set
intersection loop across the code-clone detector, CLI verify reporting, and the
Markdown prose engine. Moving the CLI verify and recall-loss reports to the
`nose_detect::multiset_jaccard` helper removes the avoidable code-clone/CLI
duplication without making the deliberately separate Markdown prose engine depend
on the code-clone engine. The same line movement changes the already reviewed
`report_falsify`/`soundness_gate` representative from `41586dd06dff63b5` to
`1f922efb624c7f79`. No new family appears, so the baseline budget is tightened
to 45.

The source-fact accessor cleanup tightens the count from 45 to 44. The self-query
report flagged `7a67923c4ee93aca`, the repeated `source_*_at_node` lookup shape
inside `nose-semantics/src/evidence.rs`. Moving the common span-keyed
`SourceFactKind` projection into `source_fact_value_at_node` keeps the
provenance-specialized `source_cast_at_node` path separate while removing the
repeated asserted-evidence lookup wrappers. No new family appears, so the
baseline budget is tightened to 44.

The cross-crate evidence-fixture cleanup tightens the count from 44 to 42. The
self-query report flagged `52cb1ae313158c0c`, repeated compatibility-pack
`EvidenceRecord` fixture construction across normalize, semantics, and detect
tests. `EvidenceRecord::new`, `EvidenceRecord::builtin`, and the feature-gated
`nose-semantics::test_support` fixture builders remove that repeated record
literal shape while keeping crate-local fixture names stable. The cleanup also
removes the already reviewed language-core evidence helper representatives
`8fba27133717de21` and `99f8c8a9192a0930`. Three accepted test-scope
representatives move with the line changes: `4f5e190b35a2dac2` becomes
`8977e7bce9b8d9a5` for the `library_api_contract_evidence` fixture pair,
`a72c9bc5138a4045` becomes `e5b8f23a075e9657` for the
`method_call_library_api_evidence` fixture pair, and `eadc678efab56738` is the
tiny `sp` helper representative. No production family appears, so the baseline
budget is tightened to 42.

The LibraryApi fixture-builder cleanup tightens the count from 42 to 39. The
self-query report flagged `8a741b956dc35bad` and `a14558ef919c3e76`, repeated
LibraryApi test record wrappers in `nose-semantics`, plus `1a260c845757db00`,
the Python collection-factory fixture overlap between `nose-semantics` and
`nose-normalize`. Moving the semantics wrappers through a test-local
`LibraryApiFixturePack`/`LibraryApiFixtureContract` path and routing provenanced
records through `EvidenceRecord::builtin` removes the repeated contract
id/callee/arity/provenance literal shape while preserving each fixture's named
pack policy. No new family appears, so the baseline budget is tightened to 39.

The exact-fragment ordered-branch fixture cleanup tightens the count from 39 to
37. The self-query report flagged `4890b7227d416249` and `b527e97155167c1b`,
the repeated temp-project/query/assertion skeleton across ordered conditional,
ordered loop-conditional, and ordered effect branch CLI tests. Moving the
fragment-only query path and branch positive/negative checks into
`exact_fragments/support.rs` keeps each fixture matrix local while removing the
duplicated harness. No new family appears, so the baseline budget is tightened
to 37.

The exact-fragment branch-shape cleanup keeps the count at 37 while narrowing
the reviewed production fragment mirror representative. The old
`9a228db20ad1a68b` family mixed contract loop-effect recognition, legacy
loop-effect predicates, conditional guard summarization, and self-field branch
handling. Moving the shared `if` branch-block shape into
`il_utils::if_branch_blocks` removes that broad cross-shape representative, and
splitting the loop-effect body scanners keeps the contract recognizer and the
legacy differential guard independent. The remaining `5ad08a3c9ab9f5c3`
representative is the narrower two-copy loop temp-window scanner mirror. It is
kept as reviewed differential-test debt rather than extracted into a shared
recognizer helper, because sharing that scanner would couple the production
contract path to its independent predicate oracle. No new budget is accepted.

The value-graph binary node inspection cleanup tightens the count from 37 to
36. The self-query report flagged `1dfaba2582163d7c`, a whole-impl span between
Phi selection canonicalization and loop-reduction recognition. The semantic
policies stay separate: selection still owns ternary/Phi canonicalization, and
reductions still own accumulator-step recognition. The cleanup only generalizes
the existing `bin_args` structural helper to `bin_op_args`, then routes both
modules through that low-level `ValOp::Bin` argument reader. That removes the
reviewed cross-module representative without introducing a shared min/max or
reduction policy helper. The already reviewed `2a5aa3db45d33592` builder/value
DAG whole-impl span and `149bb759833d2d51` oracle/value-graph evaluator span
remain accepted span-noise families.

The UnionFind boundary review leaves `e633f3912604730d` accepted. The duplicate
shape is real, but it crosses the deliberately separate code-clone and Markdown
prose engines: `nose-detect` owns clone clustering over IL units, while
`nose-markdown` is a self-contained prose detector with no `nose-detect`
dependency. Introducing a shared crate or making Markdown depend on the code
detector just for this data structure would make dependency direction less clear
than the duplicated implementation. Keep the family visible under the ratchet
until a broader shared utility crate exists for multiple domain-neutral
structures, not just this one helper.

The pre-large-work design/docs cleanup tightens the count from 36 to 34. The
self-query report no longer reports `9510e3368e161f45`, the repeated
`language_profile.rs` language/provenance match family, because
`language_core_evidence_provenance_hashes` now derives both stable hashes from
`language_core_evidence_provenance` instead of maintaining a second language
match. It also no longer reports `936f238ab2e0d6b2`, the production Raw-node
classification overlap between frontend coverage and CLI gap-impact
diagnostics, because `nose_frontend::raw_node_surface` now owns the shared
`NodeKind::Raw`/surface-kind/boundary projection. No new default-surface family
appears, so the baseline budget is tightened to 34.

The verify soundness classification cleanup tightens the count from 34 to 33.
The self-query report no longer reports `1f922efb624c7f79`, the reviewed
overlap between `verify_report.rs::report_falsify` and
`recall_loss_report.rs::soundness_gate`. `verify_soundness` now owns the shared
fingerprint-group classification, while falsification search consumes the same
hard-gate equal-pair selection instead of repeating the grouping logic. No new
default-surface family appears, so the baseline budget is tightened to 33.

The LibraryApi test evidence builder integration tightens the count from 33 to
31. The self-query report no longer reports `8977e7bce9b8d9a5`, the repeated
`library_api_contract_evidence` fixture pair across normalize and detect tests,
or `e5b8f23a075e9657`, the repeated detect-only
`method_call_library_api_evidence` fixture pair. `nose_semantics::test_support`
now owns the shared LibraryApi contract evidence construction and builtin
method-call provenance policy, while crate-local helpers keep their scenario
names. The already reviewed tiny `sp` test helper representative moves from
`eadc678efab56738` to `e46ab190592b0850`; it still has no meaningful shared
removable body. No production family appears, so the baseline budget is
tightened to 31.

The Guava map factory fixture cleanup tightens the count from 31 to 29. The
self-query report no longer reports `6b3b0c88a12efe80`, the repeated
`ImmutableMap.of` hard-negative IL/evidence builder across semantic evidence,
value-graph, and strict-exact tests, or `f2646ca6f31a0c0b`, the identical
unsupported-arity `eleven_entry_payloads` helper. `nose_semantics::test_support`
now owns the shared Guava map fixture, including import evidence and Guava
LibraryApi occurrence provenance, while callers keep their axis-specific root
shape, Unit metadata, and span-line choices. The already reviewed tiny `sp` test
helper representative moves from `e46ab190592b0850` to `c2c6dbcb3016eb40`; it
still has no meaningful shared removable body. No production family appears, so
the baseline budget is tightened to 29.

The callee-dependency matcher policy-helper cleanup keeps the reviewed
default-surface count at 29. The span matcher now names the repeated free-name
proof plus shadow-safety check and the static receiver import plus shadow-safety
check as local helpers. The remaining named-callee/static-import matcher family
moves from `dbbb03b3c0fa93e8` to `95e83331abfa623f`, with the shared body reduced
from 21 to 18 lines and varying spots from 9 to 7. It is still same-file
policy-shape similarity rather than a clearer shared abstraction, so no new
budget is accepted.

The admission-resolver dogfooding debt burn-down (#679) tightens the count from
29 to 26. The self-query no longer reports three avoidable test-fixture
families: `f88aeebdec4f2c68`, the Rust `HashMap::from` call/span pack-provenance
pair; `e6d039006310127f`, the Java `Map.of` call/span pack-provenance pair; and
`7c1aef5590dfeefc`, the JavaScript `Set`/`Map` constructor pack-provenance pair.
Those tests now share narrow admission-provenance helpers that preserve the
raw-shape, missing-dependency, wrong-pack, wrong-producer, external-emitter, and
admitted-case assertions with resolver-specific failure labels. The already
reviewed Java collection factory call/span scaffold remains visible but moves
from `e3fa2e4c707e342a` to `eb2f9fe7da72f8dd` because line movement changed the
representative span; its members and judgment are unchanged. No production code
or query hot path changed, so no runtime degradation is accepted.

The proof-carrying frontier gate keeps the count at 26. Adding
`bench/type4/proof_carrying_frontier.py` shifts the reviewed cross-engine
candidate-pair enumeration representative from `cc48beefc6a85976` to
`ec66a9b9f2569018`. The current family is still
`crates/nose-markdown/src/fingerprint.rs::candidate_pairs` paired with
`crates/nose-detect/src/candidates.rs::anchor_candidates`, with the same
judgment as the post-0.17.0 runtime pass: visible cross-engine algorithm debt,
not a reason to introduce a lower-layer utility dependency for one helper. No
new budget is accepted.

The numeric-clamp proof-facts slice moves the reviewed default-surface count
from 26 to 27 after deduping the avoidable test-helper family
`b1d3bffa405a6386` in the new clamp proof fixtures. The remaining new family
`d9eaf862103c02a7` is a production whole-impl span between
`interp/exec.rs` and `value_graph/control/statements.rs`: the drilldown reports
only 16 shared lines across roughly 270-line `NodeKind` dispatchers, with 14
varying spots. This is detector span noise from two intentionally independent
statement walkers, not a useful base class/helper extraction; coupling the
interpreter to the value-graph builder would make the boundary worse. The
budget is raised to 27 for this reviewed span-noise family.

The Python loop/De Morgan detector admission moves the reviewed default-surface
count from 27 to 30. The new logical De Morgan plus literal-absence
canonicalization improves nose's own near-channel enough to surface three
pre-existing production families: `856ea94f585f0c67`, the integer and float
binary operator dispatchers in `interp/ops.rs`; `f918559454acf9c4`, the Java map
factory enum projections across constructor contracts and LibraryApi rows; and
`ea682c9db2126d8a`, the language profile/type-domain vocabulary projections.
These are real review signals, but none is useful to extract in this PR:
numeric dispatch keeps integer wrapping, float IEEE, and division/modulo
boundaries explicit; Java map factory projections intentionally keep argument
domain, contract-key, and pack-id vocabulary separate; and the language profile
matches are small exhaustiveness boundaries over different semantic surfaces.
The budget is raised to 30 for this detector-improvement surfacing, not for new
avoidable duplication.

The dense-literal TypeScript `every` proof-boundary slice keeps the reviewed
default-surface count at 30. The first CI pass surfaced avoidable draft
duplication: repeated JS-like language predicates and normalize idiom fixture
scaffolding. The shared `nose_semantics::js_like_lang` predicate and the common
normalize test module builders remove those families before baseline update.
The only remaining drift is representative churn: `44bfd76822ddbe95` no longer
reports and `2fb6d5f9c8c6d045` appears for the same reviewed value-graph
collection span-noise class, now represented by a broad
`collections/cardinality.rs` impl span and a small `collections/reductions.rs`
span with only two shared lines. This is still detector span noise, not a
useful collection-policy abstraction. No new budget is accepted.

The option absence-channel identity slice moves the reviewed default-surface
count from 30 to 31. The first pass surfaced an avoidable null-presence semantic
query fixture scaffold; switching that test to the shared `make_temp_dir` /
`write_files` helpers removed it before baseline update. Two remaining deltas
are representative churn for already reviewed semantic-query harness families:
`98f5617cbcf09658` moves to `1c3ae05377f90f1c` for the `typeof`/literal-key
boundary scaffold, and `e91ec2b8c9d99c30` moves to `b59e5826da2acff8` for the
sequence/import/static-builtin scaffold. The only new family is
`770bd29db214f114`, a tiny production match-shape overlap between
`method_receiver_domain_requirement` and `method_receiver_contract_key` after
adding `RubyCoreNilPredicate`. Those functions intentionally project the same
closed enum into different domains: semantic receiver requirements versus
stable LibraryApi contract keys. Sharing them would couple two separate policy
tables and make exhaustiveness less explicit. The budget is raised to 31 for
this reviewed enum-projection span, not for avoidable duplication.

The accepted-pair endpoint coverage fix (#817) keeps the reviewed default-surface
count at 31. Compact accepted-edge tracing added ahead of
`candidates.rs::anchor_candidates`, moving that function's source span and the
already reviewed cross-engine candidate-pair enumeration representative from
`ec66a9b9f2569018` to `0f873b1c184596cb`. Its members remain
`nose-markdown`'s `fingerprint.rs::candidate_pairs` and `nose-detect`'s
`candidates.rs::anchor_candidates`; this is representative-ID churn for the same
visible cross-engine algorithm debt, not a new duplicate family. No new budget is
accepted.

The HOF callback-purity slice (#794) tightens the reviewed default-surface count
from 31 to 30. The first self-query reported 32 families, including two
avoidable helper families introduced or enlarged by this work. Three modules
were repeating the span projection already owned by `EvidenceAnchor::span`, and
four crates maintained equivalent byte-range containment functions. Removing
those wrappers and adding the domain-neutral `Span::contains` method eliminates
the new `3929d0b5bb2cdef6` / `d25a506b4942c20e` findings and the stale accepted
evidence-anchor representative `734ee4c50e4d001e`.

The final 30-family report contains three reviewed representative moves:
`2a5aa3db45d33592` becomes `1faed9f36902890e` for the existing
builder/value-DAG whole-impl span after the containment cleanup;
`ac31c3c9bc390d55` becomes `af4b6d3f13df0fcb` for the per-frontend
call/constructor/enum-constant lowering family; and `ebf5e40476ceff32`
becomes `6bd7fd9ff22dcd0f` for the per-language expression-dispatch family,
now also represented by Ruby after its source-boundary hardening. These remain
the previously reviewed language-frontend parallelism and value-graph span-noise
classes. The old string-affix fixture family `f4bd533cf627ba92` no longer crosses
the substantial threshold. The sole new accepted representative,
`a679fd9cdbda1be2`, is a broad whole-impl match between the 489-line general
operator-semantics implementation and the 17-line callback literal-domain
matcher: only four literal projection lines are shared/removable, while the
callback matcher deliberately preserves the narrower Number/BigInt and
effect-closure policy. Sharing that policy would weaken the boundary rather
than clarify it. With the real helper duplication removed and one stale family
gone, the budget is tightened to 30.

The Swift `compactMap` option-emission slice (#795) tightens the reviewed
default-surface count from 30 to 29. The first self-query showed two apparent
new test-family IDs because the new strict-exact regression was inserted ahead
of existing receiver tests. Moving that regression after the existing tests
preserves the accepted HOF-demand and receiver-domain representatives. A final
boundary-hardening run then exposed real repeated setup across four new
cross-file `compactMap` corpus tests and pushed the pre-existing
`lower_expr`/`lower_pattern_value` Swift lowering overlap over the substantial
threshold. A shared corpus assertion helper removes the repeated setup; routing
expression-shaped patterns through `lower_expr` with guard-clause control flow
removes the overlap without adding a high-parameter abstraction. The only final
baseline delta is that the pre-existing
`interp/ops.rs::int_bin` / `float_bin` dispatcher family
`856ea94f585f0c67` no longer reports after the FilterMap semantic changes; a
member-set comparison against `origin/main` confirms that all other 29 families
are unchanged. No new duplication is accepted, so the stale family is removed
and the budget is tightened to 29.

The Swift one-level `flatMap` slice (#796) keeps the reviewed default-surface
count at 29. The final release-binary gate matches every accepted family ID in
`scripts/duplication-baseline.json`; the dedicated FlatMap callback obligation,
lexical parameter resolution, non-plain callback markers, raw-selector exact
guard, Swift dispatch barriers, cross-file tombstones, focused fixtures, and
generated proof artifacts introduce no substantial default-surface family. No
new duplication or budget change is accepted.

The guarded `flatMap` aggregate slice (#797) moves the reviewed default-surface
count from 29 to 30. Its first self-query exposed `2284c1cbcb9277f1`, repeated
temporary-corpus setup across the new cross-file Swift `filter` and
`allSatisfy` boundary tests. Routing those cases, along with the existing
`flatMap` and `compactMap` cases, through a narrow method-contract assertion
helper removes that avoidable test family before the baseline update.

The sole remaining delta is `856ea94f585f0c67`, the pre-existing
`interp/ops.rs::int_bin` / `float_bin` dispatcher family already reviewed during
the Python loop/De Morgan admission. Neither member changes in #797. The new
aggregate semantic admission shifts nose's self-query fingerprints enough for
the family to cross the value >= 40 boundary again after it had fallen below
that boundary in #795. Integer wrapping and divide/modulo checks still differ
materially from float IEEE behavior, so extracting their ten shared dispatch
lines would obscure rather than simplify the numeric boundary. The reviewed ID
is restored and the budget is raised to 30; no new avoidable duplication is
accepted.

Final reducer-cardinality hardening also moves the previously reviewed
value-graph collection whole-impl span-noise representative from
`2fb6d5f9c8c6d045` to `cf7e3e2870c92ccb`. The replacement pairs the 384-line
`cardinality.rs` `impl Builder` span with a ten-line slice in `reductions.rs`,
sharing only two lines. That is not a useful shared abstraction; it is
representative churn caused by the new fail-closed reducer guards. The family
count remains 30, so no additional budget or avoidable duplication is accepted.

The bounded #829 stabilization pass tightens the reviewed default-surface count
from 30 to 29. The removed test family `85074f64d038d1a0` repeated the complete
Rust `Map.get(...).unwrap_or(...)` evidence setup across one positive and two
adjacent hard negatives. A test-only fixture now owns the shared IL and evidence
construction while each test retains its observable admission assertion. The
fixture exposes only the two behavior-defining variations: whether map-domain
proof belongs to the call receiver, and the nested `MapGet` source arity. No
production policy is shared, no family is added, and the three focused tests
still exercise receiver dependency, arity, and builtin-kind boundaries. The
stale ID is removed and the budget is tightened to 29.

The #821 connected-witness closeout tightens the count from 29 to 28. Its first
self-query surfaced repeated async-await oracle-exclusion setup across the
obligation and classification tests; `await_oracle_exclusion_report` now owns
that shared project/report fixture, so the new connected family is removed.
Three remaining deltas are representative churn for reviewed families:
`0f873b1c184596cb` -> `17e10a85fc7439bc` for Markdown/detect candidate-pair
enumeration, `6bd7fd9ff22dcd0f` -> `1c0371a3681dc8ca` for cross-frontend
expression dispatch, and `af4b6d3f13df0fcb` -> `f6d422cfc56ae976` for
call/constructor/enum-constant lowering. The old numeric dispatcher
representative `856ea94f585f0c67` no longer crosses the threshold. No new
avoidable duplication is accepted, and the budget tightens to 28.

The #832 bounded same-unit route keeps the reviewed count at 28. Adding the
separately priced same-unit candidate arm shifts `anchor_candidates` in
`candidates.rs`, so the existing Markdown/detect candidate-pair enumeration
representative moves from `17e10a85fc7439bc` to `8462d08908be9e8a`. The members
remain exactly `fingerprint.rs::candidate_pairs` and
`candidates.rs::anchor_candidates`; the replacement shares ten lines across
roughly 50-line spans and still carries five presentation parameters. No new
family, budget, or useful shared abstraction is accepted.

The #842 generated-provenance slice moves the reviewed count from 28 to 29. Its
only final delta is `856ea94f585f0c67`, the pre-existing
`interp/ops.rs::int_bin` / `float_bin` dispatcher family; neither member changes
in #842. The source-aware CLI, fold, and evidence additions shift nose's
self-query fingerprints enough for this reviewed family to cross the value >=
40 boundary again. Integer wrapping, division/modulo, floor, and bitwise
semantics still differ materially from float IEEE behavior, so sharing the ten
common dispatch lines would blur rather than clarify the numeric policy
boundary. Reusing the existing `fam_at` helper keeps the new surface-selection
test scaffolding small without introducing another substantial family. The
accepted ID is restored and the budget is raised to 29; no new avoidable
duplication is accepted.

The #843 declaration-only type-contract slice tightens the reviewed count from
29 to 28. The only gate delta is again `856ea94f585f0c67`, the existing
`interp/ops.rs::int_bin` / `float_bin` dispatcher family; neither numeric
member changes in #843. The language-neutral surface classifier, focused
frontend boundary corrections, and their tests shift nose's self-query
fingerprints enough for this representative to fall below value 40. All other
28 accepted IDs remain exact, and no new substantial default-surface family is
introduced. The stale ID is therefore removed and the budget tightens without
accepting new duplication.

The #857 exclusion-attribution slice moves the reviewed default-surface count
from 28 to 29. Structured first-blocker propagation changes the oracle
interpreter evaluator and statement-walker spans, so the two existing
whole-implementation span-noise representatives move without changing their
members: `149bb759833d2d51` becomes `20d66ad9ef0c1f42` for
`interp/eval.rs` and `value_graph/eval/core.rs`, while `d9eaf862103c02a7`
becomes `30ae71e90215f0cc` for `interp/exec.rs` and
`value_graph/control/statements.rs`. These remain intentionally independent
execution models; sharing their 9- and 16-line dispatch skeletons would couple
oracle interpretation to exact-fingerprint construction.

The only count increase is `856ea94f585f0c67`, the pre-existing
`interp/ops.rs::int_bin` / `float_bin` dispatcher family already reviewed in
the Python loop/De Morgan and generated-provenance slices. Capability-specific
unsupported outcomes shift the self-query fingerprints enough for it to cross
value 40 again. Integer wrapping, division/modulo, floor, and bitwise semantics
remain deliberately separate from float IEEE behavior, so extracting the ten
shared dispatch lines would blur the soundness boundary. No new avoidable
duplication is accepted; the two representative IDs are replaced and the
budget is raised to 29 for the returning reviewed family.

The #858 final JavaScript Number review tightens the reviewed count from 29 to
27. The long-standing field-state test fixture family `90809d0e27461ac4` is
removed by giving the interpreter tests shared helpers for `this.field` IL
construction and Java fixture finalization; each test still owns its distinct
read, write, error, ordering, and evidence assertions. The pre-existing numeric
`int_bin` / `float_bin` dispatcher representative `856ea94f585f0c67` falls
below value 40 after the Number-result hardening shifts the self-query, so its
stale ID is removed without accepting a replacement.

The remaining two deltas are representative churn for the same independent
oracle/value-graph implementation pairs reviewed in #857:
`20d66ad9ef0c1f42` becomes `1a27aef1c8aa5d25` for `interp/eval.rs` and
`value_graph/eval/core.rs`, while `30ae71e90215f0cc` becomes
`0028e1c168824352` for `interp/exec.rs` and
`value_graph/control/statements.rs`. Their members and rationale are unchanged;
no new family is accepted, and the budget is tightened to 27.
