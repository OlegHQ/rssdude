class Rssdude < Formula
  desc "Local-first RSS feed reader and content curation CLI"
  homepage "https://github.com/OlegHQ/rssdude"
  version "0.2.1"

  on_macos do
    on_arm do
      url "https://github.com/OlegHQ/rssdude/releases/download/v#{version}/rssdude-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "2fa4f5b122accf4a4d569e472e1b2c3505769a33395a753af499ad04f8a2a8ed"
    end
    on_intel do
      odie "rssdude v#{version} ships only as arm64; x86_64 build is planned."
    end
  end

  def install
    bin.install "rssdude"
  end

  test do
    assert_match "rssdude #{version}", shell_output("#{bin}/rssdude --version")
  end
end
