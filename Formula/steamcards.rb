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
  version "0.1.3"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.3/steamcards-aarch64-apple-darwin.tar.gz"
    sha256 "2f06ae5647ec62a287edfa7449ae8dbf162ed3f052ab9d9ac71a386c0a7675d2"
  end

  on_intel do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.3/steamcards-x86_64-apple-darwin.tar.gz"
    sha256 "39793b25d691d1127bd88bd2114acceb459dc19a5a2188014879986cca4068bc"
  end

  def install
    bin.install "steamcards"
  end

  test do
    assert_match "steamcards #{version}", shell_output("#{bin}/steamcards --version")
  end
end
