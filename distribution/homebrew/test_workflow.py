import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class HomebrewWorkflowContractTests(unittest.TestCase):
    def workflow(self, name):
        path = ROOT / ".github" / "workflows" / name
        self.assertTrue(path.is_file(), f"missing workflow: {path}")
        return path.read_text(encoding="utf-8")

    def test_formula_workflow_is_verified_and_protected(self):
        workflow = self.workflow("homebrew-release.yml")

        for required in (
            "workflow_dispatch:",
            "publish:",
            "contents: read",
            "static.lexiang-asset.com/download/app/lexiang-desktop",
            "LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64",
            "Formula/lx.rb",
            "candidate/Formula/lx.rb",
            "brew tap-new --no-git local/lexiang-distribution",
            "brew audit --strict",
            "brew install --formula local/lexiang-distribution/lx",
            "environment: release",
            "actions/create-github-app-token@v2",
            "HOMEBREW_TAP_APP_ID",
            "HOMEBREW_TAP_APP_PRIVATE_KEY",
            "HOMEBREW_TAP_REPOSITORY",
            "git status --porcelain -- Formula/lx.rb",
            "homebrew-lx-release",
        ):
            self.assertIn(required, workflow)
        self.assertNotIn("cargo build", workflow)
        self.assertNotIn("git diff --quiet -- Formula/lx.rb", workflow)
        self.assertNotIn("pull_request_target", workflow)
        self.assertNotIn("id-token: write", workflow)
        self.assertLess(workflow.index("environment: release"), workflow.index("secrets."))

    def test_cask_workflow_requires_platform_security_before_publication(self):
        workflow = self.workflow("homebrew-cask-release.yml")

        for required in (
            "workflow_dispatch:",
            "date_stamp:",
            "publish:",
            "contents: read",
            "static.lexiang-asset.com/download/app/lexiang-desktop",
            "TencentLexiang.app",
            "codesign --verify",
            "spctl --assess",
            "xcrun stapler validate",
            "brew audit --cask --strict",
            "brew install --cask",
            "brew tap-new --no-git local/lexiang-distribution",
            "local/lexiang-distribution/lexiang",
            "Casks/lexiang.rb",
            "candidate/Casks/lexiang.rb",
            "environment: release",
            "actions/create-github-app-token@v2",
            "HOMEBREW_TAP_APP_PRIVATE_KEY",
            "git status --porcelain -- Casks/lexiang.rb",
            "homebrew-lexiang-cask-release",
        ):
            self.assertIn(required, workflow)
        self.assertGreaterEqual(workflow.count("xcrun stapler validate"), 2)
        self.assertNotIn("cargo build", workflow)
        self.assertNotIn("git diff --quiet -- Casks/lexiang.rb", workflow)
        self.assertNotIn("pull_request_target", workflow)
        self.assertNotIn("--no-quarantine", workflow)
        self.assertLess(workflow.index("environment: release"), workflow.index("secrets."))


if __name__ == "__main__":
    unittest.main()
