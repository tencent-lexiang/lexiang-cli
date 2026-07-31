import argparse
from pathlib import Path

from distribution.homebrew._validation import (
    validate_date_stamp,
    validate_sha256,
    validate_version,
)


CDN_ROOT = "https://static.lexiang-asset.com/download/app/lexiang-desktop"


def render_cask(
    *,
    version: str,
    date_stamp: str,
    arm64_sha256: str,
    x64_sha256: str,
) -> str:
    validate_version(version)
    validate_date_stamp(date_stamp)
    validate_sha256(arm64_sha256, "arm64")
    validate_sha256(x64_sha256, "x64")

    return f'''cask "lexiang" do
  version "{version}"
  arch arm: "arm64", intel: "x64"

  sha256 arm: "{arm64_sha256}", intel: "{x64_sha256}"

  url "{CDN_ROOT}/mac-#{{arch}}/#{{version}}/lexiang-desktop-#{{version}}-{date_stamp}-#{{arch}}.dmg",
      verified: "static.lexiang-asset.com/"

  name "乐享知识库"
  desc "Tencent Lexiang knowledge and agent desktop application"
  homepage "https://lexiang.tencent.com"

  auto_updates true

  app "TencentLexiang.app"
end
'''


def main() -> None:
    parser = argparse.ArgumentParser(description="Render Casks/lexiang.rb")
    parser.add_argument("--version", required=True)
    parser.add_argument("--date-stamp", required=True)
    parser.add_argument("--arm64-sha256", required=True)
    parser.add_argument("--x64-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    rendered = render_cask(
        version=args.version,
        date_stamp=args.date_stamp,
        arm64_sha256=args.arm64_sha256,
        x64_sha256=args.x64_sha256,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(rendered, encoding="utf-8")


if __name__ == "__main__":
    main()
