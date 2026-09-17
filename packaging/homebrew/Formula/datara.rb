class Datara < Formula
  desc "High-performance Post-OOP systems language & Forgen compiler"
  homepage "https://github.com/datara-lang/datara"
  version "1.4.2"
  license any_of: ["Apache-2.0", "MIT"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/datara-lang/datara/releases/download/v1.4.2/forgen-darwin-arm64.tar.gz"
      sha256 "f6360e7f52a048c91c9dfd1c2d81ede401053266d998d9d8556e97a696c45c26"
    else
      url "https://github.com/datara-lang/datara/releases/download/v1.4.2/forgen-darwin-x64.tar.gz"
      sha256 "f6360e7f52a048c91c9dfd1c2d81ede401053266d998d9d8556e97a696c45c26"
    end
  end

  on_linux do
    url "https://github.com/datara-lang/datara/releases/download/v1.4.2/forgen-linux-x64.tar.gz"
    sha256 "f6360e7f52a048c91c9dfd1c2d81ede401053266d998d9d8556e97a696c45c26"
  end

  def install
    bin.install "forgen"
    bin.install_symlink "forgen" => "datara"
    pkgshare.install Dir["stdlib/*"]
    (lib/"datara").install Dir["runtime/*"] if Dir.exist?("runtime")
  end

  test do
    (testpath/"test.dtr").write <<~EOS
      fn main() {
        println("Hello from Homebrew Datara!")
      }
    EOS
    assert_match "Hello from Homebrew Datara!", shell_output("#{bin}/forgen run test.dtr")
  end
end
