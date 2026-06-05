#!/usr/bin/env bash
# Clone + mine a balanced corpus of repos in parallel. Writes per-repo JSONL events
# to $WORK and a combined all-events.jsonl. See docs/hazard-benchmark.md.
set -u
NOSE="${NOSE:-/Users/ak/prjs/cc/nose/target/release/nose}"
MINE="${MINE:-/Users/ak/prjs/cc/nose-worktrees/hazard-ranking/eval/hazard/mine.py}"
WORK="${WORK:-/tmp/hazard-mine}"
MONTHS="${MONTHS:-60}"
mkdir -p "$WORK"

# name url stratum   (whole-repo scan; default branch auto-detected)
REPOS=(
  "django|https://github.com/django/django.git|S"
  "vue-core|https://github.com/vuejs/core.git|S"
  "tokio|https://github.com/tokio-rs/tokio.git|S"
  "redis|https://github.com/redis/redis.git|S"
  "hugo|https://github.com/gohugoio/hugo.git|S"
  "thrift|https://github.com/apache/thrift.git|X"
)

clone_one() {
  local name="$1" url="$2" dir="$WORK/$name"
  if [ ! -d "$dir/.git" ]; then
    rm -rf "$dir"
    git clone -q --no-single-branch "$url" "$dir" 2>"$WORK/$name.clone.log" || { echo "[clone FAIL] $name"; return 1; }
  fi
  echo "[cloned] $name"
}

mine_one() {
  local name="$1" stratum="$2" dir="$WORK/$name"
  # reset to the remote default branch (a prior run may have left detached HEAD)
  local branch; branch=$(git -C "$dir" symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')
  [ -z "$branch" ] && branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD 2>/dev/null)
  git -C "$dir" checkout -q "$branch" 2>/dev/null
  python3 "$MINE" --repo "$dir" --nose "$NOSE" --branch "$branch" \
    --max-months "$MONTHS" --out "$WORK/$name-events.jsonl" \
    > "$WORK/$name.mine.log" 2>&1
  # tag stratum onto each record
  python3 - "$WORK/$name-events.jsonl" "$stratum" <<'PY'
import json,sys
path,strat=sys.argv[1],sys.argv[2]
try: lines=open(path).read().splitlines()
except FileNotFoundError: lines=[]
with open(path,"w") as f:
    for l in lines:
        d=json.loads(l); d["stratum"]=strat; f.write(json.dumps(d)+"\n")
PY
  echo "[mined] $name ($(wc -l < "$WORK/$name-events.jsonl" 2>/dev/null || echo 0) events)"
}

echo "=== cloning ${#REPOS[@]} repos in parallel ==="
for spec in "${REPOS[@]}"; do IFS='|' read -r name url stratum <<< "$spec"; clone_one "$name" "$url" & done
wait

echo "=== mining ${#REPOS[@]} repos in parallel (months=$MONTHS) ==="
for spec in "${REPOS[@]}"; do IFS='|' read -r name url stratum <<< "$spec"; mine_one "$name" "$stratum" & done
wait

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
