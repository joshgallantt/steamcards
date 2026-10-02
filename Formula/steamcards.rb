# Homebrew formula for steamcards' prebuilt macOS binaries. This repository
# is its own tap:
#
#   brew tap joshgallantt/steamcards https://github.com/joshgallantt/steamcards
#   brew install joshgallantt/steamcards/steamcards
#
# Each release points it at the new archives: `cargo xtask release` does it
# once the release is published, and pushes the change to main. That rewrites
# the version, each url and the sha256 line after it, so every url line has
# to be followed by its sha256 line. Until the first release is published,
# the checksums are placeholders, and installing it fails.
class Steamcards < Formula
  desc "Farms your Steam trading cards from the terminal"
  homepage "https://github.com/joshgallantt/steamcards"
  version "0.1.1"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.1/steamcards-aarch64-apple-darwin.tar.gz"
    sha256 "47a9aa5174e3afa2e89035c27cc108f364ce19497913cd1a5f5caa5bbab5313d"
  end

  on_intel do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.1/steamcards-x86_64-apple-darwin.tar.gz"
    sha256 "dd3ba9fc8c93bb6c3d6e15c737fcf24babbe48ac8599f9069f8abe1926ac9dcf"
  end

  def install
    bin.install "steamcards"
  end

  test do
    assert_match "steamcards #{version}", shell_output("#{bin}/steamcards --version")
  end
end
