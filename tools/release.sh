#!/usr/bin/env bash
# Releases the historyflow command: tags this repository and asks the
# public distribution repository (jdf/historyflow-cli) to build that tag
# and publish it as a GitHub release there, then asks the Homebrew tap
# (jdf/homebrew-historyflow) to rewrite its formula for that release.
# It waits for the build, which takes a few minutes.
#
#   tools/release.sh 0.2.0
#
# The version must be the one in Cargo.toml, and the commit released is
# main as pushed: bump the version, commit, push, then run this.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: tools/release.sh <version>}"
PUBLIC=jdf/historyflow-cli
TAP=jdf/homebrew-historyflow

IN_CARGO=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
if [ "$IN_CARGO" != "$VERSION" ]; then
  echo "release: Cargo.toml says $IN_CARGO, not $VERSION" >&2
  exit 1
fi
if gh release view "v$VERSION" -R "$PUBLIC" >/dev/null 2>&1; then
  echo "release: $PUBLIC already has v$VERSION" >&2
  exit 1
fi
# What is built is what GitHub has, so main must be pushed and be what
# is here.
jj git fetch >/dev/null
if [ "$(jj log --no-graph -r main -T commit_id)" != "$(jj log --no-graph -r main@origin -T commit_id)" ]; then
  echo "release: main is not pushed" >&2
  exit 1
fi
COMMIT=$(jj log --no-graph -r main -T commit_id)

ASKED=$(date -u +%Y-%m-%dT%H:%M:%SZ)
git tag "v$VERSION" "$COMMIT"
git push origin "v$VERSION"
gh workflow run release.yml -R "$PUBLIC" -f version="$VERSION" -f ref="v$VERSION"

# The run just asked for takes a moment to be listed.
RUN=""
for _ in $(seq 30); do
  RUN=$(gh run list -R "$PUBLIC" --workflow release.yml --event workflow_dispatch \
    --limit 5 --json databaseId,createdAt \
    -q "[.[] | select(.createdAt >= \"$ASKED\")] | last | .databaseId // empty")
  [ -n "$RUN" ] && break
  sleep 2
done
if [ -z "$RUN" ]; then
  echo "release: the release workflow did not start in $PUBLIC" >&2
  exit 1
fi
echo "release: building v$VERSION (run $RUN in $PUBLIC)"
if ! gh run watch "$RUN" -R "$PUBLIC" --exit-status >/dev/null; then
  echo "release: the build failed: gh run view $RUN -R $PUBLIC --log-failed" >&2
  exit 1
fi

gh workflow run update.yml -R "$TAP" -f version="$VERSION"
echo "release: v$VERSION is published, and $TAP is rewriting its formula"
