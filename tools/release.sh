#!/usr/bin/env bash
# Releases the historyflow command: tags this repository and asks the
# public distribution repository (jdf/historyflow-cli) to build that tag
# and publish it, as a GitHub release there and as its Homebrew formula.
#
#   tools/release.sh 0.2.0
#
# The version must be the one in Cargo.toml, and the commit released is
# main as pushed: bump the version, commit, push, then run this.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: tools/release.sh <version>}"
PUBLIC=jdf/historyflow-cli

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

git tag "v$VERSION" "$COMMIT"
git push origin "v$VERSION"
gh workflow run release.yml -R "$PUBLIC" -f version="$VERSION" -f ref="v$VERSION"
echo "release: building v$VERSION; watch it with"
echo "  gh run watch -R $PUBLIC \$(gh run list -R $PUBLIC --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')"
