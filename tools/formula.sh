#!/usr/bin/env bash
# Writes Formula/historyflow.rb, the Homebrew formula, for a published
# release, from that release's checksums.
#
#   tools/formula.sh 0.2.1
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:?usage: tools/formula.sh <version>}"
RELEASES=https://github.com/jdf/historyflow/releases/download

SUMS=$(curl -fsSL "$RELEASES/v$VERSION/SHA256SUMS")
sha() {
  local sum
  sum=$(echo "$SUMS" | grep " historyflow-$VERSION-$1.tar.gz\$" | cut -d' ' -f1)
  [ ${#sum} = 64 ] || { echo "formula: no checksum for $1" >&2; exit 1; }
  echo "$sum"
}
url() { echo "$RELEASES/v$VERSION/historyflow-$VERSION-$1.tar.gz"; }
ARM=$(sha aarch64-apple-darwin)
INTEL=$(sha x86_64-apple-darwin)
LINUX=$(sha x86_64-unknown-linux-musl)

mkdir -p Formula
cat > Formula/historyflow.rb <<FORMULA
# Written by tools/formula.sh; edit it there.
class Historyflow < Formula
  desc "See how a document came to be, version by version"
  homepage "https://github.com/jdf/historyflow"
  version "$VERSION"
  license "MIT"

  on_macos do
    on_arm do
      url "$(url aarch64-apple-darwin)"
      sha256 "$ARM"
    end
    on_intel do
      url "$(url x86_64-apple-darwin)"
      sha256 "$INTEL"
    end
  end

  on_linux do
    on_intel do
      url "$(url x86_64-unknown-linux-musl)"
      sha256 "$LINUX"
    end
  end

  def install
    bin.install "historyflow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/historyflow --version")
  end
end
FORMULA
