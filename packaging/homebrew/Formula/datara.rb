class Datara < Formula
  desc "High-performance Post-OOP systems language & Forgen compiler"
  homepage "https://github.com/datara-lang/datara"
  version "1.4.1"
  license any_of: ["Apache-2.0", "MIT"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-darwin-arm64.tar.gz"
      sha256 "96d587f3f796dc6ac7fddb61d0e85bf09103640cecb23505218a425b5fad4e97"
    else
      url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-darwin-x64.tar.gz"
      sha256 "9f54b96a56a6d98501a7c98c73cf23b6df55d4f0586bce0134d7a4a4f81c5e42"
    end
  end

  on_linux do
    url "https://github.com/datara-lang/datara/releases/download/v1.4.1/forgen-linux-x64.tar.gz"
    sha256 "7a3940385388ad7868161f61a5cb6a1b412b66df07c24536b1da2886a807ace5"
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
