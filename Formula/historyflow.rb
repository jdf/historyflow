# Written by tools/formula.sh; edit it there.
class Historyflow < Formula
  desc "See how a document came to be, version by version"
  homepage "https://github.com/jdf/historyflow"
  version "0.2.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.4/historyflow-0.2.4-aarch64-apple-darwin.tar.gz"
      sha256 "a54d2414fee60c8fe3397c0f5ee2eb783dcd5613230e417d8071b96104e37b7e"
    end
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.4/historyflow-0.2.4-x86_64-apple-darwin.tar.gz"
      sha256 "43406496284133a7677e959ebb2422c44830b3f393806c5080c42e6ddfd76fbb"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.4/historyflow-0.2.4-x86_64-unknown-linux-musl.tar.gz"
      sha256 "a93043b45b4205e58768a92fd807eeffe3764e23e4c35a88b4b2714ec70a96bf"
    end
  end

  def install
    bin.install "historyflow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/historyflow --version")
  end
end
