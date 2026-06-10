#!/usr/bin/env bash
# Run the pinned benchmark corpus through `nose verify` one repository at a time.
# The nightly GitHub Action uses this as the zero-false-merge tripwire while keeping
# per-repo logs for triage.
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
    cat <<'EOF'
usage: ./scripts/corpus-verify-nightly.sh [options]

Options:
  --nose PATH        nose binary to run (default: target/release/nose, then cargo run)
  --repos-root DIR  checked-out pinned corpus root (default: bench/repos)
  --logs-dir DIR    per-repo log/output directory (default: target/corpus-verify-logs)
  --jobs N          repository-level parallelism (default: nproc/sysctl, capped at 6)
  --repo ID         run only one corpus repo id; may be repeated
  --self-test       run a fake-nose harness that proves pass/fail/advisory aggregation
  -h, --help        show this help
EOF
}

default_jobs() {
    local cores
    cores="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)"
    if [[ "$cores" -gt 6 ]]; then
        echo 6
    elif [[ "$cores" -lt 1 ]]; then
        echo 1
    else
        echo "$cores"
    fi
}

parse_count() {
    local pattern="$1"
    local file="$2"
    python3 - "$pattern" "$file" <<'PY'
import re
import sys

pattern = re.compile(sys.argv[1])
path = sys.argv[2]
count = 0
try:
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            match = pattern.search(line)
            if match:
                count = int(match.group(1))
except FileNotFoundError:
    pass
print(count)
PY
}

if [[ "${1:-}" == "__run_repo" ]]; then
    nose="$2"
    repos_root="$3"
    logs_dir="$4"
    status_dir="$5"
    repo_id="$6"
    repo_dir="$repos_root/$repo_id"
    log="$logs_dir/$repo_id.log"
    status_file="$status_dir/$repo_id.tsv"

    started="$(date +%s)"
    mkdir -p "$logs_dir" "$status_dir"

    if [[ ! -d "$repo_dir" ]]; then
        {
            echo "missing pinned corpus repo: $repo_dir"
            echo "run bench/setup_repos.sh before corpus verify"
        } >"$log"
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$repo_id" "fail" "127" "0" "0" "0" "0" >"$status_file"
        exit 0
    fi

    set +e
    if [[ "$nose" == "__cargo_run__" ]]; then
        cargo run --quiet -p nose-cli -- verify "$repo_dir" --max-violations 0 >"$log" 2>&1
    else
        "$nose" verify "$repo_dir" --max-violations 0 >"$log" 2>&1
    fi
    code=$?
    set -e

    finished="$(date +%s)"
    elapsed=$((finished - started))
    false_merges="$(parse_count '\[!\] ([0-9]+) VIOLATION\(S\)' "$log")"
    canon_changes="$(parse_count '\[!\] ([0-9]+) unit\(s\) whose behavior CHANGED' "$log")"
    advisory="$(parse_count 'advisory \(symbolic-trace disagreements.*\): ([0-9]+)' "$log")"
    status="pass"
    if [[ "$code" -ne 0 || "$false_merges" -ne 0 || "$canon_changes" -ne 0 ]]; then
        status="fail"
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$repo_id" "$status" "$code" "$false_merges" "$canon_changes" "$advisory" "$elapsed" \
        >"$status_file"
    exit 0
fi

self_test() {
    local script_path tmp fake_nose code
    script_path="$(pwd)/${BASH_SOURCE[0]}"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/nose-corpus-verify-test.XXXXXX")"
    trap 'rm -rf "$tmp"' RETURN
    mkdir -p "$tmp/repos/arrow" "$tmp/repos/black"
    fake_nose="$tmp/nose"
    cat >"$fake_nose" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
repo="${2##*/}"
case "$repo" in
  arrow)
    cat <<'LOG'
=== value-graph oracle (soundness + completeness) ===

CANON PRESERVATION - normalization preserves behavior:
  PRESERVED: every canon-changed unit computes the same thing

SOUNDNESS - fingerprint-equal => behavior-equal (exact claim surface):
  fingerprint groups (>=2): 1
  SOUND: no false merges
  advisory (symbolic-trace disagreements - review, not gated): 2

GATE: 0 <= 0 false merges - OK
LOG
    ;;
  black)
    cat <<'LOG'
=== value-graph oracle (soundness + completeness) ===

CANON PRESERVATION - normalization preserves behavior:
  [!] 1 unit(s) whose behavior CHANGED under canonicalization:
    black/f.py:1-3

SOUNDNESS - fingerprint-equal => behavior-equal (exact claim surface):
  fingerprint groups (>=2): 1
  SOUND: no false merges
LOG
    exit 1
    ;;
esac
EOF
    chmod +x "$fake_nose"

    set +e
    "$script_path" \
        --nose "$fake_nose" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/logs" \
        --jobs 2 \
        --repo arrow \
        --repo black \
        >"$tmp/out" 2>&1
    code=$?
    set -e

    [[ "$code" -eq 1 ]] || {
        cat "$tmp/out" >&2
        echo "self-test expected aggregate failure, got exit $code" >&2
        exit 1
    }
    grep -q 'failed repos: 1' "$tmp/out"
    grep -q 'canon-preservation changes: 1' "$tmp/out"
    grep -q 'advisory symbolic-trace disagreements: 2' "$tmp/out"
    grep -q 'black' "$tmp/logs/summary.md"
    grep -q 'arrow' "$tmp/logs/summary.md"
    echo "ok corpus verify runner self-test"
}

nose=""
repos_root="bench/repos"
logs_dir="target/corpus-verify-logs"
jobs="${NOSE_CORPUS_VERIFY_JOBS:-$(default_jobs)}"
repo_filters=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --nose)
            nose="${2:?missing value for --nose}"
            shift 2
            ;;
        --repos-root)
            repos_root="${2:?missing value for --repos-root}"
            shift 2
            ;;
        --logs-dir)
            logs_dir="${2:?missing value for --logs-dir}"
            shift 2
            ;;
        --jobs)
            jobs="${2:?missing value for --jobs}"
            shift 2
            ;;
        --repo)
            repo_filters+=("${2:?missing value for --repo}")
            shift 2
            ;;
        --self-test)
            self_test
            exit 0
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

[[ "$jobs" =~ ^[0-9]+$ && "$jobs" -gt 0 ]] || {
    echo "--jobs must be a positive integer, got: $jobs" >&2
    exit 2
}

if [[ -z "$nose" ]]; then
    if [[ -x target/release/nose ]]; then
        nose="target/release/nose"
    else
        nose="__cargo_run__"
    fi
fi
if [[ "$nose" != "__cargo_run__" && ! -x "$nose" ]]; then
    echo "nose binary is not executable: $nose" >&2
    exit 2
fi

rm -rf "$logs_dir"
mkdir -p "$logs_dir/status"

repo_list="$logs_dir/repos.txt"
python3 - "${repo_filters[@]}" >"$repo_list" <<'PY'
import json
import sys

filters = set(sys.argv[1:])
with open("bench/goldens/corpus.json", encoding="utf-8") as handle:
    repositories = json.load(handle)["repositories"]
ids = [repo["id"] for repo in repositories if not filters or repo["id"] in filters]
unknown = sorted(filters - {repo["id"] for repo in repositories})
if unknown:
    raise SystemExit(f"unknown corpus repo id(s): {', '.join(unknown)}")
for repo_id in ids:
    print(repo_id)
PY

repo_count="$(wc -l <"$repo_list" | tr -d ' ')"
if [[ "$repo_count" -eq 0 ]]; then
    echo "no corpus repositories selected" >&2
    exit 2
fi

echo "corpus verify: $repo_count repos, jobs=$jobs, logs=$logs_dir"
if [[ "$nose" == "__cargo_run__" ]]; then
    echo "using cargo run -p nose-cli"
else
    echo "using nose binary: $nose"
fi

xargs -n 1 -P "$jobs" "$0" __run_repo "$nose" "$repos_root" "$logs_dir" "$logs_dir/status" \
    <"$repo_list"

summary_tsv="$logs_dir/summary.tsv"
{
    printf 'repo\tstatus\texit_code\tfalse_merges\tcanon_changes\tadvisory\tseconds\n'
    LC_ALL=C sort "$logs_dir"/status/*.tsv
} >"$summary_tsv"

totals="$(
    awk -F '\t' 'NR > 1 {
        repos += 1
        if ($2 != "pass") failed += 1
        false_merges += $4
        canon_changes += $5
        advisory += $6
        seconds += $7
    }
    END {
        printf "%d\t%d\t%d\t%d\t%d\t%d", repos, failed, false_merges, canon_changes, advisory, seconds
    }' "$summary_tsv"
)"
IFS=$'\t' read -r total_repos failed_repos total_false total_canon total_advisory total_seconds <<<"$totals"

summary_md="$logs_dir/summary.md"
{
    echo "## Corpus verify"
    echo
    echo "- repositories: $total_repos"
    echo "- failed repos: $failed_repos"
    echo "- hard false merges: $total_false"
    echo "- canon-preservation changes: $total_canon"
    echo "- advisory symbolic-trace disagreements: $total_advisory"
    echo "- summed repo seconds: $total_seconds"
    echo
    echo "| repo | status | false merges | canon changes | advisory | seconds |"
    echo "|---|---:|---:|---:|---:|---:|"
    awk -F '\t' 'NR > 1 {
        printf "| %s | %s | %s | %s | %s | %s |\n", $1, $2, $4, $5, $6, $7
    }' "$summary_tsv"
} >"$summary_md"

cat "$summary_md"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    cat "$summary_md" >>"$GITHUB_STEP_SUMMARY"
fi

if [[ "$failed_repos" -ne 0 ]]; then
    echo
    echo "failed repo logs:"
    awk -F '\t' -v logs="$logs_dir" 'NR > 1 && $2 != "pass" {
        printf "  %s -> %s/%s.log\n", $1, logs, $1
    }' "$summary_tsv"
    exit 1
fi

echo
echo "corpus verify gate passed"
