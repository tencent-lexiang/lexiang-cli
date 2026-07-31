import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class DistributionCIContractTests(unittest.TestCase):
    def test_ci_is_credential_free_and_covers_all_distribution_adapters(self):
        path = ROOT / ".github" / "workflows" / "distribution-ci.yml"
        self.assertTrue(path.is_file(), f"missing workflow: {path}")
        workflow = path.read_text(encoding="utf-8")

        for required in (
            "pull_request:",
            "distribution/**",
            "python3 -m unittest",
            "ruby -c",
            "runs-on: windows-latest",
            "dotnet test",
            "dotnet pack",
            "System.IO.Compression.ZipFile",
            "actionlint",
            "markdownlint-cli2",
            "actionlint .github/workflows/distribution-ci.yml",
            ".github/workflows/homebrew-release.yml",
            ".github/workflows/homebrew-cask-release.yml",
            ".github/workflows/nuget-release.yml",
            "tools/net8.0/any/native/lx.exe",
        ):
            self.assertIn(required, workflow)
        for forbidden in (
            "environment: release",
            "secrets.",
            "HOMEBREW_TAP_APP_PRIVATE_KEY",
            "NUGET_USER",
            "id-token: write",
            "cargo build",
            "unzip -l",
            "actionlint .github/workflows/*.yml",
        ):
            self.assertNotIn(forbidden, workflow)


if __name__ == "__main__":
    unittest.main()
