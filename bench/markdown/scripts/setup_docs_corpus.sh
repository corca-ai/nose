#!/usr/bin/env bash
# Deterministically curate a multi-domain Markdown corpus for the docs golden set
# (bench/markdown/corpus-docs/) from the vendored bench/repos. The result is COMMITTED, so the
# golden's line-spans are frozen and reproducible without bench/repos — this script only documents
# provenance / how to regenerate. Run from the repo root after `bench/setup_repos.sh`.
#
# Genres (multi-domain, on purpose — the CoC golden is single-genre boilerplate):
#   CLI reference (curl options), function/API reference (hugo functions), guides + news (jekyll),
#   framework docs (prettier, trpc), and cross-repo README boilerplate.
set -euo pipefail
REPOS="${REPOS:-$(pwd)/bench/repos}"
DEST="$(pwd)/bench/markdown/corpus-docs"
rm -rf "$DEST"; mkdir -p "$DEST"

# copy_slice <relative-glob-root> <repo> <count> <tag>
copy_slice() {
  local root="$1" repo="$2" n="$3" tag="$4" i=0
  while IFS= read -r f; do
    [ -z "$f" ] && continue
    local rel="${f#"$REPOS"/}"
    local flat="${rel//\//__}"
    cp "$f" "$DEST/${tag}__${flat}"
    i=$((i+1)); [ "$i" -ge "$n" ] && break
  done < <(find "$REPOS/$root" -name '*.md' 2>/dev/null | sort)
}

copy_slice "curl/docs/cmdline-opts" curl       45 cli
copy_slice "hugo/docs/content/en/functions" hugo 30 fnref
copy_slice "jekyll/docs"           jekyll     28 guide
copy_slice "prettier/docs"         prettier   22 fwdoc
copy_slice "trpc/www/docs"         trpc       25 fwdoc
[ "$(ls "$DEST" | grep -c '^fwdoc__trpc' || true)" -gt 0 ] || copy_slice "trpc" trpc 25 fwdoc

# cross-repo README boilerplate (cross-project near-dups: install snippets, badges, sections)
i=0
while IFS= read -r f; do
  rel="${f#"$REPOS"/}"; flat="${rel//\//__}"
  cp "$f" "$DEST/readme__${flat}"
  i=$((i+1)); [ "$i" -ge 15 ] && break
done < <(find "$REPOS" -maxdepth 2 -iname 'README.md' 2>/dev/null | sort)

echo "corpus-docs: $(ls "$DEST" | wc -l | tr -d ' ') files"
ls "$DEST" | sed -E 's/__.*//' | sort | uniq -c
