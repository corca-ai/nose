#!/usr/bin/env bash
# Reconstruct the benchmark corpus under bench/repos at the exact commits pinned in
# bench/goldens/corpus.json. The goldens reference specific file/line ranges at these
# commits, so the corpus MUST be pinned — never point the detector at a working copy
# that can drift (that silently invalidates every measurement).
#
# bench/repos is gitignored (111M). Run this once on a fresh checkout.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bench/repos

python3 - <<'PY' | while IFS=$'\t' read -r id url commit; do
import json
for r in json.load(open("bench/goldens/corpus.json"))["repositories"]:
    print(f"{r['id']}\t{r.get('url','')}\t{r.get('commit','')}")
PY
  dst="bench/repos/$id"
  if [ -d "$dst/.git" ]; then
    have=$(git -C "$dst" rev-parse HEAD 2>/dev/null || echo none)
    [ "${have:0:12}" = "${commit:0:12}" ] && { echo "ok   $id @ ${commit:0:12}"; continue; }
  fi
  echo "clone $id @ ${commit:0:12}"
  rm -rf "$dst"
  git clone --quiet "$url" "$dst"
  git -C "$dst" checkout --quiet "$commit"
done

# Prune generated and vendored files. These are not source a developer would refactor;
# they flood the corpus (a repo's committed Javadoc HTML, npm/vendored deps, build
# output, minified bundles, generated code) and skew clone-detection measurements toward
# boilerplate. NOTE: radash's `cdn/` bundles are deliberately KEPT — they are a
# cross-language gold target — so `cdn` is intentionally absent from the dir list below.
prune_corpus_dirs() {
  # whole dirs: vendored deps, build output, generated doc sites
  find bench/repos -type d \( \
      -name node_modules -o -name vendor -o -name third_party -o -name third-party \
      -o -name _site -o -name apidocs -o -name dist -o -path '*/website/public' \
    \) -prune -exec rm -rf {} + 2>/dev/null || true
  # minified / bundled files
  find bench/repos -type f \( -name '*.min.js' -o -name '*.min.css' -o -name '*.bundle.js' \) \
    -delete 2>/dev/null || true
}
prune_corpus_dirs
# File-level generated/vendored prune. A Python pass (bench/prune_corpus.py) replaces the
# old bash grep, which omitted .rs/.py (Rust/Python generated files leaked) and could not
# see data-table dirs, vendored stdlib forks, ragel output, or *BSD compat shims. It
# scans ALL source extensions in the banner zone and CROSS-CHECKS every removal against
# the protected gold set (frozen duplicates ∪ worthy labels) — a protected file is never
# removed. Writes bench/labels/prune_manifest.json for audit.
python3 bench/prune_corpus.py --apply
echo "corpus ready under bench/repos (generated/vendored files pruned)"
