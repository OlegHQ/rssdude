class Rssdude < Formula
  desc "Local-first RSS feed reader and content curation CLI"
  homepage "https://github.com/OlegHQ/rssdude"
  version "0.1.0"

  on_macos do
    on_arm do
      url "https://github.com/OlegHQ/rssdude/releases/download/v#{version}/rssdude-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "92fa156d1e58e89aef7aa9dcea8ffb75c9392d210b8c59ad1382f1af213215a9"
    end
    on_intel do
      odie "rssdude v#{version} ships only as arm64; x86_64 build is planned for v0.1.1."
    end
  end

  def install
    bin.install "rssdude"
  end

  test do
    assert_match "rssdude #{version}", shell_output("#{bin}/rssdude --version")
  end
end
