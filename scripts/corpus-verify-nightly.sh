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
  --expected-nose-sha256 HEX
                     fail before the run unless the explicit binary has this SHA-256
  --source-commit HEX bind evidence to the producing source commit
  --corpus-manifest FILE
                     pinned repository manifest (default: bench/goldens/corpus.json)
  --repos-root DIR  checked-out pinned corpus root (default: bench/repos)
  --logs-dir DIR    per-repo log/output directory (default: target/corpus-verify-logs)
  --jobs N          repository-level parallelism (default: nproc/sysctl, capped at 6)
  --timeout-seconds N
                     fail a repository that exceeds this wall-clock limit (default: 900)
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
    timeout_seconds="$6"
    repo_id="$7"
    expected_commit="$8"
    repo_dir="$repos_root/$repo_id"
    log="$logs_dir/$repo_id.log"
    status_file="$status_dir/$repo_id.tsv"

    mkdir -p "$logs_dir" "$status_dir"

    if [[ ! -d "$repo_dir" ]]; then
        {
            echo "missing pinned corpus repo: $repo_dir"
            echo "run bench/setup_repos.sh before corpus verify"
        } >"$log"
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$repo_id" "fail" "127" "0" "0" "0" >"$status_file"
        exit 0
    fi

    observed_commit="$(git -C "$repo_dir" rev-parse HEAD 2>/dev/null || true)"
    if [[ "$observed_commit" != "$expected_commit" ]]; then
        {
            echo "pinned corpus repo is at the wrong commit: $repo_dir"
            echo "expected: $expected_commit"
            echo "observed: ${observed_commit:-not-a-git-checkout}"
            echo "run bench/setup_repos.sh before corpus verify"
        } >"$log"
        printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$repo_id" "fail" "126" "0" "0" "0" >"$status_file"
        exit 0
    fi

    command=("$nose" verify "$repo_dir" --max-violations 0)
    if [[ "$nose" == "__cargo_run__" ]]; then
        command=(cargo run --quiet -p nose-cli -- verify "$repo_dir" --max-violations 0)
    fi
    set +e
    python3 - "$timeout_seconds" "$log" "${command[@]}" <<'PY'
import os
import signal
import subprocess
import sys

timeout = int(sys.argv[1])
log_name = sys.argv[2]
command = sys.argv[3:]
with open(log_name, "wb") as log:
    process = subprocess.Popen(
        command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True
    )
    try:
        code = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        log.write(f"\ncorpus verify timeout after {timeout} seconds\n".encode())
        code = 124
raise SystemExit(code)
PY
    code=$?
    set -e

    false_merges="$(parse_count '\[!\] ([0-9]+) VIOLATION\(S\)' "$log")"
    canon_changes="$(parse_count '\[!\] ([0-9]+) unit\(s\) whose behavior CHANGED' "$log")"
    advisory="$(parse_count 'advisory \([^)]*disagreements[^)]*\): ([0-9]+)' "$log")"
    status="pass"
    if [[ "$code" -ne 0 || "$false_merges" -ne 0 || "$canon_changes" -ne 0 ]]; then
        status="fail"
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$repo_id" "$status" "$code" "$false_merges" "$canon_changes" "$advisory" \
        >"$status_file"
    exit 0
fi

self_test() {
    local script_path tmp fake_nose code
    script_path="$(pwd)/${BASH_SOURCE[0]}"
    tmp="$(mktemp -d "${TMPDIR:-/tmp}/nose-corpus-verify-test.XXXXXX")"
    trap 'rm -rf "$tmp"' RETURN
    mkdir -p "$tmp/repos/arrow" "$tmp/repos/black"
    for repo in arrow black; do
        git -C "$tmp/repos/$repo" init -q
        printf 'fixture\n' >"$tmp/repos/$repo/source.txt"
        git -C "$tmp/repos/$repo" add source.txt
        git -C "$tmp/repos/$repo" \
            -c user.name='nose corpus self-test' \
            -c user.email='nose-corpus-self-test@example.invalid' \
            commit -q -m fixture
    done
    python3 - "$tmp/repos" "$tmp/corpus.json" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
repositories = []
for repo in ("arrow", "black"):
    commit = subprocess.check_output(
        ["git", "-C", str(root / repo), "rev-parse", "HEAD"], text=True
    ).strip()
    repositories.append({"id": repo, "commit": commit})
Path(sys.argv[2]).write_text(json.dumps({"repositories": repositories}) + "\n")
PY
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
  advisory (symbolic-trace disagreements - inspect, not gated): 2

GATE: 0 <= 0 false merges - OK
LOG
    ;;
  black)
    if [[ "${NOSE_CORPUS_SELF_TEST_SLEEP:-0}" == "1" ]]; then
      sleep 2
    fi
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
        --expected-nose-sha256 0000000000000000000000000000000000000000000000000000000000000000 \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/hash-logs" \
        >"$tmp/hash-out" 2>&1
    code=$?
    set -e
    [[ "$code" -eq 2 ]] || {
        cat "$tmp/hash-out" >&2
        echo "self-test expected binary hash failure, got exit $code" >&2
        exit 1
    }
    grep -q 'binary SHA-256 mismatch' "$tmp/hash-out"

    set +e
    "$script_path" \
        --nose "$fake_nose" \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/logs" \
        --jobs 2 \
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
    python3 - "$tmp/logs/evidence.json" <<'PY'
import json
import sys

evidence = json.load(open(sys.argv[1], encoding="utf-8"))
assert evidence["complete"] is True
assert evidence["full_corpus"] is False
assert all(
    (row["status"] == "pass")
    == (row["exit_code"] == 0 and row["false_merges"] == 0 and row["canon_changes"] == 0)
    for row in evidence["results"]
)
PY
    cp -R "$tmp/logs" "$tmp/logs-first"
    set +e
    "$script_path" \
        --nose "$fake_nose" \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/logs" \
        --jobs 1 \
        >"$tmp/repeat-out" 2>&1
    code=$?
    set -e
    [[ "$code" -eq 1 ]] || {
        cat "$tmp/repeat-out" >&2
        echo "self-test expected repeated aggregate failure, got exit $code" >&2
        exit 1
    }
    diff -ru "$tmp/logs-first" "$tmp/logs"

    rm -rf "$tmp/repos/black"
    set +e
    "$script_path" \
        --nose "$fake_nose" \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/missing-logs" \
        --repo black \
        >"$tmp/missing-out" 2>&1
    code=$?
    set -e
    [[ "$code" -eq 1 ]] || {
        cat "$tmp/missing-out" >&2
        echo "self-test expected missing repository failure, got exit $code" >&2
        exit 1
    }
    grep -q 'missing pinned corpus repo' "$tmp/missing-logs/black.log"

    git -C "$tmp/repos" clone -q "$tmp/repos/arrow" black
    black_commit="$(git -C "$tmp/repos/black" rev-parse HEAD)"
    python3 - "$tmp/corpus.json" "$black_commit" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
value = json.loads(path.read_text())
for repository in value["repositories"]:
    if repository["id"] == "black":
        repository["commit"] = sys.argv[2]
path.write_text(json.dumps(value) + "\n")
PY
    set +e
    NOSE_CORPUS_SELF_TEST_SLEEP=1 "$script_path" \
        --nose "$fake_nose" \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/timeout-logs" \
        --timeout-seconds 1 \
        --repo black \
        >"$tmp/timeout-out" 2>&1
    code=$?
    set -e
    [[ "$code" -eq 1 ]] || {
        cat "$tmp/timeout-out" >&2
        echo "self-test expected timeout failure, got exit $code" >&2
        exit 1
    }
    grep -q 'timeout after 1 seconds' "$tmp/timeout-logs/black.log"

    rm -rf "$tmp/repos/black"
    git -C "$tmp/repos" clone -q "$tmp/repos/arrow" black
    git -C "$tmp/repos/arrow" \
        -c user.name='nose corpus self-test' \
        -c user.email='nose-corpus-self-test@example.invalid' \
        commit -q --allow-empty -m drift
    set +e
    "$script_path" \
        --nose "$fake_nose" \
        --corpus-manifest "$tmp/corpus.json" \
        --repos-root "$tmp/repos" \
        --logs-dir "$tmp/pin-logs" \
        --jobs 1 \
        --repo arrow \
        >"$tmp/pin-out" 2>&1
    code=$?
    set -e
    [[ "$code" -eq 1 ]] || {
        cat "$tmp/pin-out" >&2
        echo "self-test expected changed pin failure, got exit $code" >&2
        exit 1
    }
    grep -q 'wrong commit' "$tmp/pin-logs/arrow.log"
    echo "ok corpus verify runner self-test"
}

nose=""
expected_nose_sha256=""
source_commit=""
corpus_manifest="bench/goldens/corpus.json"
repos_root="bench/repos"
logs_dir="target/corpus-verify-logs"
jobs="${NOSE_CORPUS_VERIFY_JOBS:-$(default_jobs)}"
timeout_seconds="${NOSE_CORPUS_VERIFY_TIMEOUT_SECONDS:-900}"
repo_filters=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --nose)
            nose="${2:?missing value for --nose}"
            shift 2
            ;;
        --expected-nose-sha256)
            expected_nose_sha256="${2:?missing value for --expected-nose-sha256}"
            shift 2
            ;;
        --source-commit)
            source_commit="${2:?missing value for --source-commit}"
            shift 2
            ;;
        --corpus-manifest)
            corpus_manifest="${2:?missing value for --corpus-manifest}"
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
        --timeout-seconds)
            timeout_seconds="${2:?missing value for --timeout-seconds}"
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
[[ "$timeout_seconds" =~ ^[0-9]+$ && "$timeout_seconds" -gt 0 ]] || {
    echo "--timeout-seconds must be a positive integer, got: $timeout_seconds" >&2
    exit 2
}
[[ -z "$expected_nose_sha256" || "$expected_nose_sha256" =~ ^[0-9a-f]{64}$ ]] || {
    echo "--expected-nose-sha256 must be 64 lowercase hex characters" >&2
    exit 2
}
[[ -z "$source_commit" || "$source_commit" =~ ^[0-9a-f]{40}$ ]] || {
    echo "--source-commit must be 40 lowercase hex characters" >&2
    exit 2
}
if [[ -n "$source_commit" ]]; then
    observed_source_commit="$(git rev-parse HEAD 2>/dev/null || true)"
    if [[ "$observed_source_commit" != "$source_commit" ]]; then
        echo "source commit mismatch" >&2
        echo "expected: $source_commit" >&2
        echo "observed: ${observed_source_commit:-not-a-git-checkout}" >&2
        exit 2
    fi
fi
[[ -f "$corpus_manifest" ]] || {
    echo "corpus manifest does not exist: $corpus_manifest" >&2
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

nose_sha256=""
nose_version=""
if [[ "$nose" != "__cargo_run__" ]]; then
    nose_sha256="$(python3 - "$nose" <<'PY'
import hashlib
import sys

digest = hashlib.sha256()
with open(sys.argv[1], "rb") as handle:
    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        digest.update(chunk)
print(digest.hexdigest())
PY
)"
    nose_version="$("$nose" --version 2>/dev/null || true)"
fi
if [[ -n "$expected_nose_sha256" && "$nose_sha256" != "$expected_nose_sha256" ]]; then
    echo "nose binary SHA-256 mismatch" >&2
    echo "expected: $expected_nose_sha256" >&2
    echo "observed: ${nose_sha256:-unavailable-for-cargo-run}" >&2
    exit 2
fi

rm -rf "$logs_dir"
mkdir -p "$logs_dir/status"

repo_list="$logs_dir/repos.txt"
python3 - "$corpus_manifest" "${repo_filters[@]}" >"$repo_list" <<'PY'
import json
import sys

filters = set(sys.argv[2:])
with open(sys.argv[1], encoding="utf-8") as handle:
    repositories = json.load(handle)["repositories"]
selected = [repo for repo in repositories if not filters or repo["id"] in filters]
unknown = sorted(filters - {repo["id"] for repo in repositories})
if unknown:
    raise SystemExit(f"unknown corpus repo id(s): {', '.join(unknown)}")
for repo in selected:
    print(f"{repo['id']}\t{repo['commit']}")
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
    echo "using nose binary: $nose (sha256=$nose_sha256)"
fi

prune_verified="false"
default_corpus_manifest="$(pwd)/bench/goldens/corpus.json"
corpus_manifest_absolute="$(cd "$(dirname "$corpus_manifest")" && pwd)/$(basename "$corpus_manifest")"
if [[ "${#repo_filters[@]}" -eq 0 && "$corpus_manifest_absolute" == "$default_corpus_manifest" ]]; then
    python3 bench/prune_corpus.py --repos-root "$repos_root" --check-manifest
    prune_verified="true"
fi

xargs -n 2 -P "$jobs" "$0" __run_repo "$nose" "$repos_root" "$logs_dir" "$logs_dir/status" \
    "$timeout_seconds" \
    <"$repo_list"
if [[ "$prune_verified" == "true" ]]; then
    python3 bench/prune_corpus.py --repos-root "$repos_root" --check-manifest
fi

summary_tsv="$logs_dir/summary.tsv"
{
    printf 'repo\tstatus\texit_code\tfalse_merges\tcanon_changes\tadvisory\n'
    LC_ALL=C sort "$logs_dir"/status/*.tsv
} >"$summary_tsv"

totals="$(
    awk -F '\t' 'NR > 1 {
        repos += 1
        if ($2 != "pass") failed += 1
        false_merges += $4
        canon_changes += $5
        advisory += $6
    }
    END {
        printf "%d\t%d\t%d\t%d\t%d", repos, failed, false_merges, canon_changes, advisory
    }' "$summary_tsv"
)"
IFS=$'\t' read -r total_repos failed_repos total_false total_canon total_advisory <<<"$totals"

python3 - \
    "$logs_dir/evidence.json" \
    "$repo_list" \
    "$repos_root" \
    "$nose_sha256" \
    "$nose_version" \
    "$source_commit" \
    "$corpus_manifest" \
    "bench/labels/prune_manifest.json" \
    "$prune_verified" \
    "$summary_tsv" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


(
    output_name, repo_list_name, repos_root_name, nose_sha256, nose_version, source_commit,
    corpus_manifest_name, prune_manifest_name, prune_verified, summary_name,
) = sys.argv[1:]
repo_list = Path(repo_list_name)
repos_root = Path(repos_root_name)
corpus_manifest = Path(corpus_manifest_name)
prune_manifest = Path(prune_manifest_name)
summary = Path(summary_name)
repositories = []
for line in repo_list.read_text().splitlines():
    repo_id, expected_commit = line.split("\t")
    result = subprocess.run(
        ["git", "-C", str(repos_root / repo_id), "rev-parse", "HEAD"],
        text=True, capture_output=True, check=False,
    )
    observed_commit = result.stdout.strip() if result.returncode == 0 else None
    repositories.append({
        "id": repo_id,
        "expected_commit": expected_commit,
        "observed_commit": observed_commit,
    })

summary_rows = [line.split("\t") for line in summary.read_text().splitlines()[1:] if line]
canonical = ("\n".join(sorted("\t".join(row) for row in summary_rows)) + "\n").encode()
full_corpus = prune_verified == "true"
prune = json.loads(prune_manifest.read_text()) if full_corpus else None
results = [
    {
        "id": row[0],
        "status": row[1],
        "exit_code": int(row[2]),
        "false_merges": int(row[3]),
        "canon_changes": int(row[4]),
        "advisory": int(row[5]),
    }
    for row in sorted(summary_rows)
]
evidence = {
    "schema": "nose-corpus-verify-evidence/v2",
    # Reaching this writer means every selected repository produced exactly one status row.
    # A failed repository is still a complete shard and remains red through its row/totals.
    "complete": True,
    "full_corpus": full_corpus,
    "nose": {"sha256": nose_sha256 or None, "version": nose_version or None},
    "corpus_manifest_sha256": sha256_file(corpus_manifest),
    "prune_manifest_sha256": sha256_file(prune_manifest) if full_corpus else None,
    "pruned_corpus_digest_sha256": (
        prune["corpus_digest_after_prune"]["hex"] if prune is not None else None
    ),
    "repositories": sorted(repositories, key=lambda row: row["id"]),
    "results": results,
    "totals": {
        "repositories": len(results),
        "failed_repositories": sum(row["status"] != "pass" for row in results),
        "false_merges": sum(row["false_merges"] for row in results),
        "canon_changes": sum(row["canon_changes"] for row in results),
        "advisory": sum(row["advisory"] for row in results),
    },
    "summary_sha256": sha256_file(summary),
    "canonical_result_sha256": hashlib.sha256(canonical).hexdigest(),
}
if source_commit:
    evidence["source_commit"] = source_commit
Path(output_name).write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
PY

summary_md="$logs_dir/summary.md"
{
    echo "## Corpus verify"
    echo
    echo "- repositories: $total_repos"
    echo "- failed repos: $failed_repos"
    echo "- hard false merges: $total_false"
    echo "- canon-preservation changes: $total_canon"
    echo "- advisory symbolic-trace disagreements: $total_advisory"
    echo
    echo "| repo | status | false merges | canon changes | advisory |"
    echo "|---|---:|---:|---:|---:|"
    awk -F '\t' 'NR > 1 {
        printf "| %s | %s | %s | %s | %s |\n", $1, $2, $4, $5, $6
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
