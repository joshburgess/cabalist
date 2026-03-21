class Cabalist < Formula
  desc "Interactive TUI, CLI, and LSP for managing Haskell .cabal files"
  homepage "https://github.com/joshburgess/cabalist"
  version "0.1.0"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/joshburgess/cabalist/releases/download/v#{version}/cabalist-v#{version}-aarch64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/joshburgess/cabalist/releases/download/v#{version}/cabalist-v#{version}-x86_64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/joshburgess/cabalist/releases/download/v#{version}/cabalist-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "PLACEHOLDER"
    end
    on_intel do
      url "https://github.com/joshburgess/cabalist/releases/download/v#{version}/cabalist-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      # sha256 "PLACEHOLDER"
    end
  end

  def install
    bin.install "cabalist"
    bin.install "cabalist-cli"
    bin.install "cabalist-lsp"
  end

  test do
    system "#{bin}/cabalist-cli", "--version"
    system "#{bin}/cabalist-lsp", "--help"
  end
end
