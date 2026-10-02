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
  version "0.1.4"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.4/steamcards-aarch64-apple-darwin.tar.gz"
    sha256 "8c95b5e6c43e08f16bfb5f5ce6de962dca44810baf5cb341d19297cfac92754c"
  end

  on_intel do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.4/steamcards-x86_64-apple-darwin.tar.gz"
    sha256 "fe366acd1acb249f253247deb7876799a5b24b1bc26fa6f2af71d8e0380bd7a2"
  end

  def install
    bin.install "steamcards"
  end

  test do
    assert_match "steamcards #{version}", shell_output("#{bin}/steamcards --version")
  end
end
