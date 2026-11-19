# Written by tools/formula.sh; edit it there.
class Historyflow < Formula
  desc "See how a document came to be, version by version"
  homepage "https://github.com/jdf/historyflow"
  version "0.2.3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.3/historyflow-0.2.3-aarch64-apple-darwin.tar.gz"
      sha256 "582413f5a335aa1338a5469a04293429b28c19c0f327f57dad6011821acf527a"
    end
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.3/historyflow-0.2.3-x86_64-apple-darwin.tar.gz"
      sha256 "56143bf1313f1d192955bccecf5359404542907f036dcd04b279187578a79227"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/jdf/historyflow/releases/download/v0.2.3/historyflow-0.2.3-x86_64-unknown-linux-musl.tar.gz"
      sha256 "65499d0aff3a4c8a3cf59ee4812722a3065a6f18e568cc01d86e3ad07364ba18"
    end
  end

  def install
    bin.install "historyflow"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/historyflow --version")
  end
end
