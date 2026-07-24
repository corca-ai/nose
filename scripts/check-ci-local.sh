#!/usr/bin/env bash
# Local CI preflight.
#
# Modes:
#   --fast        PR/push preflight: catches the common CI failures quickly.
#   --full        Full local mirror of the GitHub Actions gates.
#   --gate <name> Run one named gate. GitHub Actions uses this internal surface
#                 so local and remote checks have one command owner.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="fast"
gate_name=""
gate_args=()
case "${1:-}" in
    "" | --fast)
        mode="fast"
        ;;
    --full)
        mode="full"
        ;;
    --gate)
        if [[ -z "${2:-}" ]]; then
            echo "missing gate name" >&2
            exit 2
        fi
        mode="gate"
        gate_name="$2"
        gate_args=("${@:3}")
        ;;
    -h | --help)
        cat <<'EOF'
usage: ./scripts/check-ci-local.sh [--fast|--full]
       ./scripts/check-ci-local.sh --gate <name> [gate arguments...]

  --fast  corpus and semantic-pack self-tests, Type-4 packet/replay checks,
          rustfmt, file-length ratchet, legacy-prelude guard, shellcheck,
          clippy -D warnings, nose-cli tests, docs wiki lint
  --full  full local mirror of CI: format, clippy, docs, release build/tests,
          file-length ratchet, duplication, MSRV, supply-chain, docs wiki,
          formal obligation lint, and Lean proofs
  --gate  internal named-gate surface shared with GitHub Actions
EOF
        exit 0
        ;;
    *)
        echo "unknown mode: $1" >&2
        echo "usage: ./scripts/check-ci-local.sh [--fast|--full]" >&2
        exit 2
        ;;
esac

step() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        if [[ -n "${2:-}" ]]; then
            echo "$2" >&2
        fi
        exit 127
    fi
}

run_docs_wiki_lint() {
    need_cmd awiki "install it with: brew install corca-ai/tap/awiki"
    need_cmd python3
    ./scripts/check-docs.sh
}

run_formal_obligations_lint() {
    need_cmd python3
    python3 scripts/check-formal-obligations.py --self-test
    python3 scripts/check-formal-obligations.py
}

run_formal_lean() {
    ./scripts/check-lean-proofs.sh
}

run_file_length_ratchet() {
    need_cmd python3
    python3 scripts/check-file-lengths.py --self-test

    need_cmd git
    local ratchet_base="${1:-origin/main}"
    if ! git rev-parse --verify "$ratchet_base" >/dev/null 2>&1; then
        echo "missing file-length ratchet base: $ratchet_base" >&2
        exit 127
    fi
    python3 scripts/check-file-lengths.py --ratchet-base "$ratchet_base"
}

run_legacy_prelude_guard() {
    need_cmd python3
    python3 scripts/check-legacy-prelude.py --self-test
    python3 scripts/check-legacy-prelude.py
}

run_semantic_pack_pricing_selftest() {
    need_cmd python3
    python3 bench/semantic_pack/pricing.py --selftest
    python3 bench/semantic_pack/pricing.py --check-artifacts
}

run_type4_frontier_evidence_checks() {
    need_cmd python3
    python3 bench/type4/generator_selftest.py
    python3 bench/type4/frontier_platform.py --selftest
    python3 bench/type4/frontier_platform.py --check
    python3 bench/type4/python_loop_demorgan_proof_facts.py --selftest
    python3 bench/type4/python_loop_demorgan_proof_facts.py --check
    python3 bench/type4/proof_carrying_frontier.py --selftest
    python3 bench/type4/proof_carrying_frontier.py --check
}

run_type4_executable_expectations() {
    need_cmd python3
    NOSE_BIN="${1}" bench/type4/adversarial/scripts/type4-exec-check \
        --stable-report \
        --json-out bench/type4/executable_expectations.v1.json
    NOSE_BIN="${1}" python3 bench/type4/real_frontier_replay.py \
        --stable-report \
        --check
    python3 bench/type4/python_loop_demorgan_proof_facts.py --check
    python3 bench/type4/proof_carrying_frontier.py --check
}

run_type4_axis_language_claims() {
    need_cmd python3
    local nose_bin="$1"
    local ratchet_base="${2:-origin/main}"
    python3 bench/type4/coverage_probe.py \
        --nose "$nose_bin" \
        --blind-report bench/type4/blind_attack.v1.json
    python3 bench/type4/coverage_sweep.py --nose "$nose_bin" --quiet
    python3 bench/type4/coverage_matrix.py matrix
    python3 bench/type4/check_axis_language_claims.py --self-test
    python3 bench/type4/check_axis_language_claims.py \
        --nose "$nose_bin" \
        --ratchet-base "$ratchet_base"
    git diff --exit-code -- \
        bench/type4/coverage_evidence.v1.json \
        bench/type4/coverage_matrix.v1.json \
        bench/type4/blind_attack.v1.json
}

run_regression_checker_selftests() {
    need_cmd python3
    need_cmd node
    python3 scripts/check-domain-calibration.py
    python3 scripts/check-domain-calibration.py --self-test
    python3 bench/labels/query_schema.py --self-test
    python3 bench/labels/default_head_query_schema.py --self-test
    python3 bench/labels/live_query_schema.py --self-test
    python3 bench/labels/eval_by_language.py --self-test
    python3 bench/labels/check_default_head_baseline.py --self-test
    python3 bench/labels/check_default_head_baseline.py
    python3 bench/labels/labelset.py --self-test
    python3 bench/labels/label_refresh.py --self-test
    python3 bench/labels/default_head_taxonomy.py --self-test
    python3 bench/labels/label_refresh.py validate-runway \
        --dev-candidates bench/labels/default_head_label_runway_2026_07_13.dev.v1.json \
        --heldout-seal bench/labels/default_head_label_runway_2026_07_13.heldout.seal.v1.json \
        --labelset bench/labels/refactoring_families.v7.json \
        --evaluation bench/labels/product_quality_evaluation_v7_dev_runway_2026_07_13.v1.json
    python3 bench/labels/default_head_taxonomy.py validate \
        bench/labels/default_head_taxonomy_2026_07_13.dev.v1.json \
        --pragmatic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.pragmatic.v1.json \
        --dedupe bench/labels/default_head_taxonomy_votes_2026_07_13.dev.dedupe.v1.json \
        --skeptic bench/labels/default_head_taxonomy_votes_2026_07_13.dev.skeptic.v1.json
    python3 bench/labels/default_head_heldout.py validate
    python3 bench/labels/default_head_heldout.py self-test
    python3 bench/labels/default_head_heldout_commitment_receipt.py validate
    python3 bench/labels/default_head_heldout_commitment_receipt.py self-test
    python3 bench/labels/default_head_heldout_panel.py self-test
    python3 bench/labels/default_head_heldout_vote_receipt.py validate
    python3 bench/labels/default_head_heldout_vote_receipt.py self-test
    python3 bench/labels/default_head_heldout_arbitration.py self-test
    python3 bench/labels/default_head_heldout_arbitration.py validate
    python3 bench/labels/default_head_heldout_arbitration_receipt.py validate
    python3 bench/labels/default_head_heldout_arbitration_receipt.py self-test
    python3 bench/labels/default_head_heldout_arbitration_result.py self-test
    python3 bench/labels/default_head_heldout_arbitration_result.py validate-public \
        bench/labels/default_head_heldout_arbitration_result_2026_07_14.heldout.v3.json
    python3 bench/labels/default_head_heldout_arbitration_result_receipt.py validate
    python3 bench/labels/default_head_heldout_arbitration_result_receipt.py self-test
    python3 bench/labels/default_head_heldout_reveal.py self-test
    test ! -e bench/labels/.default_head_heldout_reveal.transaction.json
    test ! -L bench/labels/.default_head_heldout_reveal.transaction.json
    local reveal=bench/labels/default_head_heldout_reveal_2026_07_14.heldout.v3.json
    if [[ -e "$reveal" || -L "$reveal" ]]; then
        python3 bench/labels/default_head_heldout_reveal.py validate
        python3 bench/labels/default_head_heldout_reveal_receipt.py validate
        python3 bench/labels/default_head_heldout_reveal_receipt.py self-test
    fi
    python3 bench/labels/proof_actionability_no_go.py --self-test
    python3 bench/labels/residual_ranking.py validate
    python3 bench/labels/residual_ranking.py self-test
    python3 bench/labels/residual_ranking_topup.py validate
    python3 bench/labels/residual_ranking_topup.py self-test
    python3 bench/labels/residual_ranking_panel.py validate-arbitration
    python3 bench/labels/residual_ranking_panel.py validate-decisions
    python3 bench/labels/residual_ranking_panel.py validate-component
    python3 bench/labels/residual_ranking_panel.py self-test
    python3 bench/labels/residual_ranking_closeout.py validate
    python3 bench/labels/residual_ranking_closeout.py self-test
    python3 bench/labels/default_head_fresh_repository_audit.py
    python3 bench/labels/default_head_fresh_repository_audit.py --self-test
    python3 bench/labels/default_head_measurement_replay.py validate
    python3 bench/labels/default_head_measurement_replay.py self-test
    python3 bench/labels/default_head_closeout.py
    python3 bench/labels/default_head_closeout.py --self-test
    python3 eval/divergence_fire/replay.py selftest
    python3 eval/divergence_fire/replay.py check-artifacts
    python3 eval/divergence_fire/precision_protocol.py validate
    python3 eval/divergence_fire/precision_protocol.py self-test
    python3 eval/divergence_fire/precision_protocol_receipt.py validate
    python3 eval/divergence_fire/precision_protocol_receipt.py self-test
    python3 bench/labels/generated_provenance_behavior.py --self-test
    python3 bench/labels/generated_provenance_behavior.py validate
    python3 bench/labels/generated_provenance_closeout.py --self-test
    python3 bench/labels/generated_provenance_closeout.py
    python3 bench/labels/declaration_type_contract_behavior.py --self-test
    python3 bench/labels/declaration_type_contract_behavior.py validate
    python3 bench/labels/declaration_type_contract_closeout.py --self-test
    python3 bench/labels/declaration_type_contract_closeout.py
    python3 bench/labels/recall_ceiling_probe.py --self-test
    python3 bench/labels/missed_worthy_stage_audit.py --self-test
    python3 bench/labels/missed_worthy_heldout_confirmation.py --self-test
    python3 bench/labels/missed_worthy_source_bounds.py --self-test
    python3 bench/labels/accepted_pair_coverage.py --self-test
    python3 scripts/binary_identity.py --self-test
    python3 scripts/query-regression-harness.py --self-test
    python3 scripts/cache-query-regression.py --self-test
    python3 scripts/cache-query-regression.py --validate-receipt \
        bench/cache/issue-872-mutation-matrix-receipt-2026-07-20.v1.json
    python3 scripts/cache-query-regression.py --validate-report \
        bench/cache/issue-872-v0.19.0-vs-candidate-sympy-paired-2026-07-20.v1.json
    python3 scripts/cache-query-regression.py --validate-report \
        bench/cache/issue-873-portable-cas-sympy-paired-2026-07-20.v1.json
    python3 scripts/watch-session-benchmark.py --self-test
    python3 scripts/watch-session-benchmark.py --validate-report \
        bench/cache/issue-878-watch-session-2026-07-21.v1.json
    python3 scripts/check-release-evidence-0.20.0.py --self-test
    python3 scripts/check-release-evidence-0.20.0.py
    python3 scripts/ruby-redefinition-scaling.py --self-test
    python3 scripts/semantic-regression-summary.py --self-test
    python3 scripts/recall-loss-diff.py --self-test
    python3 scripts/check-query-regression.py --self-test
    python3 scripts/check-recall-loss-baselines.py --self-test
    python3 scripts/check-soundness-scorecard.py --self-test
    python3 scripts/check-soundness-scorecard.py
    python3 scripts/soundness-lab-gate.py self-test
    python3 scripts/soundness-lab-gate.py check
    python3 scripts/soundness_exclusions.py --self-test
    python3 scripts/soundness_exclusions.py
}

run_accepted_pair_coverage_checks() {
    need_cmd python3
    python3 bench/labels/accepted_pair_coverage.py \
        --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.baseline.v2.json
    python3 bench/labels/accepted_pair_coverage.py \
        --validate bench/labels/accepted_pair_coverage_2026_07_11.dev.head.v2.json
    python3 scripts/check-query-regression.py \
        bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.primary.v3.json \
        --same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.control.v3.json \
        --expected-drift-manifest bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.expected-drift.v1.json \
        --focused-report bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused.v3.json \
        --focused-same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.focused-control.v3.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.status.v3.json \
        --check-markdown bench/labels/accepted_pair_coverage_pricing_2026_07_11.semantic.summary.v3.md
    python3 scripts/check-query-regression.py \
        bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.primary.v3.json \
        --same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.control.v3.json \
        --expected-drift-manifest bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.expected-drift.v1.json \
        --focused-report bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.focused.v3.json \
        --focused-same-binary-control bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.focused-control.v3.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.status.v3.json \
        --check-markdown bench/labels/accepted_pair_coverage_pricing_2026_07_11.default.summary.v3.md
}

run_missed_worthy_frontier_checks() {
    need_cmd python3
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_2026_07_11.v2.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_2026_07_11.dev.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-decisions bench/labels/missed_worthy_audit_decisions_2026_07_11.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_2026_07_11.v2.json
    python3 bench/labels/missed_worthy_heldout_confirmation.py \
        --validate bench/labels/missed_worthy_stage_confirmation_2026_07_11.heldout.v1.json
    python3 bench/labels/missed_worthy_source_bounds.py \
        --validate bench/labels/missed_worthy_audit_source_bounds_2026_07_11.dev.v1.json
    python3 scripts/check-query-regression.py \
        bench/labels/missed_worthy_grouping_pricing_2026_07_11.primary.v1.json \
        --same-binary-control bench/labels/missed_worthy_grouping_pricing_2026_07_11.control.v1.json \
        --require-same-binary-control \
        --max-runtime-delta-pct 5 \
        --min-runtime-delta-ms 5 \
        --check-status bench/labels/missed_worthy_grouping_pricing_2026_07_11.status.v1.json \
        --check-markdown bench/labels/missed_worthy_grouping_pricing_2026_07_11.summary.md
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-closeout bench/labels/missed_worthy_frontier_closeout_2026_07_11.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_post_817_2026_07_12.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate-decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json
    python3 bench/labels/missed_worthy_source_bounds.py \
        --validate bench/labels/missed_worthy_audit_source_bounds_post_817_2026_07_12.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
        --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
    python3 bench/labels/missed_worthy_heldout_confirmation.py \
        --validate bench/labels/missed_worthy_stage_confirmation_post_817_2026_07_12.heldout.v2.json \
        --artifact bench/labels/recall_ceiling_probe_post_817_2026_07_12.v1.json \
        --decisions bench/labels/missed_worthy_audit_decisions_post_817_2026_07_12.dev.v2.json
    python3 bench/labels/recall_ceiling_probe.py \
        --validate bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_post_821_2026_07_13.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
    python3 bench/labels/missed_worthy_stage_audit.py \
        --validate bench/labels/missed_worthy_stage_audit_issue_832_2026_07_13.dev.v1.json \
        --artifact bench/labels/recall_ceiling_probe_post_821_2026_07_13.v1.json
}

run_product_query_schema_live_check() {
    need_cmd python3
    python3 bench/labels/live_query_schema.py --self-test --nose "$1"
}

run_shell_script_lint() {
    need_cmd shellcheck "install it with: brew install shellcheck"
    shellcheck -x .githooks/pre-commit .githooks/pre-push scripts/*.sh
}

run_msrv_check() {
    need_cmd rustup
    local msrv
    msrv="$(grep -m1 '^rust-version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
    if ! rustup toolchain list 2>/dev/null | grep -q "^${msrv}"; then
        echo "missing Rust MSRV toolchain: ${msrv}" >&2
        echo "install it with: rustup toolchain install ${msrv}" >&2
        exit 127
    fi
    cargo "+${msrv}" check --workspace --all-targets
}

run_semantic_pack_example_conformance() {
    local nose_bin="$1"
    "$nose_bin" semantic-pack check \
        docs/examples/semantic-packs/v0 \
        docs/examples/semantic-packs/v1 \
        --format json
    "$nose_bin" semantic-pack status \
        docs/examples/semantic-pack-lock-v1.json \
        --format json
}

run_coverage_gate() {
    need_cmd cargo
    need_cmd cargo-llvm-cov "install it with: cargo install cargo-llvm-cov"
    source scripts/coverage-threshold.env
    cargo llvm-cov \
        --workspace \
        --summary-only \
        --fail-under-lines "${NOSE_COVERAGE_FAIL_UNDER_LINES}"
}

run_supply_chain_checks() {
    need_cmd cargo-machete "install it with: cargo install cargo-machete"
    need_cmd cargo-deny "install it with: cargo install cargo-deny"
    cargo machete
    cargo deny check
}

run_named_gate() {
    local name="$1"
    shift
    case "$name" in
        corpus-prune-selftest)
            need_cmd python3
            python3 bench/prune_corpus.py --self-test
            ;;
        corpus-verify-selftest)
            ./scripts/corpus-verify-nightly.sh --self-test
            ;;
        semantic-pack-pricing)
            run_semantic_pack_pricing_selftest
            ;;
        type4-frontier)
            run_type4_frontier_evidence_checks
            ;;
        regression-selftests)
            run_regression_checker_selftests
            ;;
        missed-worthy-frontier)
            run_missed_worthy_frontier_checks
            ;;
        accepted-pair-coverage)
            run_accepted_pair_coverage_checks
            ;;
        cargo-target-prune-selftest)
            ./scripts/prune-cargo-target.sh --self-test
            ;;
        shell-lint)
            run_shell_script_lint
            ;;
        format)
            need_cmd cargo
            cargo fmt --all --check
            ;;
        file-length)
            run_file_length_ratchet "${1:-origin/main}"
            ;;
        legacy-prelude)
            run_legacy_prelude_guard
            ;;
        clippy)
            need_cmd cargo
            cargo clippy --all-targets --all-features -- -D warnings
            ;;
        doc)
            need_cmd cargo
            RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --quiet
            ;;
        build-debug-cli)
            need_cmd cargo
            cargo build -p nose-cli
            ;;
        build-release)
            need_cmd cargo
            cargo build --release
            ;;
        test-debug-cli)
            need_cmd cargo
            cargo test -p nose-cli
            ;;
        test-release)
            need_cmd cargo
            cargo test --release
            ;;
        product-query-schema)
            run_product_query_schema_live_check "$1"
            ;;
        semantic-pack-examples)
            run_semantic_pack_example_conformance "$1"
            ;;
        type4-executable)
            run_type4_executable_expectations "$1"
            ;;
        type4-axis-language)
            run_type4_axis_language_claims "$1" "${2:-origin/main}"
            ;;
        coverage)
            run_coverage_gate
            ;;
        duplication)
            ./scripts/check-duplication.sh
            ;;
        msrv)
            run_msrv_check
            ;;
        supply-chain)
            run_supply_chain_checks
            ;;
        docs)
            run_docs_wiki_lint
            ;;
        formal-obligations)
            run_formal_obligations_lint
            ;;
        formal-lean)
            run_formal_lean
            ;;
        *)
            echo "unknown CI gate: $name" >&2
            exit 2
            ;;
    esac
}

if [[ "$mode" == "gate" ]]; then
    run_named_gate "$gate_name" "${gate_args[@]}"
    exit 0
fi

need_cmd cargo
source scripts/coverage-threshold.env

step "corpus prune self-test"
run_named_gate corpus-prune-selftest

step "corpus verify runner self-test"
run_named_gate corpus-verify-selftest

step "semantic-pack pricing self-test"
run_named_gate semantic-pack-pricing

step "Type-4 frontier evidence checks"
run_named_gate type4-frontier

step "regression checker self-tests"
run_named_gate regression-selftests

step "current missed-worthy frontier artifacts"
run_named_gate missed-worthy-frontier

step "current accepted-pair coverage artifacts"
run_named_gate accepted-pair-coverage

step "Cargo target prune self-test"
run_named_gate cargo-target-prune-selftest

step "shell scripts (shellcheck)"
run_named_gate shell-lint

step "rustfmt (formatting)"
run_named_gate format

step "Rust file-length ratchet"
run_named_gate file-length origin/main

step "CLI legacy-prelude guard"
run_named_gate legacy-prelude

step "clippy (lints, -D warnings)"
run_named_gate clippy

if [[ "$mode" == "fast" ]]; then
    step "nose-cli tests"
    run_named_gate test-debug-cli

    step "product query JSON schema"
    run_named_gate build-debug-cli
    run_named_gate product-query-schema target/debug/nose

    step "Type-4 executable focused expectations"
    run_named_gate type4-executable target/debug/nose

    step "Type-4 axis-language claim perimeter"
    run_named_gate type4-axis-language target/debug/nose origin/main

    step "docs wiki connectivity (awiki)"
    run_named_gate docs

    printf '\n\033[1;32mFast local CI gates passed.\033[0m\n'
    exit 0
fi

step "doc (rustdoc warnings)"
run_named_gate doc

step "build (release)"
run_named_gate build-release

step "product query JSON schema"
run_named_gate product-query-schema target/release/nose

step "semantic-pack example conformance"
run_named_gate semantic-pack-examples target/release/nose

step "Type-4 executable focused expectations"
run_named_gate type4-executable target/release/nose

step "Type-4 axis-language claim perimeter"
run_named_gate type4-axis-language target/release/nose origin/main

step "test (release)"
run_named_gate test-release

# CI runs the same coverage ratchet before PR merge and before release publishing.
# Keep it here so --full stays a complete local mirror.
step "coverage gate (cargo-llvm-cov, >= ${NOSE_COVERAGE_FAIL_UNDER_LINES}% lines)"
run_named_gate coverage

step "duplication gate (nose on itself)"
run_named_gate duplication

step "MSRV (minimum supported rust version)"
run_named_gate msrv

step "supply chain (unused dependencies / advisories / licenses)"
run_named_gate supply-chain

step "docs wiki connectivity (awiki)"
run_named_gate docs

step "formal obligation registry"
run_named_gate formal-obligations

step "Lean proofs (formal soundness)"
run_named_gate formal-lean

printf '\n\033[1;32mFull local CI gates passed.\033[0m\n'
