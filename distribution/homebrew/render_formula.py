import argparse
from pathlib import Path

from distribution.homebrew._validation import (
    validate_cdn_url,
    validate_sha256,
    validate_version,
)


def render_formula(
    *,
    version: str,
    arm64_url: str,
    arm64_sha256: str,
    x64_url: str,
    x64_sha256: str,
) -> str:
    validate_version(version)
    validate_cdn_url(arm64_url, "arm64 URL")
    validate_cdn_url(x64_url, "x64 URL")
    validate_sha256(arm64_sha256, "arm64")
    validate_sha256(x64_sha256, "x64")

    return f'''class Lx < Formula
  desc "Tencent Lexiang command-line client"
  homepage "https://lexiang.tencent.com"
  license "MIT"

  depends_on :macos

  on_macos do
    if Hardware::CPU.arm?
      url "{arm64_url}"
      sha256 "{arm64_sha256}"
    else
      url "{x64_url}"
      sha256 "{x64_sha256}"
    end
  end

  def install
    bin.install "lx"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/lx version")
  end
end
'''


def main() -> None:
    parser = argparse.ArgumentParser(description="Render Formula/lx.rb")
    parser.add_argument("--version", required=True)
    parser.add_argument("--arm64-url", required=True)
    parser.add_argument("--arm64-sha256", required=True)
    parser.add_argument("--x64-url", required=True)
    parser.add_argument("--x64-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    rendered = render_formula(
        version=args.version,
        arm64_url=args.arm64_url,
        arm64_sha256=args.arm64_sha256,
        x64_url=args.x64_url,
        x64_sha256=args.x64_sha256,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")


if __name__ == "__main__":
    main()
