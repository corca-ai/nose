#!/usr/bin/env bash
# Run every quality gate locally, in the same order CI does. Mirrors .github/
# workflows/ci.yml so a green run here means a green CI.
#
#   ./scripts/check.sh
#
# Optional tools (cargo-machete, cargo-deny) are skipped with a notice if
# absent; install them with:
#   cargo install cargo-machete cargo-deny
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

step "rustfmt (formatting)"
cargo fmt --all --check

step "clippy (lints, -D warnings)"
cargo clippy --all-targets --all-features -- -D warnings

step "doc (broken intra-doc links)"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --quiet

step "build (release)"
cargo build --release

step "test"
cargo test --release

step "cargo-machete (unused dependencies)"
if command -v cargo-machete >/dev/null 2>&1; then
    cargo machete
else
    echo "skipped — cargo install cargo-machete"
fi

step "cargo-deny (advisories / licenses / bans / sources)"
if command -v cargo-deny >/dev/null 2>&1; then
    cargo deny check
else
    echo "skipped — cargo install cargo-deny"
fi

step "MSRV (minimum supported rust version)"
msrv="$(grep -m1 '^rust-version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')"
if rustup toolchain list 2>/dev/null | grep -q "^${msrv}"; then
    cargo "+${msrv}" check --workspace --all-targets
else
    echo "skipped — rustup toolchain install ${msrv}"
fi

step "copy-paste gate (nose on itself)"
./scripts/check-duplication.sh

printf '\n\033[1;32mAll quality gates passed.\033[0m\n'
