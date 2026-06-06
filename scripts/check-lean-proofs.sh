#!/usr/bin/env bash
# Type-check the Lean proof corpus.
#
# Obligation proofs may import shared modules from formal/lib. Lean needs those
# modules compiled to .olean files first; keep that build output under target/lean
# so the formal tree stays source-only.
set -euo pipefail
cd "$(dirname "$0")/.."

toolchain="$(cat lean-toolchain 2>/dev/null || printf 'leanprover/lean4:v4.30.0')"
if command -v elan >/dev/null 2>&1; then
    lean_cmd=(elan run "$toolchain" lean)
else
    if ! command -v lean >/dev/null 2>&1; then
        echo "missing required command: lean" >&2
        exit 127
    fi
    lean_cmd=(lean)
fi

build_dir="target/lean"
mkdir -p "$build_dir"

while IFS= read -r f; do
    out="$build_dir/${f%.lean}.olean"
    mkdir -p "$(dirname "$out")"
    echo "building $f"
    LEAN_PATH="$PWD/$build_dir${LEAN_PATH:+:$LEAN_PATH}" \
        "${lean_cmd[@]}" --error=warning -R . -o "$out" "$f"
done < <(find formal/lib -name '*.lean' -print | sort)

while IFS= read -r f; do
    echo "checking $f"
    LEAN_PATH="$PWD/$build_dir${LEAN_PATH:+:$LEAN_PATH}" \
        "${lean_cmd[@]}" --error=warning -R . "$f"
done < <(find formal/obligations -name '*.lean' -print | sort)
