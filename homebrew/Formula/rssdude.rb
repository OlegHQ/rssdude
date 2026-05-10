class Rssdude < Formula
  desc "Local-first RSS feed reader and content curation CLI"
  homepage "https://github.com/OlegHQ/rssdude"
  version "0.3.0"

  on_macos do
    on_arm do
      url "https://github.com/OlegHQ/rssdude/releases/download/v#{version}/rssdude-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "337501ae6b19c9e023a930c46d8d93a241ede20f11db7fdc35017ad645a34270"
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
