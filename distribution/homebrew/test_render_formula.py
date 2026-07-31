import unittest

from distribution.homebrew.render_formula import render_formula


class FormulaRendererTests(unittest.TestCase):
    def test_renders_deterministic_architecture_specific_formula(self):
        arm64_url = (
            "https://static.lexiang-asset.com/download/app/"
            "lexiang-desktop/mac-arm64/cli/1.2.3/lx"
        )
        x64_url = (
            "https://static.lexiang-asset.com/download/app/"
            "lexiang-desktop/mac-x64/cli/1.2.3/lx"
        )
        arm64_sha256 = "a" * 64
        x64_sha256 = "b" * 64

        formula = render_formula(
            version="1.2.3",
            arm64_url=arm64_url,
            arm64_sha256=arm64_sha256,
            x64_url=x64_url,
            x64_sha256=x64_sha256,
        )

        self.assertIn("class Lx < Formula", formula)
        self.assertNotIn('version "1.2.3"', formula)
        self.assertIn("depends_on :macos", formula)
        self.assertIn("on_macos do", formula)
        self.assertIn("if Hardware::CPU.arm?", formula)
        self.assertIn("else", formula)
        self.assertNotIn("on_arm do", formula)
        self.assertNotIn("on_intel do", formula)
        self.assertIn('bin.install "lx"', formula)
        self.assertIn('shell_output("#{bin}/lx version")', formula)
        self.assertNotIn("latest", formula)
        for value in (arm64_url, x64_url, arm64_sha256, x64_sha256):
            self.assertEqual(formula.count(value), 1)
        self.assertTrue(formula.endswith("\n"))

    def test_rejects_non_semantic_version(self):
        with self.assertRaisesRegex(ValueError, "version"):
            render_formula(
                version="latest",
                arm64_url="https://static.lexiang-asset.com/arm64/lx",
                arm64_sha256="a" * 64,
                x64_url="https://static.lexiang-asset.com/x64/lx",
                x64_sha256="b" * 64,
            )

    def test_rejects_non_cdn_url(self):
        with self.assertRaisesRegex(ValueError, "CDN"):
            render_formula(
                version="1.2.3",
                arm64_url="https://example.invalid/arm64/lx",
                arm64_sha256="a" * 64,
                x64_url="https://static.lexiang-asset.com/x64/lx",
                x64_sha256="b" * 64,
            )


if __name__ == "__main__":
    unittest.main()
