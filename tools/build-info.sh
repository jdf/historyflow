#!/usr/bin/env bash
# Stamp dist with the build identifier: a commit id and the build date.
# Writes only on change, so dependents don't rebuild.
#
# The id has one job — let a bug report name the source that built the
# dist it came from — so it names the newest ancestor of the working copy
# that has been pushed, not jj's @. An empty @ on top of a pushed commit
# builds byte-identical content, and stamping its local-only id leaves
# reports pointing at a commit no reader can fetch. Builds that carry
# anything beyond the pushed commit — unpushed commits, uncommitted
# edits, untracked sources (cargo picks up any new file) — get a +local suffix,
# which is the honest answer: no public commit describes this dist.
#
# Every consumer treats the id as free text (console line, Feedback
# dialog footer, export metadata, deploy's site-repo commit message).
#
#   tools/build-info.sh <output.js>
set -euo pipefail
OUT="$1"

# Resolves a revset to a short commit id, empty if it selects nothing.
rev() { jj log --no-graph -r "$1" -T "${2:-commit_id.short(12)}" 2>/dev/null || true; }

LOCAL=""
if command -v jj >/dev/null 2>&1 && jj root >/dev/null 2>&1; then
  # remote_bookmarks() leaves out a colocated repo's own git refs (those
  # sit on a remote named "git", which has to be asked for by name), so
  # moving a bookmark locally doesn't pass for published — only a push to
  # a real remote does.
  BASE='heads(::@ & ::remote_bookmarks())'
  ID=$(rev "$BASE")
  if [ -n "$ID" ]; then
    # Any non-empty commit after the pushed base, @ (where uncommitted
    # edits live) included.
    if [ -n "$(rev "$BASE..@ & ~empty()" '"x"')" ]; then
      LOCAL="+local"
    fi
  else
    ID=$(rev @)  # nothing pushed yet; @ is all there is, and it's local
    LOCAL="+local"
  fi
else
  # Plain git checkout — this is the CI path, where HEAD is the pushed
  # commit and the tree is clean, so the id comes out bare.
  ID=$(git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)
  if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    LOCAL="+local"
  fi
fi
DATE=$(date -u +%Y-%m-%d)

TMP="$OUT.tmp"
printf "window.__historyflowBuild = { id: '%s', date: '%s' };\n" "$ID$LOCAL" "$DATE" > "$TMP"
if [ -f "$OUT" ] && cmp -s "$TMP" "$OUT"; then
  rm "$TMP"
else
  mv "$TMP" "$OUT"
fi
