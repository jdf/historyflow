#!/usr/bin/env bash
# Releases the historyflow command: tags main, runs the release workflow
# on that tag (which builds for each platform and publishes a GitHub
# release), then writes the Homebrew formula for the release, commits it
# and pushes. It waits for the build, which takes a few minutes.
#
#   tools/release.sh 0.2.0
#
# The version must be the one in Cargo.toml, and the commit released is
# main as pushed: bump the version, commit, push, then run this.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: tools/release.sh <version>}"
PUBLIC=jdf/historyflow

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
gh workflow run release.yml -R "$PUBLIC" --ref main -f version="$VERSION" -f ref="v$VERSION"

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

tools/formula.sh "$VERSION"
jj commit -m "The Homebrew formula for $VERSION" Formula/historyflow.rb
jj bookmark set main -r @-
jj git push --bookmark main
echo "release: v$VERSION is published, and the formula is pushed"
