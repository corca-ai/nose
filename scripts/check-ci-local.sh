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

  --fast  rustfmt, clippy -D warnings, nose-cli tests, docs wiki lint
  --full  full local mirror of CI: format, clippy, docs, release build/tests,
          duplication, MSRV, supply-chain, docs wiki, formal obligation lint,
          and Lean proofs
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

step "corpus prune self-test"
need_cmd python3
python3 bench/prune_corpus.py --self-test

step "corpus verify runner self-test"
./scripts/corpus-verify-nightly.sh --self-test

step "rustfmt (formatting)"
cargo fmt --all --check

step "clippy (lints, -D warnings)"
cargo clippy --all-targets --all-features -- -D warnings

if [[ "$mode" == "fast" ]]; then
    step "nose-cli tests"
    cargo test -p nose-cli

    step "docs wiki connectivity (awiki)"
    run_docs_wiki_lint

    printf '\n\033[1;32mFast local CI gates passed.\033[0m\n'
    exit 0
fi

step "doc (rustdoc warnings)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --quiet

step "build (release)"
cargo build --release

step "test (release)"
cargo test --release

# In CI this is a POST-MERGE gate (runs on push to main, not on PRs — it is the
# slowest job and is a quality ratchet, not a soundness gate). Kept here so --full
# stays a complete local mirror; run it before merging if you touched test coverage.
step "coverage gate (cargo-llvm-cov, >= 86% lines)"
need_cmd cargo-llvm-cov "install it with: cargo install cargo-llvm-cov"
cargo llvm-cov --workspace --summary-only --fail-under-lines 86

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
