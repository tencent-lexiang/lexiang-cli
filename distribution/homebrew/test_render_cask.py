import unittest

from distribution.homebrew.render_cask import render_cask


class CaskRendererTests(unittest.TestCase):
    def test_renders_deterministic_architecture_specific_cask(self):
        cask = render_cask(
            version="1.2.3",
            date_stamp="20260731",
            arm64_sha256="a" * 64,
            x64_sha256="b" * 64,
        )

        versioned_url = (
            "https://static.lexiang-asset.com/download/app/lexiang-desktop/"
            "mac-#{arch}/#{version}/"
            "lexiang-desktop-#{version}-20260731-#{arch}.dmg"
        )
        self.assertIn('cask "lexiang" do', cask)
        self.assertIn('version "1.2.3"', cask)
        self.assertIn("auto_updates true", cask)
        self.assertIn('arch arm: "arm64", intel: "x64"', cask)
        self.assertIn(
            f'sha256 arm: "{"a" * 64}", intel: "{"b" * 64}"',
            cask,
        )
        self.assertIn(versioned_url, cask)
        self.assertNotIn("on_arm do", cask)
        self.assertNotIn("on_intel do", cask)
        self.assertIn('app "TencentLexiang.app"', cask)
        self.assertIn('name "乐享知识库"', cask)
        for value in ("a" * 64, "b" * 64):
            self.assertEqual(cask.count(value), 1)
        self.assertTrue(cask.endswith("\n"))

    def test_rejects_non_semantic_version(self):
        with self.assertRaisesRegex(ValueError, "version"):
            render_cask(
                version="nightly",
                date_stamp="20260731",
                arm64_sha256="a" * 64,
                x64_sha256="b" * 64,
            )

    def test_rejects_invalid_date_stamp(self):
        with self.assertRaisesRegex(ValueError, "date"):
            render_cask(
                version="1.2.3",
                date_stamp="2026-07-31",
                arm64_sha256="a" * 64,
                x64_sha256="b" * 64,
            )

    def test_rejects_invalid_sha256(self):
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            render_cask(
                version="1.2.3",
                date_stamp="20260731",
                arm64_sha256="not-a-digest",
                x64_sha256="b" * 64,
            )


if __name__ == "__main__":
    unittest.main()
