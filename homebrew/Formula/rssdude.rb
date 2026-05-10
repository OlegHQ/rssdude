class Rssdude < Formula
  desc "Local-first RSS feed reader and content curation CLI"
  homepage "https://github.com/OlegHQ/rssdude"
  version "0.3.1"

  on_macos do
    on_arm do
      url "https://github.com/OlegHQ/rssdude/releases/download/v#{version}/rssdude-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "d3e13572bf851f6330d63e66bdb0c09c8044056a81e2e521a7666e1b0bcecef2"
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
