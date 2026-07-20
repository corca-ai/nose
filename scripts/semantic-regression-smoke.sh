#!/usr/bin/env bash
# Build and compare base/head release binaries on the pinned semantic smoke corpus.
set -euo pipefail
cd "$(dirname "$0")/.."
runner_command="$(printf '%q ' "$0" "$@")"

base_ref="origin/main"
head_ref="HEAD"
repos_root="target/semantic-regression/repos"
artifact_dir="target/semantic-regression/artifacts"
baseline_binary=""
current_binary=""
skip_setup=false
force=false
relevance_only=false
expected_drift_manifest=".github/semantic-regression-expected-drift.json"
repos=(fastlane asciidoctor sidekiq alacritty requests junit5 prettier)

usage() {
  cat <<'EOF'
usage: scripts/semantic-regression-smoke.sh [options]

  --base-ref REF                 comparison base (default: origin/main)
  --head-ref REF                 comparison head (default: HEAD)
  --repos-root PATH              pinned subset location
  --artifact-dir PATH            raw report and summary destination
  --baseline-binary PATH         reuse a prebuilt base binary
  --current-binary PATH          reuse a prebuilt head binary
  --skip-setup                   trust the supplied repos root
  --force                        run even when the diff is not relevant
  --relevance-only               print true/false and exit
  --expected-drift-manifest PATH exact intentional-output ledger
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-ref) base_ref="$2"; shift 2 ;;
    --head-ref) head_ref="$2"; shift 2 ;;
    --repos-root) repos_root="$2"; shift 2 ;;
    --artifact-dir) artifact_dir="$2"; shift 2 ;;
    --baseline-binary) baseline_binary="$2"; shift 2 ;;
    --current-binary) current_binary="$2"; shift 2 ;;
    --skip-setup) skip_setup=true; shift ;;
    --force) force=true; shift ;;
    --relevance-only) relevance_only=true; shift ;;
    --expected-drift-manifest) expected_drift_manifest="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ -n "$baseline_binary" || -n "$current_binary" ]]; then
  if [[ -z "$baseline_binary" || -z "$current_binary" ]]; then
    echo "--baseline-binary and --current-binary must be supplied together" >&2
    exit 2
  fi
fi

base_sha="$(git rev-parse --verify "$base_ref^{commit}")"
head_sha="$(git rev-parse --verify "$head_ref^{commit}")"

is_relevant_path() {
  case "$1" in
    Cargo.toml|Cargo.lock|rust-toolchain.toml|crates/*|\
    bench/goldens/corpus.json|bench/labels/prune_manifest.json|\
    bench/semantic_regression_corpus.v1.json|bench/setup_repos.sh|\
    bench/prune_corpus.py|bench/corpus_prune/*|\
    .github/semantic-regression-expected-drift.json|.github/workflows/ci.yml|\
    scripts/query-regression-harness.py|scripts/check-query-regression.py|\
    scripts/ruby-redefinition-scaling.py|scripts/semantic-regression-smoke.sh|\
    scripts/semantic-regression-summary.py)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

relevant=false
while IFS= read -r changed_path; do
  if is_relevant_path "$changed_path"; then
    relevant=true
    break
  fi
done < <(git diff --name-only "$base_sha" "$head_sha")

if $relevance_only; then
  echo "$relevant"
  exit 0
fi
if ! $force && ! $relevant; then
  mkdir -p "$artifact_dir"
  cat >"$artifact_dir/summary.md" <<EOF
## Semantic regression smoke

**Status:** \`skipped\`

No semantic, lowering, normalization, query, harness, or pinned-corpus files changed
between \`$base_sha\` and \`$head_sha\`.
EOF
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    cat "$artifact_dir/summary.md" >>"$GITHUB_STEP_SUMMARY"
  fi
  echo "semantic regression smoke skipped: no relevant changes"
  exit 0
fi

mkdir -p "$artifact_dir" "$repos_root"
rm -f \
  "$artifact_dir/primary.json" \
  "$artifact_dir/primary-control.json" \
  "$artifact_dir/focused.json" \
  "$artifact_dir/focused-control.json" \
  "$artifact_dir/check-status.json" \
  "$artifact_dir/context.json" \
  "$artifact_dir/ruby-scaling.json" \
  "$artifact_dir/summary.md"

worktree_root=""
cleanup() {
  if [[ -n "$worktree_root" ]]; then
    git worktree remove --force "$worktree_root/base" >/dev/null 2>&1 || true
    git worktree remove --force "$worktree_root/head" >/dev/null 2>&1 || true
    rm -rf "$worktree_root"
  fi
}
trap cleanup EXIT

if [[ -z "$baseline_binary" ]]; then
  worktree_root="$(mktemp -d "${TMPDIR:-/tmp}/nose-semantic-regression.XXXXXX")"
  git worktree add --detach "$worktree_root/base" "$base_sha"
  git worktree add --detach "$worktree_root/head" "$head_sha"
  cargo_target="$(pwd)/target/semantic-regression/cargo"
  (
    cd "$worktree_root/base"
    cargo build --release --locked --target-dir "$cargo_target"
  )
  baseline_binary="$(pwd)/target/semantic-regression/baseline-nose"
  cp "$cargo_target/release/nose" "$baseline_binary"
  # Cargo's package fingerprints do not distinguish two worktrees of the same
  # package version in one target directory. Without this workspace-only clean,
  # the head build can reuse a base-worktree rlib across an internal API change
  # and fail (or, worse, produce a mixed-revision binary). Keep the Cargo cache,
  # but rebuild the release target from the head tree.
  (
    cd "$worktree_root/head"
    cargo clean --release --workspace --target-dir "$cargo_target"
  )
  (
    cd "$worktree_root/head"
    cargo build --release --locked --target-dir "$cargo_target"
  )
  current_binary="$(pwd)/target/semantic-regression/current-nose"
  cp "$cargo_target/release/nose" "$current_binary"
  build_mode="worktrees"
  base_build_cwd="$worktree_root/base"
  head_build_cwd="$worktree_root/head"
  build_command="cargo build --release --locked --target-dir $cargo_target (workspace artifacts cleaned between revisions)"
else
  build_mode="prebuilt"
  base_build_cwd=""
  head_build_cwd=""
  build_command=""
fi

baseline_binary="$(cd "$(dirname "$baseline_binary")" && pwd)/$(basename "$baseline_binary")"
current_binary="$(cd "$(dirname "$current_binary")" && pwd)/$(basename "$current_binary")"
repos_root="$(cd "$repos_root" && pwd)"
artifact_dir="$(cd "$artifact_dir" && pwd)"
expected_drift_manifest="$(cd "$(dirname "$expected_drift_manifest")" && pwd)/$(basename "$expected_drift_manifest")"

if ! $skip_setup; then
  setup_args=(--repos-root "$repos_root")
  for repo in "${repos[@]}"; do
    setup_args+=(--repo "$repo")
  done
  bench/setup_repos.sh "${setup_args[@]}"
fi

NOSE_RUNNER_COMMAND="$runner_command" \
NOSE_BUILD_MODE="$build_mode" \
NOSE_BASE_BUILD_CWD="$base_build_cwd" \
NOSE_HEAD_BUILD_CWD="$head_build_cwd" \
NOSE_BUILD_COMMAND="$build_command" \
NOSE_COMPARISON_BASE_SHA="$base_sha" \
NOSE_COMPARISON_HEAD_SHA="$head_sha" \
NOSE_BASE_BINARY="$baseline_binary" \
NOSE_HEAD_BINARY="$current_binary" \
python3 - "$artifact_dir/context.json" <<'PY'
import json
import os
import sys
from pathlib import Path

def optional(name):
    return os.environ.get(name) or None

context = {
    "schema": "nose.semantic_regression_context.v1",
    "runner_command": os.environ["NOSE_RUNNER_COMMAND"],
    "comparison_base_sha": os.environ["NOSE_COMPARISON_BASE_SHA"],
    "comparison_head_sha": os.environ["NOSE_COMPARISON_HEAD_SHA"],
    "pull_request_head_sha": optional("NOSE_PR_HEAD_SHA"),
    "github": {
        "event_name": optional("GITHUB_EVENT_NAME"),
        "workflow_sha": optional("GITHUB_SHA"),
    },
    "build": {
        "mode": os.environ["NOSE_BUILD_MODE"],
        "command": optional("NOSE_BUILD_COMMAND"),
        "base_cwd": optional("NOSE_BASE_BUILD_CWD"),
        "head_cwd": optional("NOSE_HEAD_BUILD_CWD"),
        "baseline_binary": os.environ["NOSE_BASE_BINARY"],
        "current_binary": os.environ["NOSE_HEAD_BINARY"],
    },
}
Path(sys.argv[1]).write_text(json.dumps(context, indent=2, sort_keys=True) + "\n")
PY

corpus_args=(
  --corpus-manifest "$(pwd)/bench/goldens/corpus.json"
  --prune-manifest "$(pwd)/bench/labels/prune_manifest.json"
)
if [[ -f "$repos_root/.nose-corpus-state.json" ]]; then
  corpus_args+=(
    --corpus-state "$repos_root/.nose-corpus-state.json"
    --expected-corpus-state "$(pwd)/bench/semantic_regression_corpus.v1.json"
  )
fi
repo_args=()
for repo in "${repos[@]}"; do
  repo_args+=(--repo "$repo")
done

run_harness() {
  local output="$1"
  local base_binary="$2"
  local head_binary="$3"
  local iterations="$4"
  local warmups="$5"
  local source_base_ref="$6"
  local source_head_ref="$7"
  local source_base_sha="$8"
  local source_head_sha="$9"
  shift 9
  python3 scripts/query-regression-harness.py \
    --baseline-binary "$base_binary" \
    --current-binary "$head_binary" \
    --baseline-source-ref "$source_base_ref" \
    --current-source-ref "$source_head_ref" \
    --baseline-source-sha "$source_base_sha" \
    --current-source-sha "$source_head_sha" \
    --repos-root "$repos_root" \
    --iterations "$iterations" \
    --warmups "$warmups" \
    "${corpus_args[@]}" \
    "$@" \
    --output "$output"
}

set +e
python3 scripts/ruby-redefinition-scaling.py \
  --binary "$current_binary" \
  --output "$artifact_dir/ruby-scaling.json"
scaling_rc=$?
set -e

run_harness \
  "$artifact_dir/primary.json" "$baseline_binary" "$current_binary" 2 0 \
  "$base_ref" "$head_ref" "$base_sha" "$head_sha" \
  "${repo_args[@]}"
run_harness \
  "$artifact_dir/primary-control.json" "$baseline_binary" "$baseline_binary" 2 0 \
  "$base_ref" "$base_ref" "$base_sha" "$base_sha" \
  "${repo_args[@]}"

checker_args=(
  "$artifact_dir/primary.json"
  --same-binary-control "$artifact_dir/primary-control.json"
  --expected-drift-manifest "$expected_drift_manifest"
  --require-same-binary-control
  --require-corpus-provenance
  --max-runtime-delta-pct 5
  --min-runtime-delta-ms 5
  --min-focused-iterations 6
  --status-output "$artifact_dir/check-status.json"
  --markdown-output "$artifact_dir/summary.md"
)

set +e
python3 scripts/check-query-regression.py "${checker_args[@]}"
checker_rc=$?
set -e

if [[ $checker_rc -eq 3 ]]; then
  focused_repos=()
  while IFS= read -r repo; do
    focused_repos+=("$repo")
  done < <(
    python3 - "$artifact_dir/check-status.json" <<'PY'
import json
import sys
for repo in json.load(open(sys.argv[1]))["focused_repos"]:
    print(repo)
PY
  )
  focused_repo_args=()
  for repo in "${focused_repos[@]}"; do
    focused_repo_args+=(--repo "$repo")
  done
  run_harness \
    "$artifact_dir/focused.json" "$baseline_binary" "$current_binary" 6 1 \
    "$base_ref" "$head_ref" "$base_sha" "$head_sha" \
    "${focused_repo_args[@]}"
  run_harness \
    "$artifact_dir/focused-control.json" "$baseline_binary" "$baseline_binary" 6 1 \
    "$base_ref" "$base_ref" "$base_sha" "$base_sha" \
    "${focused_repo_args[@]}"
  set +e
  python3 scripts/check-query-regression.py \
    "${checker_args[@]}" \
    --focused-report "$artifact_dir/focused.json" \
    --focused-same-binary-control "$artifact_dir/focused-control.json"
  checker_rc=$?
  set -e
fi

python3 scripts/semantic-regression-summary.py \
  --scaling-report "$artifact_dir/ruby-scaling.json" \
  --summary "$artifact_dir/summary.md" \
  --status "$artifact_dir/check-status.json" \
  --scaling-return-code "$scaling_rc" \
  --checker-return-code "$checker_rc"

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  cat "$artifact_dir/summary.md" >>"$GITHUB_STEP_SUMMARY"
fi
if [[ $checker_rc -ne 0 || $scaling_rc -ne 0 ]]; then
  echo "semantic regression smoke failed; see $artifact_dir" >&2
  exit 1
fi
echo "semantic regression smoke passed; see $artifact_dir"
