# Written by tools/formula.sh; edit it there.
class Historyflow < Formula
  desc "See how a document came to be, version by version"
  homepage "https://github.com/jdf/historyflow"
  version "0.2.5"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.5/historyflow-0.2.5-aarch64-apple-darwin.tar.gz"
      sha256 "81eea401cb342c01f833f3253f109ff763df54371d066ce8f8cb8ca225311c8e"
    end
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.5/historyflow-0.2.5-x86_64-apple-darwin.tar.gz"
      sha256 "6a72f012556ce0aac07d37154e61453a8876d74225948f5285962e87e5468b88"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.5/historyflow-0.2.5-x86_64-unknown-linux-musl.tar.gz"
      sha256 "cd5142c113bbb19b720b10f2507979a65268e414b9da1781f56e487d1d507525"
    end
  end

  def install
    bin.install "historyflow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/historyflow --version")
  end
end
