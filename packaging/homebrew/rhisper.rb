# Reference copy of the formula actually published at
# https://github.com/lv10/homebrew-rhisper/blob/main/Formula/rhisper.rb -
# the release pipeline's publish-homebrew job updates that file directly
# (via mislav/bump-homebrew-formula-action, which patches `url`/`sha256` in
# place), it does not copy this file. Keep this in sync by hand when the
# formula's structure (deps, install steps) changes.
#
# Builds from source rather than shipping a prebuilt binary, so there's no
# downloaded unsigned executable for Gatekeeper to quarantine - no
# codesigning/notarization needed.
class Rhisper < Formula
  desc "Dictation at cursor for Linux and macOS"
  homepage "https://github.com/lv10/rhisper"
  url "https://github.com/lv10/rhisper/archive/refs/tags/v0.2.3.tar.gz"
  sha256 "c2d288635db2ce4567244e904135f6e29125307d7c081908b8e8234192ff522c"
  license "MIT"

  depends_on "rust" => :build
  depends_on "sox"
  depends_on "ffmpeg"

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
    # rhispertool is a Linux-only uinput daemon/client (see
    # src/bin/rhispertool.rs's non-Linux stub) - don't ship the stub binary.
    (bin/"rhispertool").delete if (bin/"rhispertool").exist?
  end

  test do
    system "#{bin}/rhisper", "--config"
  end
end
