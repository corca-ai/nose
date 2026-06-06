#!/usr/bin/env bash
# Clone + mine a balanced corpus of repos in parallel. Writes per-repo JSONL events
# to $WORK and a combined all-events.jsonl. Incremental: skips repos already mined.
# See docs/hazard-benchmark.md and docs/hazard-release-checklist.md.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
NOSE="${NOSE:-$REPO_ROOT/target/release/nose}"
MINE="${MINE:-$SCRIPT_DIR/mine.py}"
WORK="${WORK:-/tmp/hazard-mine}"
MONTHS="${MONTHS:-60}"
mkdir -p "$WORK"

# name|url|stratum|subdir   (subdir empty = whole repo; huge monorepos scope to a dir)
REPOS=(
  "django|https://github.com/django/django.git|S|"
  "vue-core|https://github.com/vuejs/core.git|S|"
  "tokio|https://github.com/tokio-rs/tokio.git|S|"
  "redis|https://github.com/redis/redis.git|S|"
  "hugo|https://github.com/gohugoio/hugo.git|S|"
  "thrift|https://github.com/apache/thrift.git|X|"
  "express|https://github.com/expressjs/express.git|S|"
  "kafka|https://github.com/apache/kafka.git|S|clients"
  "pandas|https://github.com/pandas-dev/pandas.git|S|pandas"
  "grpc|https://github.com/grpc/grpc.git|X|src"
  "ripgrep|https://github.com/BurntSushi/ripgrep.git|S|"
  "terraform|https://github.com/hashicorp/terraform.git|S|internal"
)

clone_one() {
  local name="$1" url="$2" dir="$WORK/$name"
  [ -s "$WORK/$name-events.jsonl" ] && { echo "[skip-clone] $name (mined)"; return; }
  if [ ! -d "$dir/.git" ]; then
    rm -rf "$dir"
    git clone -q --no-single-branch "$url" "$dir" 2>"$WORK/$name.clone.log" || { echo "[clone FAIL] $name"; return 1; }
  fi
  echo "[cloned] $name"
}

mine_one() {
  local name="$1" stratum="$2" subdir="$3" dir="$WORK/$name"
  [ -s "$WORK/$name-events.jsonl" ] && { echo "[skip-mine] $name (exists)"; return; }
  # reset to the remote default branch (a prior run may have left detached HEAD)
  local branch; branch=$(git -C "$dir" symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')
  [ -z "$branch" ] && branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD 2>/dev/null)
  git -C "$dir" checkout -q "$branch" 2>/dev/null
  python3 "$MINE" --repo "$dir" --nose "$NOSE" --branch "$branch" --subdir "$subdir" \
    --max-months "$MONTHS" --out "$WORK/$name-events.jsonl" \
    --g1-evidence "$WORK/$name-g1ev.jsonl" \
    > "$WORK/$name.mine.log" 2>&1
  [ -f "$WORK/$name-events.jsonl" ] || { echo "[mine FAIL] $name (no events file)"; return 1; }
  python3 - "$WORK/$name-events.jsonl" "$stratum" <<'PY'
import json,sys,os
path,strat=sys.argv[1],sys.argv[2]
lines=open(path).read().splitlines() if os.path.exists(path) else []
with open(path,"w") as f:
    for l in lines:
        d=json.loads(l); d["stratum"]=strat; f.write(json.dumps(d)+"\n")
PY
  echo "[mined] $name ($(wc -l < "$WORK/$name-events.jsonl" 2>/dev/null || echo 0) events)"
}

echo "=== cloning (parallel; skips already-mined) ==="
pids=()
for spec in "${REPOS[@]}"; do IFS='|' read -r name url stratum subdir <<< "$spec"; clone_one "$name" "$url" & pids+=("$!"); done
for pid in "${pids[@]}"; do wait "$pid"; done

echo "=== mining (parallel; months=$MONTHS) ==="
pids=()
for spec in "${REPOS[@]}"; do IFS='|' read -r name url stratum subdir <<< "$spec"; mine_one "$name" "$stratum" "$subdir" & pids+=("$!"); done
for pid in "${pids[@]}"; do wait "$pid"; done

cat "$WORK"/*-events.jsonl > "$WORK/all-events.jsonl" 2>/dev/null
echo "=== corpus totals ==="
python3 - "$WORK/all-events.jsonl" <<'PY'
import json,sys
from collections import Counter
by_label=Counter(); by_strat=Counter()
for l in open(sys.argv[1]):
    d=json.loads(l); by_label[d["label"]]+=1; by_strat[(d["stratum"],d["label"])]+=1
print("labels:", dict(by_label), "total", sum(by_label.values()))
print("by stratum:", dict(by_strat))
PY
echo "=== DONE ==="
