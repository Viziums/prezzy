class Prezzy < Formula
  desc "Make any CLI output beautiful. Zero config. Just pipe."
  homepage "https://github.com/viziums/prezzy"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/viziums/prezzy/releases/download/v#{version}/prezzy-aarch64-apple-darwin.tar.gz"
      # sha256 "UPDATE_AFTER_RELEASE"
    else
      url "https://github.com/viziums/prezzy/releases/download/v#{version}/prezzy-x86_64-apple-darwin.tar.gz"
      # sha256 "UPDATE_AFTER_RELEASE"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/viziums/prezzy/releases/download/v#{version}/prezzy-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_AFTER_RELEASE"
    else
      url "https://github.com/viziums/prezzy/releases/download/v#{version}/prezzy-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "UPDATE_AFTER_RELEASE"
    end
  end

  def install
    bin.install "prezzy"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/prezzy --version")
    assert_match "name", pipe_output("#{bin}/prezzy --color=never", '{"name":"test"}')
  end
end
