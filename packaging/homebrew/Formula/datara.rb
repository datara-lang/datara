class Datara < Formula
  desc "High-performance Post-OOP systems language & Forgen compiler"
  homepage "https://github.com/datara-lang/datara"
  version "1.4.1"
  license any_of: ["Apache-2.0", "MIT"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-darwin-arm64.tar.gz"
      sha256 "fd30408c78a9eeeaa90a23b502936953753b87684ae34c5ab1e099b8ba9c2e67"
    else
      url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-darwin-x64.tar.gz"
      sha256 "b19ca68f70e66f73a1640ab54c2e3776645d78c9c123c10e9bd838ba4d8a7e87"
    end
  end

  on_linux do
    url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-linux-x64.tar.gz"
    sha256 "c472b8942be0d8982b44cb92ee16694387b9f065e34e1110d07024f687d1d7d4"
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
