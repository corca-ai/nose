#!/usr/bin/env bash
# Duplication gate — nose dogfooding itself.
#
# Fails when the set of *substantial* duplicate families on nose's own source differs
# from the accepted baseline. The mode, minimum refactoring value, output surface, and
# accepted family IDs live in scripts/duplication-baseline.json; the decision trail lives
# in docs/dogfooding.md. To accept a genuinely new one, either dedupe it or update both
# files with a one-line justification in the PR.
#
# Runs only the `near` channel: this gate is about *design-level* Type-3 duplication
# (families worth extracting), not the syntax copy-paste floor — which always surfaces
# the accepted per-grammar frontend parallelism (see docs/dogfooding.md).
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
# are dedup candidates (see docs/dogfooding.md); the gate stays a ratchet against NEW duplication
# on top of this stronger detector.
# Scope expansion in the quality-gates pass: the gate now scans tests as well as production
# code. Current binary, current tree: production-only default surface reports 24 substantial
# families, while the tests-included default surface reports 39. The newly visible
# tests/mixed families and post-release refresh deltas are reviewed in docs/dogfooding.md;
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
# the value ≥ 40 line — not new avoidable duplication. See docs/dogfooding.md.
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
# known semantic-kernel plumbing made more explicit by the migration. Reviewed in docs/dogfooding.md;
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
# span with only one shared/removable line. Reviewed in docs/dogfooding.md; accepted as detector
# span noise, not avoidable duplication.
# 54 -> 54 (#587 Rust module resolution 1-3): context-aware imported literal export collection
# moves the accepted context/export assignment-counting representative again. Reviewed in
# docs/dogfooding.md; no new budget is accepted.
# 54 -> 54 (#587 Rust direct re-exports): one-hop public-use alias evidence moves the accepted
# context/export representative, the value-graph whole-impl span, and the semantic-kernel
# provenance-helper representative. Reviewed in docs/dogfooding.md; no new budget is accepted.
BIN="${NOSE_BIN:-./target/release/nose}"
BASELINE="${NOSE_DUP_BASELINE:-scripts/duplication-baseline.json}"

if [ ! -x "$BIN" ]; then
    echo "error: nose binary not found at '$BIN' (build with: cargo build --release)" >&2
    exit 2
fi

python3 scripts/check-duplication-baseline.py --bin "$BIN" --baseline "$BASELINE"
