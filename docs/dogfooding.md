# Dogfooding nose on nose — a critical review

Goal: honestly assess whether `nose scan crates` produces *real* design-level
refactoring opportunities on its own codebase, act on the genuine ones, and record
where the tool is weak. The third-party counterpart is [field evaluation](field-evaluation.md);
the duplication gate that grew out of this lives in [CONTRIBUTING](../CONTRIBUTING.md).

Original review command: `nose scan crates --exclude tests` (6 Rust crates, 8-language
frontends). Result at the start of this review: 34 candidate families, ~662 duplicated
lines (the arc below took it to 23 / ~411). The numbers are a dated snapshot of *this*
review pass; re-running on today's larger codebase reports more, since the crates have
since grown.

The current CI gate is [scripts/check-duplication.sh](../scripts/check-duplication.sh):
it runs `nose scan crates --exclude tests --mode near --min-value 40` and compares the
substantial near-duplicate count with the accepted budget recorded in that script.

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
`nose features --format json` (`anchors[].line_start` / `line_end`).

Those line ranges are now surfaced at the **family** level too: when every member of a clone family
shares a heavy sub-DAG, each site in the report carries `locations[].shared_subdag = [start, end]`
— its OWN source range for the shared computation — and `nose scan`'s text output appends
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
near-family — `interp.rs` ↔ `value_graph/eval.rs` ↔ `value_graph/control.rs`, sharing ~12 of ~1082
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
