class Datara < Formula
  desc "High-performance Post-OOP systems language & Forgen compiler"
  homepage "https://github.com/datara-lang/datara"
  version "1.4.3"
  license any_of: ["Apache-2.0", "MIT"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/datara-lang/datara/releases/download/v1.4.3/forgen-darwin-arm64.tar.gz"
      sha256 "24b41da0e51677dcc9f84cd42cdb631e43ffee1da550f63bbdc5dc767d2ccba7"
    else
      url "https://github.com/datara-lang/datara/releases/download/v1.4.3/forgen-darwin-x64.tar.gz"
      sha256 "24b41da0e51677dcc9f84cd42cdb631e43ffee1da550f63bbdc5dc767d2ccba7"
    end
  end

  on_linux do
    url "https://github.com/datara-lang/datara/releases/download/v1.4.3/forgen-linux-x64.tar.gz"
    sha256 "24b41da0e51677dcc9f84cd42cdb631e43ffee1da550f63bbdc5dc767d2ccba7"
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
