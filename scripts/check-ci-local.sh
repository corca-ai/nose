#!/usr/bin/env bash
# Local CI preflight.
#
# Modes:
#   --fast  PR/push preflight: catches the common CI failures quickly.
#   --full  Full local mirror of the GitHub Actions gates.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="fast"
case "${1:-}" in
    "" | --fast)
        mode="fast"
        ;;
    --full)
        mode="full"
        ;;
    -h | --help)
        cat <<'EOF'
usage: ./scripts/check-ci-local.sh [--fast|--full]

  --fast  corpus and semantic-pack self-tests, Type-4 packet/replay checks,
          rustfmt, file-length ratchet, legacy-prelude guard, shellcheck,
          clippy -D warnings, nose-cli tests, docs wiki lint
  --full  full local mirror of CI: format, clippy, docs, release build/tests,
          file-length ratchet, duplication, MSRV, supply-chain, docs wiki,
          formal obligation lint, and Lean proofs
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
    if ! git rev-parse --verify origin/main >/dev/null 2>&1; then
        echo "missing origin/main; fetch it before running the local file-length ratchet" >&2
        echo "try: git fetch origin main" >&2
        exit 127
    fi
    python3 scripts/check-file-lengths.py --ratchet-base origin/main
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

run_regression_checker_selftests() {
    need_cmd python3
    python3 bench/labels/query_schema.py --self-test
    python3 bench/labels/default_head_query_schema.py --self-test
    python3 bench/labels/eval_by_language.py --self-test
    python3 bench/labels/check_default_head_baseline.py
    python3 bench/labels/labelset.py --self-test
    python3 bench/labels/label_refresh.py --self-test
    python3 bench/labels/recall_ceiling_probe.py --self-test
    python3 bench/labels/missed_worthy_stage_audit.py --self-test
    python3 bench/labels/missed_worthy_heldout_confirmation.py --self-test
    python3 bench/labels/missed_worthy_source_bounds.py --self-test
    python3 bench/labels/accepted_pair_coverage.py --self-test
    python3 scripts/binary_identity.py --self-test
    python3 scripts/query-regression-harness.py --self-test
    python3 scripts/ruby-redefinition-scaling.py --self-test
    python3 scripts/semantic-regression-summary.py --self-test
    python3 scripts/recall-loss-diff.py --self-test
    python3 scripts/check-query-regression.py --self-test
    python3 scripts/check-recall-loss-baselines.py --self-test
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
    python3 bench/labels/query_schema.py --self-test --nose "$1"
    python3 bench/labels/default_head_query_schema.py --self-test --nose "$1"
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

need_cmd cargo
source scripts/coverage-threshold.env

step "corpus prune self-test"
need_cmd python3
python3 bench/prune_corpus.py --self-test

step "corpus verify runner self-test"
./scripts/corpus-verify-nightly.sh --self-test

step "semantic-pack pricing self-test"
run_semantic_pack_pricing_selftest

step "Type-4 frontier evidence checks"
run_type4_frontier_evidence_checks

step "regression checker self-tests"
run_regression_checker_selftests

step "current missed-worthy frontier artifacts"
run_missed_worthy_frontier_checks

step "current accepted-pair coverage artifacts"
run_accepted_pair_coverage_checks

step "Cargo target prune self-test"
./scripts/prune-cargo-target.sh --self-test

step "shell scripts (shellcheck)"
run_shell_script_lint

step "rustfmt (formatting)"
cargo fmt --all --check

step "Rust file-length ratchet"
run_file_length_ratchet

step "CLI legacy-prelude guard"
run_legacy_prelude_guard

step "clippy (lints, -D warnings)"
cargo clippy --all-targets --all-features -- -D warnings

if [[ "$mode" == "fast" ]]; then
    step "nose-cli tests"
    cargo test -p nose-cli

    step "product query JSON schema"
    cargo build -p nose-cli
    run_product_query_schema_live_check target/debug/nose

    step "Type-4 executable focused expectations"
    run_type4_executable_expectations target/debug/nose

    step "docs wiki connectivity (awiki)"
    run_docs_wiki_lint

    printf '\n\033[1;32mFast local CI gates passed.\033[0m\n'
    exit 0
fi

step "doc (rustdoc warnings)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --quiet

step "build (release)"
cargo build --release

step "product query JSON schema"
run_product_query_schema_live_check target/release/nose

step "semantic-pack example conformance"
target/release/nose semantic-pack check docs/examples/semantic-packs/v0 --format json

step "Type-4 executable focused expectations"
run_type4_executable_expectations target/release/nose

step "test (release)"
cargo test --release

# CI runs the same coverage ratchet before PR merge and before release publishing.
# Keep it here so --full stays a complete local mirror.
step "coverage gate (cargo-llvm-cov, >= ${NOSE_COVERAGE_FAIL_UNDER_LINES}% lines)"
need_cmd cargo-llvm-cov "install it with: cargo install cargo-llvm-cov"
cargo llvm-cov --workspace --summary-only --fail-under-lines "${NOSE_COVERAGE_FAIL_UNDER_LINES}"

step "duplication gate (nose on itself)"
./scripts/check-duplication.sh

step "MSRV (minimum supported rust version)"
run_msrv_check

step "cargo-machete (unused dependencies)"
need_cmd cargo-machete "install it with: cargo install cargo-machete"
cargo machete

step "cargo-deny (advisories / licenses / bans / sources)"
need_cmd cargo-deny "install it with: cargo install cargo-deny"
cargo deny check

step "docs wiki connectivity (awiki)"
run_docs_wiki_lint

step "formal obligation registry"
run_formal_obligations_lint

step "Lean proofs (formal soundness)"
run_formal_lean

printf '\n\033[1;32mFull local CI gates passed.\033[0m\n'
