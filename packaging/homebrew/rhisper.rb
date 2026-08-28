# Source of truth for the Homebrew formula. The release pipeline mirrors
# this (with `url`/`sha256` filled in for the tagged release) into the
# lv10/homebrew-rhisper tap repo - this file itself is never installed
# directly via `brew install ./rhisper.rb` in production.
#
# Builds from source rather than shipping a prebuilt binary, so there's no
# downloaded unsigned executable for Gatekeeper to quarantine - no
# codesigning/notarization needed.
class Rhisper < Formula
  desc "Dictation at cursor for Linux and macOS"
  homepage "https://github.com/lv10/rhisper"
  url "https://github.com/lv10/rhisper/archive/refs/tags/v0.0.0.tar.gz"
  sha256 "REPLACED_PER_RELEASE"
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
