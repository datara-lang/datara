class Datara < Formula
  desc "High-performance Post-OOP systems language & Forgen compiler"
  homepage "https://github.com/datara-lang/datara"
  version "1.2.7"
  license any_of: ["Apache-2.0", "MIT"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/datara-lang/datara/releases/download/v1.2.7/forgen-darwin-arm64.tar.gz"
      sha256 "b492d116e1c1bef761363ce582913f53bef66b5abcb5268e3fcc6bc8f528b317"
    else
      url "https://github.com/datara-lang/datara/releases/download/v1.2.7/forgen-darwin-x64.tar.gz"
      sha256 "e38dab858c083a2faf5783840e170dd888c353383a21975fb4707005ec2eec7a"
    end
  end

  on_linux do
    url "https://github.com/datara-lang/datara/releases/download/v1.2.7/forgen-linux-x64.tar.gz"
    sha256 "9ed70d548e28c284198caf2a37fb80d4a608bd76e3bb0c3cd415e67fbd968be9"
  end

  def install
    bin.install "forgen"
    bin.install_symlink "forgen" => "datara"
    pkgshare.install Dir["stdlib/*"]
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
