# Homebrew formula template — rendered by CI, committed to tinyhumansai/homebrew-aura.
# Placeholders replaced by .github/workflows/release-packages.yml before commit.
class Aura < Formula
  desc "AI-powered assistant for communities — Aura CLI"
  homepage "https://github.com/tinyhumansai/aura"
  version "@VERSION@"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/tinyhumansai/aura/releases/download/v@VERSION@/aura-core-@VERSION@-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256_MACOS_ARM64@"
    end
    on_intel do
      url "https://github.com/tinyhumansai/aura/releases/download/v@VERSION@/aura-core-@VERSION@-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256_MACOS_X64@"
    end
  end

  on_linux do
    on_arm do
      # ARM64 (aarch64)
      url "https://github.com/tinyhumansai/aura/releases/download/v@VERSION@/aura-core-@VERSION@-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA256_LINUX_ARM64@"
    end
    on_intel do
      url "https://github.com/tinyhumansai/aura/releases/download/v@VERSION@/aura-core-@VERSION@-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA256_LINUX_X64@"
    end
  end

  def install
    bin.install "aura-core" => "aura"
  end

  test do
    system "#{bin}/aura", "--version"
  end
end
