#!/usr/bin/env bash
# Local CI preflight.
#
# Modes:
#   --fast        PR/push preflight: catches the common CI failures quickly.
#   --full        Full local mirror of the GitHub Actions gates.
#   --gate <name> Run one named gate. GitHub Actions uses this internal surface
#                 so local and remote checks have one command owner.
#   --list-gates  Render the checked gate inventory.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="fast"
gate_name=""
gate_args=()
list_format="text"
parallel_jobs="${NOSE_CI_JOBS:-1}"
case "${1:-}" in
    "")
        mode="fast"
        ;;
    --fast)
        mode="fast"
        shift
        ;;
    --full)
        mode="full"
        shift
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
    --list-gates)
        mode="list"
        if [[ "${2:-}" == "--format" ]]; then
            list_format="${3:-}"
            if [[ "$list_format" != "text" && "$list_format" != "json" ]]; then
                echo "--list-gates format must be text or json" >&2
                exit 2
            fi
        elif [[ -n "${2:-}" ]]; then
            echo "unknown --list-gates argument: $2" >&2
            exit 2
        fi
        ;;
    --validate-gates)
        mode="validate"
        ;;
    -h | --help)
        cat <<'EOF'
usage: ./scripts/check-ci-local.sh [--fast|--full] [--jobs <count>]
       ./scripts/check-ci-local.sh --gate <name> [gate arguments...]
       ./scripts/check-ci-local.sh --list-gates [--format text|json]
       ./scripts/check-ci-local.sh --validate-gates

  --fast  corpus and semantic-pack self-tests, Type-4 packet/replay checks,
          rustfmt, file-length ratchet, legacy-prelude guard, shellcheck,
          clippy -D warnings, nose-cli tests, docs wiki lint
  --full  full local mirror of CI: format, clippy, docs, release build/tests,
          file-length ratchet, duplication, MSRV, supply-chain, docs wiki,
          formal obligation lint, and Lean proofs
  --jobs  opt into bounded parallel local-plan execution; default: 1
  --gate  internal named-gate surface shared with GitHub Actions
  --list-gates
          authoritative owner, lane, effect, cache, and focused-command inventory
  --validate-gates
          verify registry, dispatcher, local plans, and workflow membership
EOF
        exit 0
        ;;
    *)
        echo "unknown mode: $1" >&2
        echo "usage: ./scripts/check-ci-local.sh [--fast|--full]" >&2
        exit 2
        ;;
esac

if [[ "$mode" == "fast" || "$mode" == "full" ]]; then
    if [[ "${1:-}" == "--jobs" ]]; then
        if [[ -z "${2:-}" ]]; then
            echo "--jobs requires a positive integer" >&2
            exit 2
        fi
        parallel_jobs="$2"
        shift 2
    fi
    if [[ "$#" -ne 0 ]]; then
        echo "unknown local-plan argument: $1" >&2
        exit 2
    fi
    if [[ ! "$parallel_jobs" =~ ^[1-9][0-9]*$ ]]; then
        echo "--jobs must be a positive integer: $parallel_jobs" >&2
        exit 2
    fi
fi

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

need_python3() {
    need_cmd python3
    if ! python3 -c 'import sys; raise SystemExit(sys.version_info < (3, 10))'; then
        echo "Python 3.10 or newer is required for repository quality gates." >&2
        echo "observed: $(python3 --version 2>&1)" >&2
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

run_evidence_artifact_lifecycle() {
    need_cmd python3
    python3 scripts/evidence/validate_artifacts.py --self-test
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

source scripts/ci/evidence-gates.sh

run_product_query_schema_live_check() {
    need_cmd python3
    python3 bench/labels/live_query_schema.py --self-test --nose "$1"
}

run_shell_script_lint() {
    need_cmd shellcheck "install it with: brew install shellcheck"
    shellcheck -x .githooks/pre-commit .githooks/pre-push scripts/*.sh scripts/ci/*.sh
}

run_msrv_check() {
    need_cmd rustup
    local msrv
    local msrv_target_dir="${NOSE_MSRV_TARGET_DIR:-target/msrv}"
    msrv="$(grep -m1 '^rust-version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
    if ! rustup toolchain list 2>/dev/null | grep -q "^${msrv}"; then
        echo "missing Rust MSRV toolchain: ${msrv}" >&2
        echo "install it with: rustup toolchain install ${msrv}" >&2
        exit 127
    fi
    CARGO_TARGET_DIR="$msrv_target_dir" \
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

run_gate_registry_validation() {
    need_python3
    python3 scripts/ci/gate_registry.py validate
}

run_local_plan() {
    local plan_mode="$1"
    local planned_name
    local planned_label
    local planned_arg_one
    local planned_arg_two
    local planned_args

    while IFS='|' read -r \
        planned_name planned_label planned_arg_one planned_arg_two; do
        [[ -n "$planned_name" ]] || continue
        planned_args=()
        if [[ -n "$planned_arg_one" ]]; then
            planned_args+=("$planned_arg_one")
        fi
        if [[ -n "$planned_arg_two" ]]; then
            planned_args+=("$planned_arg_two")
        fi
        step "$planned_label"
        if [[ "${#planned_args[@]}" -eq 0 ]]; then
            run_named_gate "$planned_name"
        else
            run_named_gate "$planned_name" "${planned_args[@]}"
        fi
    done < <(python3 scripts/ci/gate_registry.py plan --mode "$plan_mode")
}

run_parallel_local_plan() {
    local plan_mode="$1"
    local jobs="$2"
    need_python3
    python3 scripts/ci/run_plan.py --mode "$plan_mode" --jobs "$jobs"
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
        default-head-evidence)
            run_default_head_evidence_checks
            ;;
        divergence-evidence)
            run_divergence_evidence_checks
            ;;
        surface-recall-evidence)
            run_surface_and_recall_evidence_checks
            ;;
        runtime-soundness-evidence)
            run_runtime_and_soundness_evidence_checks
            ;;
        evidence-artifacts)
            run_evidence_artifact_lifecycle
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
        test-ci-compile)
            need_cmd cargo
            cargo test --workspace --profile ci-test --no-run
            ;;
        test-ci)
            need_cmd cargo
            cargo test --workspace --profile ci-test
            ;;
        test-release-compile)
            need_cmd cargo
            cargo test --workspace --release --no-run
            ;;
        test-release)
            need_cmd cargo
            cargo test --workspace --release
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
    run_gate_registry_validation
    if [[ "${#gate_args[@]}" -eq 0 ]]; then
        run_named_gate "$gate_name"
    else
        run_named_gate "$gate_name" "${gate_args[@]}"
    fi
    exit 0
fi

if [[ "$mode" == "list" ]]; then
    need_python3
    python3 scripts/ci/gate_registry.py list --format "$list_format"
    exit 0
fi

if [[ "$mode" == "validate" ]]; then
    run_gate_registry_validation
    python3 scripts/ci/gate_registry.py validate --self-test
    python3 scripts/ci/run_plan.py --self-test
    exit 0
fi

run_gate_registry_validation
if [[ "$parallel_jobs" -eq 1 ]]; then
    run_local_plan "$mode"
else
    run_parallel_local_plan "$mode" "$parallel_jobs"
fi
if [[ "$mode" == "fast" ]]; then
    printf '\n\033[1;32mFast local CI gates passed.\033[0m\n'
else
    printf '\n\033[1;32mFull local CI gates passed.\033[0m\n'
fi
