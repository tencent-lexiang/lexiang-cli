import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class NuGetWorkflowContractTests(unittest.TestCase):
    def test_pack_validates_native_binary_before_generating_a_package(self):
        path = (
            ROOT
            / "distribution"
            / "nuget"
            / "TencentLexiang.Cli"
            / "TencentLexiang.Cli.csproj"
        )
        project = path.read_text(encoding="utf-8")

        self.assertIn(
            '<Target Name="ValidateLxBinary" BeforeTargets="GenerateNuspec">',
            project,
        )
        self.assertNotIn('BeforeTargets="Pack"', project)

    def test_workflow_builds_without_credentials_and_publishes_with_oidc(self):
        path = ROOT / ".github" / "workflows" / "nuget-release.yml"
        self.assertTrue(path.is_file(), f"missing workflow: {path}")
        workflow = path.read_text(encoding="utf-8")

        for required in (
            "workflow_dispatch:",
            "publish:",
            "contents: read",
            "LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64",
            "static.lexiang-asset.com/download/app/lexiang-desktop",
            "expected_platform=\"win32\"",
            "expected_arch=\"x64\"",
            "dotnet test",
            "dotnet pack",
            "dotnet tool install",
            "lx version",
            "environment: release",
            "id-token: write",
            "uses: NuGet/login@v1",
            "NUGET_USER",
            "dotnet nuget push",
            "nuget-lx-release",
        ):
            self.assertIn(required, workflow)
        self.assertNotIn("cargo build", workflow)
        self.assertNotIn("pull_request_target", workflow)
        self.assertNotIn("NUGET_API_KEY:", workflow)
        self.assertLess(workflow.index("environment: release"), workflow.index("id-token: write"))
        self.assertLess(workflow.index("environment: release"), workflow.index("secrets."))


if __name__ == "__main__":
    unittest.main()
