#!/usr/bin/env bash
# Duplication gate — nose dogfooding itself.
#
# Fails when the set of *substantial* duplicate families on nose's own source differs
# from the accepted baseline. The mode, minimum refactoring value, output surface, and
# accepted family IDs live in scripts/duplication-baseline.json; the decision trail lives
# in docs/dogfooding-history.md. To accept a genuinely new one, either dedupe it or update
# the baseline and history with a one-line justification in the PR.
#
# Runs only the `near` channel: this gate is about *design-level* Type-3 duplication
# (families worth extracting), not the syntax copy-paste floor — which always surfaces
# the accepted per-grammar frontend parallelism (see docs/dogfooding-history.md).
#
# DETERMINISM: the count is reproducible run-to-run AND across platforms — nose hashes with
# FxHash (no random seed) and ranks with IEEE correctly-rounded ops only (+ - * / sqrt), and the
# family dedup sorts by a TOTAL order (span, value, then min source location). So CI and a local
# run report the SAME number; a count change is a real detection change (new duplication or a
# grammar/parse difference), never platform jitter. If they ever disagree, suspect a stale binary
# or a tree-sitter grammar version skew — not nondeterminism.
set -euo pipefail

# Re-baselined 6 → 20 in PR #82: that PR STRENGTHENS the `near` channel (value-fingerprint
# candidates + high-vj acceptance for impure code, and sub-DAG anchor pairing), so nose now
# detects 14 additional PRE-EXISTING near-duplicate families in its own source — the cross-grammar
# frontend helpers and the `proven_*` value-graph factories — not new code introduced here. They
# are dedup candidates (see docs/dogfooding-history.md); the gate stays a ratchet against NEW duplication
# on top of this stronger detector.
# Scope expansion in the quality-gates pass: the gate now scans tests as well as production
# code. Current binary, current tree: production-only default surface reports 24 substantial
# families, while the tests-included default surface reports 39. The newly visible
# tests/mixed families and post-release refresh deltas are reviewed in docs/dogfooding-history.md;
# this is a scope expansion and reviewed baseline refresh, not a loosening of the old
# production-only gate.
#
# 20 → 21: weight-grading the sub-DAG score (a larger shared computation now scores higher, up to
# 0.90) lifts one PRE-EXISTING partial-clone family in nose's own source past the substantial
# (value ≥ 40) line — finer ranking surfacing real debt, not new code. Still a dedup candidate.
# 21 → 22: receiver-method LibraryApi occurrence evidence makes the near channel admit one
# PRE-EXISTING param-domain/binding helper family; new occurrence-producer duplication was deduped.
# 22 → 23: adding the Java empty-collection constructor recognizer to the `strict_exact_safe_call`
# dispatch chain (one `if recognizer { return true }` line) lifts the PRE-EXISTING
# `strict_exact_safe_call` ↔ `strict_exact_in_membership_safe` similarity (a ~4-line incidental
# overlap between a recognizer dispatch and a membership checker, not extractable duplication) past
# the value ≥ 40 line — not new avoidable duplication. See docs/dogfooding-history.md.
# Re-baselined 23 -> 24 in the #210 campaign: stronger fingerprint fidelity (deref
# stores, loop-effect keying) made one PRE-EXISTING cross-crate near-family visible —
# the assignment-name counting loops in value_graph/context.rs::seed_module_value_bindings
# and module_imports.rs::collect_statement_exports (small, cross-crate; accepted, kept).
# Re-baselined 24 -> 25 in the #283-A fix: the effect-free-reorder guard shifts a few
# self-source value-graph fingerprints, nudging one PRE-EXISTING large-span dispatch
# near-family (interp.rs / value_graph/eval.rs / control.rs, sharing ~12 of ~1082 lines)
# past the value >= 40 line — a spurious whole-function span, not new duplication.
# Re-baselined 25 -> 26 in the #315 graded-witness PR: the new value_dag.rs's
# `impl FileReferents` whole-span (~270 lines) incidentally shares ~7 boilerplate lines
# (the `impl<'a>` header + a `for u in &il.units { def_*.entry(..) }` skeleton) with
# value_graph/builders.rs's `impl Builder` — 8 varying spots, nothing extractable (the
# two impls do unrelated work). A spurious whole-impl span, not new duplication.
#
# +1 (series 9): the two table-driven decidability-filter tests in nose-cli's inline
# `tests` module — `declaration_spans_fail_open_per_language` and
# `declaration_spans_classify_per_language` — are genuinely near-identical by construction
# (a `&[(&str,&str)]` case table + an `assert!(…ast_classifies…)` loop, differing only in
# the asserted direction). The series-9 dataflow inline-soundness fix shifted enough
# structure to push the pair over the near threshold. Benign test scaffolding, nothing to
# extract; the production change it rode in on is fingerprint-neutral (family delta ≈ 0).
#
# +1 (semantic false-merge boundaries): the value-graph order-orientation soundness fix shifts
# canonicalized value fingerprints enough for this branch's binary to report the same 28-family
# count even when querying an unmodified origin/main tree. The extra counted family is the
# pre-existing high-parameter overlap slice
# `body_depends_on_iter` / `foreach_effect_body_depends_on_iter` / `single_branch_statement`,
# folded under the loop-effect family in human output. It is tracked design debt, not code
# introduced here.
#
# 36 -> 55 (builtin semantic-pack migration): moving language/library/protocol evidence into
# pack-owned provenance and file-length-compliant modules makes many existing semantic-evidence
# test helpers/resolver negatives visible as separate near families. A few production families are
# known semantic-kernel plumbing made more explicit by the migration. Reviewed in docs/dogfooding-history.md;
# accepted as migration debt rather than deduped inside the architecture move.
# 56 -> 54 (#536 JS/TS Array HOF): adding the callback-obligation review fix unifies the inline
# callback shape in method-call and typed/free-call IL fixtures, so two accepted test-fixture
# families disappear. The budget is tightened with the resolved families removed.
# 54 -> 53 (#537 Swift Sequence HOF): shared callback fixture nodes and receiver-domain setup
# remove the draft Swift-HOF test-scope duplication plus one accepted representative from the
# baseline. The budget is tightened again.
# 53 -> 52 (#557 string affix protocol pack): the new string-affix admission resolver initially
# joined the existing receiver-method LibraryApi fixture family. Extracting the shared
# receiver-method call fixture removes that accepted representative with no new family.
# 53 -> 54 (#582 receiver-domain recovery): module/static binding seeding moves the accepted
# context/export assignment-counting representative and surfaces one small value-graph whole-impl
# span with only one shared/removable line. Reviewed in docs/dogfooding-history.md; accepted as detector
# span noise, not avoidable duplication.
# 54 -> 54 (#587 Rust module resolution 1-3): context-aware imported literal export collection
# moves the accepted context/export assignment-counting representative again. Reviewed in
# docs/dogfooding-history.md; no new budget is accepted.
# 54 -> 54 (#587 Rust direct re-exports): one-hop public-use alias evidence moves the accepted
# context/export representative, the value-graph whole-impl span, and the semantic-kernel
# provenance-helper representative. Reviewed in docs/dogfooding-history.md; no new budget is accepted.
# 54 -> 53 (Promise async return recovery): same-file async DirectFunction recovery first surfaced
# an avoidable production family between call-target evidence upsert and LibraryApi evidence
# recording. Splitting the call-target upsert matcher removed it; the remaining direct-call-target
# test family is representative-ID churn from inserted fixtures. The budget is tightened.
# 53 -> 53 (Promise direct-function return recovery): the first draft surfaced avoidable
# duplication across the new Promise call-target evidence tests; extracting a shared direct-return
# fixture removed it. The remaining delta is representative-ID churn for the same
# direct-call-target negative fixture family, now `32ed015840375d04` instead of
# `727e41b9e3e96f1e`; no new budget is accepted.
# 53 -> 53 (#602 Promise.all literal aggregate): the first draft surfaced avoidable production
# duplication in a new qualified-global symbol evidence lookup; rewriting it as an explicit proof
# check removed that family. The remaining delta is representative-ID churn for the reviewed
# semantic-kernel language-core provenance helper family, now `551e7992e1632597` instead of
# `46eafe785a6f3517`; no new budget is accepted.
# 52 -> 52 (async protocol near-channel mirror): extending value-graph async protocol dual-view
# handling moves a reviewed evaluator whole-impl span. `c9fe4dc9d9cd14f5` disappears and
# `149bb759833d2d51` appears for the oracle evaluator / value-graph evaluator overlap; no new
# budget is accepted.
# 52 -> 52 (Java Future/Executor local/this-field receivers): exact Java receiver-domain evidence
# moves two reviewed frontend-lowering representatives (`7b134f23e922f405` -> `596f602568ace201`,
# `6e37683225332c86` -> `a54e8f6b173a160a`) without changing members or budget.
# 52 -> 52 (Java CompletableFuture constructor/package-shadow reporting): extracting the shared
# Java construct-call lowering helper removes the avoidable constructor-helper near family. The
# remaining delta is representative churn for the same reviewed frontend-lowering families:
# `596f602568ace201` -> `ac31c3c9bc390d55`, and `a54e8f6b173a160a` -> `ebf5e40476ceff32`.
# No new budget is accepted.
# 52 -> 52 (0.17.0 release prep): lazy import-use indexes and release-performance fixes move the
# reviewed context/export assignment-counting representative (`2a3ff0019f8a1765` ->
# `1d565f1e57ac5d8b`) and the reviewed semantic-kernel language-core provenance representative
# (`551e7992e1632597` -> `7c8432da3fcb2c67`). Reviewed in docs/dogfooding-history.md; no new budget is
# accepted.
# 52 -> 53 (post-0.17.0 runtime pass): profile-guided candidate/indexing changes move the reviewed
# context/export representative (`1d565f1e57ac5d8b` -> `c967b3bcff5a2b58`) and the reviewed
# query-origin hint representative (`77d8e8012b2ac08a` -> `a7f4d8398c1920e6`). The same pass also
# surfaces a new production cross-crate candidate-pair enumeration family (`cc48beefc6a85976`)
# between Markdown fingerprint pair generation and semantic anchor pair generation. Reviewed in
# docs/dogfooding-history.md; accepted as visible cross-engine algorithm debt, not a reason to introduce a
# lower-layer utility dependency.
# 53 -> 52 (query-opportunities dogfood): extracting the shared origin-fact summary for
# `origin_extract_hint` and `hint_reasons` removes the reviewed query-origin hint family
# `a7f4d8398c1920e6`, so the ratchet tightens.
# 52 -> 51 (switch-label dogfood): moving the identical switch-label OR-chain fold into
# `lower::fold_switch_labels` removes the reviewed C-family frontend lowering family
# `f57a5ee0ebbdf114` without changing product query output.
# 51 -> 50 (fragment block-shape dogfood): sharing `empty_or_single_block_child` removes
# the reviewed direct-effect/self-field branch family `4ac4a88371e43e72`. The old
# fragment span-noise representative `bf4255f2994b1d65` moves to `9a228db20ad1a68b`.
# 50 -> 49 (strict-exact HOF dogfood): factoring the tree/terminal-reduction/len HOF
# admission variants into `StrictExactHofUse` removes `f010e9908081b902` with no
# product query output drift.
# 44 -> 42 (cross-crate evidence fixtures): shared `EvidenceRecord` constructors and
# feature-gated `nose-semantics::test_support` helpers remove the repeated
# compatibility-pack evidence fixture builders across normalize, semantics, and detect tests.
# 42 -> 39 (LibraryApi fixture builders): shared test-only LibraryApi record
# pack/provenance builders remove the repeated contract id/callee/arity fixture wrappers.
# 39 -> 37 (exact-fragment ordered branch fixtures): shared CLI exact-fragment query and
# branch-pair assertion helpers remove the repeated ordered-branch fixture harness.
# 37 -> 37 (exact-fragment branch shape): shared `if_branch_blocks` removes the broad
# fragment mirror representative `9a228db20ad1a68b`; the narrower loop-window mirror
# representative `5ad08a3c9ab9f5c3` remains reviewed differential-test debt.
# 37 -> 36 (value-graph binary node inspection): generalized the existing `bin_args`
# helper into `bin_op_args`, removing the selection/reduction whole-impl representative
# `1dfaba2582163d7c` without sharing min/max or reduction policy.
# 36 -> 34 (design/docs cleanup): `language_core_evidence_provenance_hashes` now derives
# hashes from `language_core_evidence_provenance`, and coverage plus gap-impact diagnostics
# share Raw surface classification through `nose_frontend::raw_node_surface`. These remove the
# reviewed production representatives `9510e3368e161f45` and `936f238ab2e0d6b2`; no new family
# appears, so the ratchet tightens.
# 34 -> 33 (verify soundness classification): shared fingerprint-group classification removes
# the reviewed verify/recall-loss soundness overlap `1f922efb624c7f79`; no new family appears,
# so the ratchet tightens.
# 33 -> 31 (LibraryApi test evidence builders): common LibraryApi test evidence and method-call
# provenance helpers remove the reviewed `library_api_contract_evidence` and
# `method_call_library_api_evidence` fixture families. The tiny `sp` helper representative moves
# from `eadc678efab56738` to `e46ab190592b0850`.
# 31 -> 29 (Guava map factory fixtures): shared Guava `ImmutableMap.of` test IL/evidence and
# unsupported-arity payload builders remove `6b3b0c88a12efe80` and `f2646ca6f31a0c0b`. The tiny
# `sp` helper representative moves from `e46ab190592b0850` to `c2c6dbcb3016eb40`.
# 29 -> 29 (callee dependency matcher policy helpers): named-callee and static-receiver proof
# helpers reduce the reviewed callee-dependency matcher family without making it a clearer shared
# abstraction. The representative moves from `dbbb03b3c0fa93e8` to `95e83331abfa623f`.
# 29 -> 26 (admission resolver dogfood): shared admission-provenance helpers remove the reviewed
# `f88aeebdec4f2c68`, `e6d039006310127f`, and `7c1aef5590dfeefc` test fixture families. The
# remaining admission-resolver representative moves from `e3fa2e4c707e342a` to `eb2f9fe7da72f8dd`.
# 26 -> 26 (proof-carrying frontier gate): adding the frontier admission script moves the
# reviewed Markdown/code candidate-pair enumeration representative from `cc48beefc6a85976`
# to `ec66a9b9f2569018`. The members are still `fingerprint.rs::candidate_pairs` and
# `candidates.rs::anchor_candidates`; no new budget is accepted.
# 31 -> 31 (#817 accepted-pair endpoint coverage): accepted-edge tracing moves the reviewed
# candidate-pair enumeration representative from `ec66a9b9f2569018` to `0f873b1c184596cb`.
# The same two functions remain the only members; no new budget is accepted.
# 26 -> 27 (numeric clamp proof facts): an avoidable test-helper family in the new clamp
# proof fixtures was deduped. The remaining new family is a broad whole-impl span between
# the interpreter statement executor and the value-graph statement walker, sharing 16 lines
# across ~270-line NodeKind dispatchers. Reviewed in docs/dogfooding-history.md; accepted
# as detector span noise, not a useful shared abstraction.
# 27 -> 30 (Python loop/De Morgan admission): logical De Morgan plus literal-absence
# canonicalization surfaces three PRE-EXISTING production families in nose's own code:
# numeric int/float binary dispatch, Java map factory vocabulary projections, and language
# profile/type-domain vocabulary projections. Reviewed in docs/dogfooding-history.md;
# accepted as detector-improvement surfacing, not new avoidable duplication.
# 30 -> 30 (dense-literal TypeScript every): the draft surfaced avoidable JS-like language
# predicate and normalize idiom fixture-helper families; those were deduped by sharing
# `nose_semantics::js_like_lang` and the normalize test module builders. The remaining
# delta is representative churn from `44bfd76822ddbe95` to `2fb6d5f9c8c6d045` for the
# same value-graph collection whole-impl span-noise class. Reviewed in docs/dogfooding-history.md;
# no new budget is accepted.
# 30 -> 31 (option absence-channel identity): avoidable null-presence semantic-query
# fixture scaffolding was deduped with shared temp-project helpers. The remaining delta is
# representative churn for reviewed semantic-query harness families plus a tiny production
# enum-projection overlap between MethodReceiverContract's domain-requirement table and
# stable contract-key table after adding RubyCoreNilPredicate. Reviewed in
# docs/dogfooding-history.md; accepted as explicit policy-table span noise.
# 31 -> 30 (#794 HOF callback purity): the first self-query exposed duplicated
# EvidenceAnchor span projection and byte-range containment helpers; routing them through
# `EvidenceAnchor::span` and `Span::contains` removes both families. Remaining frontend and
# value-DAG IDs are reviewed representative churn, while one four-line callback-literal overlap
# is whole-impl span noise. A stale string-affix fixture family also disappears, so the ratchet
# tightens. See docs/dogfooding-history.md.
# 30 -> 29 (#795 Swift compactMap): shared corpus-test setup and guard-clause pattern lowering
# remove the two families exposed during final boundary hardening. The pre-existing int/float
# interpreter dispatcher family no longer reports after the FilterMap semantic changes, so the
# ratchet tightens again without accepting new duplication.
# 29 -> 29 (#796 Swift one-level flatMap): the dedicated callback obligation, lexical parameter
# proof, non-plain callback markers, raw-selector guard, dispatch barriers, corpus tombstones, and
# focused fixtures preserve every accepted family ID. No new duplication or budget change is
# accepted.
# 29 -> 30 (#797 flatMap aggregate guards): the draft's repeated cross-file Swift corpus setup was
# extracted into the shared method-contract assertion helper. The aggregate semantic admission
# restores the previously reviewed `int_bin` / `float_bin` dispatcher family
# `856ea94f585f0c67`. Final reducer-cardinality hardening also moves the reviewed value-graph
# collection whole-impl span-noise representative from `2fb6d5f9c8c6d045` to
# `cf7e3e2870c92ccb`; the replacement shares only two lines across a 384-line impl and a ten-line
# reducer slice, so there is no useful helper to extract. No new budget or avoidable duplication is
# accepted; see docs/dogfooding-history.md.
# 28 -> 28 (#832 bounded same-unit windows): the separate same-unit candidate arm shifts the
# reviewed Markdown/detect candidate-pair enumeration representative from `17e10a85fc7439bc`
# to `8462d08908be9e8a`. The same two functions remain the only members; no new budget or
# avoidable duplication is accepted.
# 28 -> 29 (#842 generated provenance): source-aware CLI, fold, and evidence additions make the
# pre-existing reviewed numeric-dispatch family `856ea94f585f0c67` cross the threshold again;
# neither member changed. The new surface-selection test reuses existing scaffolding and adds no
# further substantial family. See docs/dogfooding-history.md.
# 29 -> 28 (#843 declaration-only type contracts): the reviewed numeric int/float dispatcher
# family falls below the substantial threshold again after the new typed-surface and frontend
# boundary tests shift self-query fingerprints; neither numeric member changed. No new default
# family is accepted, so the stale ID is removed and the budget tightens. See
# docs/dogfooding-history.md.
# 28 -> 29 (#857 exclusion attribution): structured oracle diagnostics move the two reviewed
# interpreter/value-graph whole-impl span-noise representatives without changing their members.
# The same changes make the pre-existing int/float dispatcher family cross value 40 again. No new
# extractable family is accepted; see docs/dogfooding-history.md.
BIN="${NOSE_BIN:-./target/release/nose}"
BASELINE="${NOSE_DUP_BASELINE:-scripts/duplication-baseline.json}"

if [ ! -x "$BIN" ]; then
    echo "error: nose binary not found at '$BIN' (build with: cargo build --release)" >&2
    exit 2
fi

python3 scripts/check-duplication-baseline.py --bin "$BIN" --baseline "$BASELINE"
