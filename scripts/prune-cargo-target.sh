#!/usr/bin/env bash
# Prune stale rustc codegen objects from target/debug/deps.
#
# On macOS, very large Cargo deps directories can make ad-hoc signed test
# binaries spend tens of seconds in dyld/code-signing validation before Rust
# code starts. The stale *.rcgu.o files are safe to remove: Cargo will rebuild
# anything it still needs.
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_path="$script_dir/$(basename "${BASH_SOURCE[0]}")"
cd "$script_dir/.."

usage() {
    cat <<'EOF'
usage: ./scripts/prune-cargo-target.sh [--dry-run|--self-test]

Prunes stale target/debug/deps/*.rcgu.o files when the object count is high.

Environment:
  NOSE_PRUNE_CARGO_TARGET=0          skip pruning entirely
  NOSE_CARGO_TARGET_PRUNE_DAYS=N     prune objects older than N days (default: 3)
  NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS=N
                                    prune only when object count exceeds N
                                    (default: 50000)
  NOSE_CARGO_TARGET_PRUNE_DIR=PATH   Cargo target dir (default: CARGO_TARGET_DIR
                                    or target)
EOF
}

run_self_test() {
    local script="$1"
    local temp_dir deps dry_output
    temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/nose-prune-cargo-target-test.XXXXXX")"
    deps="$temp_dir/target/debug/deps"
    mkdir -p "$deps"

    touch "$deps/recent.abc.rcgu.o" "$deps/keep.d" "$deps/keep.rlib" "$deps/test-bin"
    touch -t 202001010000 "$deps/old.abc.rcgu.o"
    chmod +x "$deps/test-bin"

    dry_output="$(
        NOSE_PRUNE_CARGO_TARGET=1 \
        NOSE_CARGO_TARGET_PRUNE_SKIP_ACTIVE_CHECK=1 \
            NOSE_CARGO_TARGET_PRUNE_DIR="$temp_dir/target" \
            NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS=1 \
            NOSE_CARGO_TARGET_PRUNE_DAYS=0 \
            "$script" --dry-run
    )"
    if [[ "$dry_output" != *"would remove 1 stale .rcgu.o files"* ]]; then
        echo "self-test failed: dry-run did not report one stale object" >&2
        echo "$dry_output" >&2
        rm -rf "$temp_dir"
        return 1
    fi
    if [[ ! -f "$deps/old.abc.rcgu.o" ]]; then
        echo "self-test failed: dry-run removed a file" >&2
        rm -rf "$temp_dir"
        return 1
    fi

    NOSE_PRUNE_CARGO_TARGET=1 \
    NOSE_CARGO_TARGET_PRUNE_SKIP_ACTIVE_CHECK=1 \
        NOSE_CARGO_TARGET_PRUNE_DIR="$temp_dir/target" \
        NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS=1 \
        NOSE_CARGO_TARGET_PRUNE_DAYS=0 \
        "$script" >/dev/null
    for expected in recent.abc.rcgu.o keep.d keep.rlib test-bin; do
        if [[ ! -e "$deps/$expected" ]]; then
            echo "self-test failed: removed non-stale file $expected" >&2
            rm -rf "$temp_dir"
            return 1
        fi
    done
    if [[ -e "$deps/old.abc.rcgu.o" ]]; then
        echo "self-test failed: stale object was not removed" >&2
        rm -rf "$temp_dir"
        return 1
    fi

    rm -rf "$temp_dir"
    temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/nose-prune-cargo-target-test.XXXXXX")"
    deps="$temp_dir/target/debug/deps"
    mkdir -p "$deps"
    touch -t 202001010000 "$deps/old.abc.rcgu.o"
    NOSE_PRUNE_CARGO_TARGET=1 \
    NOSE_CARGO_TARGET_PRUNE_SKIP_ACTIVE_CHECK=1 \
        NOSE_CARGO_TARGET_PRUNE_DIR="$temp_dir/target" \
        NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS=10 \
        NOSE_CARGO_TARGET_PRUNE_DAYS=0 \
        "$script"
    if [[ ! -e "$deps/old.abc.rcgu.o" ]]; then
        echo "self-test failed: threshold guard deleted an object" >&2
        rm -rf "$temp_dir"
        return 1
    fi
    rm -rf "$temp_dir"

    echo "cargo target prune self-test passed"
}

dry_run=0
case "${1:-}" in
    "")
        ;;
    --dry-run)
        dry_run=1
        ;;
    --self-test)
        run_self_test "$script_path"
        exit $?
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        echo "unknown argument: $1" >&2
        usage >&2
        exit 2
        ;;
esac

if [[ "${NOSE_PRUNE_CARGO_TARGET:-1}" == "0" ]]; then
    echo "NOSE_PRUNE_CARGO_TARGET=0: skipping Cargo target prune"
    exit 0
fi

is_non_negative_int() {
    [[ "$1" =~ ^[0-9]+$ ]]
}

is_positive_int() {
    [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

prune_days="${NOSE_CARGO_TARGET_PRUNE_DAYS:-3}"
min_objects="${NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS:-50000}"
target_dir="${NOSE_CARGO_TARGET_PRUNE_DIR:-${CARGO_TARGET_DIR:-target}}"

if ! is_non_negative_int "$prune_days"; then
    echo "NOSE_CARGO_TARGET_PRUNE_DAYS must be a non-negative integer" >&2
    exit 2
fi

if ! is_positive_int "$min_objects"; then
    echo "NOSE_CARGO_TARGET_PRUNE_MIN_OBJECTS must be a positive integer" >&2
    exit 2
fi

deps_dir="${target_dir%/}/debug/deps"
if [[ ! -d "$deps_dir" ]]; then
    exit 0
fi

if [[ "${NOSE_CARGO_TARGET_PRUNE_SKIP_ACTIVE_CHECK:-0}" != "1" ]] &&
    command -v pgrep >/dev/null 2>&1; then
    if pgrep -x cargo >/dev/null 2>&1 ||
        pgrep -x rustc >/dev/null 2>&1 ||
        pgrep -x rustdoc >/dev/null 2>&1 ||
        pgrep -x clippy-driver >/dev/null 2>&1; then
        echo "Cargo target prune skipped: cargo/rustc appears to be running"
        exit 0
    fi
fi

lock_dir="${target_dir%/}/.nose-prune-cargo-target.lock"
if ! mkdir "$lock_dir" 2>/dev/null; then
    echo "Cargo target prune skipped: another prune is already running"
    exit 0
fi

tmp=""
cleanup() {
    if [[ -n "$tmp" ]]; then
        rm -f "$tmp"
    fi
    rmdir "$lock_dir" 2>/dev/null || true
}
trap cleanup EXIT

object_count=0
while IFS= read -r _path; do
    object_count=$((object_count + 1))
    if ((object_count > min_objects)); then
        break
    fi
done < <(find "$deps_dir" -maxdepth 1 -type f -name '*.rcgu.o' -print 2>/dev/null)

if ((object_count <= min_objects)); then
    exit 0
fi

tmp="$(mktemp "${TMPDIR:-/tmp}/nose-prune-cargo-target.XXXXXX")"
find "$deps_dir" -maxdepth 1 -type f -name '*.rcgu.o' -mtime +"$prune_days" -print0 >"$tmp"

if [[ ! -s "$tmp" ]]; then
    echo "Cargo target prune: ${object_count}+ .rcgu.o files, none older than ${prune_days} days"
    exit 0
fi

prunable_count="$(tr -cd '\000' <"$tmp" | wc -c | tr -d '[:space:]')"

if ((dry_run)); then
    echo "Cargo target prune: would remove ${prunable_count} stale .rcgu.o files from ${deps_dir}"
    exit 0
fi

xargs -0 rm -f <"$tmp"
echo "Cargo target prune: removed ${prunable_count} stale .rcgu.o files from ${deps_dir}"
