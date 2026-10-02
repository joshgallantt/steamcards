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
  version "0.1.2"
  license "MIT"

  depends_on :macos

  on_arm do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.2/steamcards-aarch64-apple-darwin.tar.gz"
    sha256 "a3887c5b242d41a5a5e4741341e368148caedec587ce52750f17286447adffd9"
  end

  on_intel do
    url "https://github.com/joshgallantt/steamcards/releases/download/v0.1.2/steamcards-x86_64-apple-darwin.tar.gz"
    sha256 "b711557e54ee4055455836db9cfdc87231b5a8f6649baf04fd29fda7b4243ee0"
  end

  def install
    bin.install "steamcards"
  end

  test do
    assert_match "steamcards #{version}", shell_output("#{bin}/steamcards --version")
  end
end
