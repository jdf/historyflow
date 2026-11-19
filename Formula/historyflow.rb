# Written by tools/formula.sh; edit it there.
class Historyflow < Formula
  desc "See how a document came to be, version by version"
  homepage "https://github.com/jdf/historyflow"
  version "0.2.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.2/historyflow-0.2.2-aarch64-apple-darwin.tar.gz"
      sha256 "bd63afa3d6fff8d8d3f98244a5a32ad562fa61e0c5eaad2462654f78ae498fa1"
    end
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.2/historyflow-0.2.2-x86_64-apple-darwin.tar.gz"
      sha256 "6ddeaaaeaf4827e74b57f826ad324ad29186259de7d39edaef15ee971ae6a366"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.2/historyflow-0.2.2-x86_64-unknown-linux-musl.tar.gz"
      sha256 "537ca39f4048ded71f4c5c285b34504b4f81e5c3aee9b0d1a1e018ec465e621a"
    end
  end

  def install
    bin.install "historyflow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/historyflow --version")
  end
end
