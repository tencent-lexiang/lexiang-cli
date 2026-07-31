# Homebrew and NuGet Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add dry-run-safe and production-gated Homebrew and NuGet distribution workflows to `tencent-lexiang/lexiang-cli` without rebuilding the Rust CLI in this repository.

**Architecture:** Both workflows consume per-platform signed CLI manifests and immutable native binaries produced by `lexiang-desktop`. Shared Python tooling verifies the Ed25519 signature, release version, HTTPS artifact URL, size, and SHA-256 before a package is generated. Homebrew publication updates a separate `tencent-lexiang/homebrew-tap` repository; NuGet publication uses nuget.org Trusted Publishing through GitHub OIDC.

**Tech Stack:** GitHub Actions, Python 3 standard library, OpenSSL CLI, Homebrew Formula DSL, .NET 8 global-tool shim, NuGet Trusted Publishing.

---

## Scope Boundary

This plan is the first independently testable part of the `lexiang-cli` repository conversion. It does not migrate the VS Code, Obsidian, JetBrains, or shared `ide-*` source and does not remove `crates/lx`. Those changes require a separate implementation plan after distribution workflows can build in dry-run mode.

## File Structure

- `openspec/changes/add-cli-distribution-workflows/`: formal OpenSpec proposal, design, requirements, and task ledger.
- `distribution/release/manifest.py`: shared signed-manifest retrieval and artifact verification.
- `distribution/release/test_manifest.py`: standard-library unit tests for verification failures and success.
- `distribution/homebrew/render_formula.py`: deterministic `Formula/lx.rb` renderer.
- `distribution/homebrew/test_render_formula.py`: renderer contract tests.
- `distribution/homebrew/README.md`: tap topology and local dry-run commands.
- `distribution/nuget/TencentLexiang.Cli/TencentLexiang.Cli.csproj`: Windows x64 .NET global-tool package.
- `distribution/nuget/TencentLexiang.Cli/Program.cs`: thin process launcher for the packaged native `lx.exe`.
- `distribution/nuget/TencentLexiang.Cli.Tests/`: launcher behavior tests using a fake native child.
- `distribution/nuget/README.md`: package build and local installation guide.
- `.github/workflows/distribution-ci.yml`: credential-free manifest, formula, and NuGet package validation.
- `.github/workflows/homebrew-release.yml`: protected Homebrew formula publication.
- `.github/workflows/nuget-release.yml`: protected NuGet OIDC publication.
- `docs/releasing/distribution-credentials.md`: credential application and GitHub configuration guide.
- `README.md`: public installation commands and repository responsibility statement.

### Task 1: Create and approve the OpenSpec change

**Files:**

- Create: `openspec/changes/add-cli-distribution-workflows/proposal.md`
- Create: `openspec/changes/add-cli-distribution-workflows/design.md`
- Create: `openspec/changes/add-cli-distribution-workflows/tasks.md`
- Create: `openspec/changes/add-cli-distribution-workflows/specs/cli-distribution/spec.md`

- [ ] **Step 1: Create the proposal**

Write `proposal.md` with this exact scope:

```markdown
# Change: Add external CLI distribution workflows

## Why
The native `lx` source is moving to `lexiang-desktop`, but Homebrew and NuGet
still need a public, auditable distribution path that does not rebuild or
modify the native executable.

## What Changes
- Add signed desktop CLI manifest and artifact verification.
- Add Homebrew formula generation and protected tap publication.
- Add a Windows x64 NuGet global-tool wrapper and OIDC publication.
- Add credential-free dry-run CI and operator credential documentation.
- Keep the existing Rust CLI temporarily; its removal is a later change.

## Impact
- Affected specs: cli-distribution
- Affected code: distribution/, .github/workflows/, docs/releasing/, README.md
```

- [ ] **Step 2: Create the delta specification**

Write `specs/cli-distribution/spec.md` with requirements for:

```markdown
## ADDED Requirements

### Requirement: Verified upstream CLI artifacts
The distribution workflows MUST consume an immutable native CLI artifact built
by `lexiang-desktop` and MUST verify the signed manifest, requested version,
artifact size, and SHA-256 before packaging.

#### Scenario: Verified artifact
- **WHEN** the manifest signature, version, target, size, and digest are valid
- **THEN** the workflow permits credential-free package construction

#### Scenario: Verification failure
- **WHEN** any signature or artifact check fails
- **THEN** the workflow exits before package construction or publication

### Requirement: Protected Homebrew publication
The repository MUST render and test a macOS arm64/x64 `lx` formula before a
protected release job updates the configured tap repository.

#### Scenario: Dry-run formula
- **WHEN** a maintainer runs the workflow without publication enabled
- **THEN** the tested formula is uploaded only as a workflow artifact

#### Scenario: Published formula
- **WHEN** publication is enabled and the release environment is approved
- **THEN** the exact tested formula is committed to the configured tap

### Requirement: Keyless NuGet publication
The repository MUST package the Windows x64 native CLI behind a thin .NET tool
launcher and MUST publish through nuget.org Trusted Publishing by default.

#### Scenario: Dry-run NuGet package
- **WHEN** a maintainer runs the workflow without publication enabled
- **THEN** the validated `.nupkg` is uploaded only as a workflow artifact

#### Scenario: OIDC publication
- **WHEN** publication is enabled and the nuget.org trusted policy matches
- **THEN** GitHub OIDC is exchanged for a temporary NuGet API key and the exact
  validated package is pushed
```

- [ ] **Step 3: Write the design and task ledger**

In `design.md`, record the fixed public CDN manifest root, allowed-host check,
checked-in or repository-variable public key, separate tap repository,
`release` GitHub Environment, NuGet package ID `TencentLexiang.Cli`, and the
decision not to use postinstall downloads or long-lived NuGet keys. Explicitly
exclude the internal `mirrors.tencent.com` upload origin from workflow inputs.

In `tasks.md`, mirror Tasks 2–8 from this plan using OpenSpec numbered checkbox
syntax.

- [ ] **Step 4: Validate the proposal**

Run:

```bash
openspec validate add-cli-distribution-workflows --strict
```

Expected: `Change 'add-cli-distribution-workflows' is valid`.

- [ ] **Step 5: Request proposal approval**

Stop implementation and ask the user to approve:

```text
OpenSpec change `add-cli-distribution-workflows` is valid. Approve implementation?
```

- [ ] **Step 6: Commit the approved proposal**

```bash
git add openspec/changes/add-cli-distribution-workflows
git commit -m "docs: 规划 CLI 外部分发工作流"
```

### Task 2: Implement signed release-manifest verification

**Files:**

- Create: `distribution/release/__init__.py`
- Create: `distribution/release/manifest.py`
- Create: `distribution/release/test_manifest.py`
- Create: `distribution/release/README.md`

- [ ] **Step 1: Write failing manifest tests**

Cover these exact cases with `unittest` and temporary files:

- `test_rejects_http_artifact_url`: assert an `http://` artifact URL raises
  `ManifestVerificationError`.
- `test_rejects_unapproved_artifact_host`: assert an HTTPS URL outside
  `allowed_hosts` is rejected before artifact download.
- `test_rejects_wrong_component`: assert a component other than `lx-cli` is
  rejected.
- `test_rejects_wrong_version`: assert a manifest version different from
  `expected_version` is rejected.
- `test_rejects_wrong_platform_or_arch`: assert either target mismatch is
  rejected.
- `test_rejects_size_mismatch`: assert the temporary artifact is removed and
  verification fails.
- `test_rejects_sha256_mismatch`: assert the temporary artifact is removed and
  verification fails.
- `test_rejects_invalid_signature`: assert a non-zero fake OpenSSL result raises
  `ManifestVerificationError`.
- `test_accepts_verified_artifact`: assert the verified binary is atomically
  moved to its final path and all normalized metadata is returned.

Use a fake OpenSSL runner in unit tests so signature behavior is deterministic;
inject that runner into the verifier instead of invoking a real process.

- [ ] **Step 2: Run tests and verify failure**

```bash
python3 -m unittest distribution.release.test_manifest -v
```

Expected: FAIL because `distribution.release.manifest` does not exist.

- [ ] **Step 3: Implement the verifier**

Define the frozen `VerifiedArtifact` dataclass with `version`, `platform`,
`arch`, `url`, `size`, `sha256`, and `binary` fields. Define
`ManifestVerificationError(RuntimeError)`. Implement a keyword-only
`fetch_and_verify` entry point accepting `manifest_url`, `signature_url`,
`public_key`, `expected_version`, `expected_platform`, `expected_arch`,
`allowed_hosts`, and `output_dir`, and returning `VerifiedArtifact`.

Implementation requirements:

- use `urllib.request` with a fixed timeout and reject redirects to an
  unapproved host;
- require `schemaVersion == 1` and `component == "lx-cli"`;
- decode the detached signature from base64 before calling
  `openssl pkeyutl -verify -pubin -inkey "$PUBLIC_KEY" -rawin -in "$MANIFEST" -sigfile "$SIGNATURE"`;
- download to a temporary filename in `output_dir`;
- verify size and SHA-256 before `os.replace`;
- print only normalized metadata, never key material.

- [ ] **Step 4: Run tests and verify pass**

```bash
python3 -m unittest distribution.release.test_manifest -v
```

Expected: 9 tests pass.

- [ ] **Step 5: Commit**

```bash
git add distribution/release
git commit -m "feat: 校验 desktop CLI 发布制品"
```

### Task 3: Generate and test the Homebrew formula

**Files:**

- Create: `distribution/homebrew/render_formula.py`
- Create: `distribution/homebrew/test_render_formula.py`
- Create: `distribution/homebrew/README.md`

- [ ] **Step 1: Write failing renderer tests**

Tests MUST assert that the rendered formula:

```python
self.assertIn('class Lx < Formula', formula)
self.assertIn('depends_on :macos', formula)
self.assertIn('on_arm do', formula)
self.assertIn('on_intel do', formula)
self.assertIn('bin.install "lx"', formula)
self.assertIn('shell_output("#{bin}/lx version")', formula)
self.assertNotIn("latest", formula)
```

Also assert the exact version, URLs, and SHA-256 values appear once.

- [ ] **Step 2: Run tests and verify failure**

```bash
python3 -m unittest distribution.homebrew.test_render_formula -v
```

Expected: FAIL because the renderer does not exist.

- [ ] **Step 3: Implement deterministic rendering**

Expose a keyword-only `render_formula` function accepting `version`,
`arm64_url`, `arm64_sha256`, `x64_url`, and `x64_sha256`, and returning the
complete formula as a string.

Generate a macOS-only `Lx` formula with fixed homepage
`https://lexiang.tencent.com`, MIT license, architecture-specific immutable
URLs, `bin.install "lx"`, and a `lx version` test.

- [ ] **Step 4: Run unit and Homebrew syntax tests**

```bash
python3 -m unittest distribution.homebrew.test_render_formula -v
python3 distribution/homebrew/render_formula.py \
  --version 0.0.0-test \
  --arm64-url https://static.lexiang-asset.com/test/arm64/lx \
  --arm64-sha256 0000000000000000000000000000000000000000000000000000000000000000 \
  --x64-url https://static.lexiang-asset.com/test/x64/lx \
  --x64-sha256 1111111111111111111111111111111111111111111111111111111111111111 \
  --output /tmp/lx.rb
ruby -c /tmp/lx.rb
```

Expected: unit tests pass and Ruby prints `Syntax OK`.

- [ ] **Step 5: Commit**

```bash
git add distribution/homebrew
git commit -m "feat: 生成 Homebrew lx 配方"
```

### Task 4: Add the protected Homebrew workflow

**Files:**

- Create: `.github/workflows/homebrew-release.yml`
- Modify: `distribution/homebrew/README.md`

- [ ] **Step 1: Add a workflow contract test**

Create `distribution/homebrew/test_workflow.py` that parses the workflow as text
and asserts:

```python
self.assertIn("workflow_dispatch:", workflow)
self.assertIn("publish:", workflow)
self.assertIn("environment: release", workflow)
self.assertIn("actions/create-github-app-token@v2", workflow)
self.assertIn("HOMEBREW_TAP_APP_ID", workflow)
self.assertIn("HOMEBREW_TAP_APP_PRIVATE_KEY", workflow)
self.assertIn("HOMEBREW_TAP_REPOSITORY", workflow)
self.assertNotIn("cargo build", workflow)
self.assertNotIn("pull_request_target", workflow)
```

- [ ] **Step 2: Run the test and verify failure**

```bash
python3 -m unittest distribution.homebrew.test_workflow -v
```

Expected: FAIL because the workflow does not exist.

- [ ] **Step 3: Implement build, test, and publish jobs**

The workflow MUST:

- accept `version` and boolean `publish` inputs;
- fetch manifests only from the fixed public CDN root
  `https://static.lexiang-asset.com/download/app/lexiang-desktop`;
- set explicit minimal `permissions: contents: read`;
- decode `vars.LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64` into a temporary key;
- verify `mac-arm64` and `mac-x64` manifests and artifacts;
- render `Formula/lx.rb`;
- upload the formula as an Actions artifact;
- test on `macos-latest` with `brew audit --strict` and a local install;
- run the publish job only when `publish == true`;
- attach the publish job to the protected `release` environment;
- use `actions/create-github-app-token@v2` with
  `${{ vars.HOMEBREW_TAP_APP_ID }}` and
  `${{ secrets.HOMEBREW_TAP_APP_PRIVATE_KEY }}` to mint a short-lived token
  scoped to `tencent-lexiang/homebrew-tap`;
- clone `${{ vars.HOMEBREW_TAP_REPOSITORY }}` with that installation token;
- copy the already-tested formula, commit `lx <version>`, and push; and
- use concurrency key `homebrew-lx-release`.

No untrusted input may be interpolated into `run:` shell source. Pass inputs
through environment variables and validate version with
`^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$`.

- [ ] **Step 4: Run contract and action lint**

```bash
python3 -m unittest distribution.homebrew.test_workflow -v
actionlint .github/workflows/homebrew-release.yml
```

Expected: tests pass and `actionlint` exits 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/homebrew-release.yml distribution/homebrew
git commit -m "ci: 添加 Homebrew 发布流程"
```

### Task 5: Build a thin Windows NuGet global-tool wrapper

**Files:**

- Create: `distribution/nuget/TencentLexiang.Cli/TencentLexiang.Cli.csproj`
- Create: `distribution/nuget/TencentLexiang.Cli/Program.cs`
- Create: `distribution/nuget/TencentLexiang.Cli.Tests/TencentLexiang.Cli.Tests.csproj`
- Create: `distribution/nuget/TencentLexiang.Cli.Tests/LauncherTests.cs`
- Create: `distribution/nuget/README.md`

- [ ] **Step 1: Write failing launcher tests**

Create a launcher abstraction with:

```csharp
internal static int RunNative(
    string binaryPath,
    IReadOnlyList<string> arguments,
    Func<ProcessStartInfo, Process> start)
```

Tests MUST prove:

- every argument is added through `ProcessStartInfo.ArgumentList`;
- `UseShellExecute` is false;
- stdin/stdout/stderr are not redirected;
- the native exit code is returned unchanged; and
- a missing packaged `native/lx.exe` prints an actionable reinstall message and
  returns non-zero.

- [ ] **Step 2: Run tests and verify failure**

```bash
dotnet test distribution/nuget/TencentLexiang.Cli.Tests/TencentLexiang.Cli.Tests.csproj
```

Expected: FAIL because the launcher project does not exist.

- [ ] **Step 3: Implement the .NET tool package**

Configure the project with:

```xml
<TargetFramework>net8.0</TargetFramework>
<OutputType>Exe</OutputType>
<PackAsTool>true</PackAsTool>
<ToolCommandName>lx</ToolCommandName>
<PackageId>TencentLexiang.Cli</PackageId>
<PackageLicenseExpression>MIT</PackageLicenseExpression>
<PackageRequireLicenseAcceptance>false</PackageRequireLicenseAcceptance>
```

Require `-p:LxBinaryPath=<absolute lx.exe>` during pack and include it as
`tools/net8.0/any/native/lx.exe`. `Program.cs` locates that file relative to
`AppContext.BaseDirectory`, starts it with inherited stdio, and returns its exit
code. It MUST NOT implement auth, update, tool, or protocol behavior.

- [ ] **Step 4: Run tests and inspect a local package**

```bash
dotnet test distribution/nuget/TencentLexiang.Cli.Tests/TencentLexiang.Cli.Tests.csproj
dotnet pack distribution/nuget/TencentLexiang.Cli/TencentLexiang.Cli.csproj \
  -c Release \
  -p:PackageVersion=0.0.0-test \
  -p:LxBinaryPath=/usr/bin/true \
  -o /tmp/lexiang-nuget
unzip -l /tmp/lexiang-nuget/TencentLexiang.Cli.0.0.0-test.nupkg
```

Expected: tests pass and package contents contain the managed tool files plus
`native/lx.exe`, with no source tree or credentials.

- [ ] **Step 5: Commit**

```bash
git add distribution/nuget
git commit -m "feat: 添加 NuGet lx 工具包装"
```

### Task 6: Add NuGet dry-run and OIDC publication

**Files:**

- Create: `.github/workflows/nuget-release.yml`
- Create: `distribution/nuget/test_workflow.py`
- Modify: `distribution/nuget/README.md`

- [ ] **Step 1: Write a failing workflow contract test**

Assert the workflow contains:

```python
self.assertIn("id-token: write", workflow)
self.assertIn("uses: NuGet/login@v1", workflow)
self.assertIn("environment: release", workflow)
self.assertIn("NUGET_USER", workflow)
self.assertIn("dotnet nuget push", workflow)
self.assertNotIn("cargo build", workflow)
self.assertNotIn("NUGET_API_KEY:", workflow)
```

- [ ] **Step 2: Run the test and verify failure**

```bash
python3 -m unittest distribution.nuget.test_workflow -v
```

Expected: FAIL because the workflow does not exist.

- [ ] **Step 3: Implement the workflow**

The workflow MUST:

- accept `version` and boolean `publish`;
- fetch manifests only from the fixed public CDN root
  `https://static.lexiang-asset.com/download/app/lexiang-desktop`;
- verify the signed Windows x64 manifest and `lx.exe`;
- run .NET tests and `dotnet pack`;
- install the generated package into a temporary tool path on
  `windows-latest`;
- run the packaged `lx version`;
- upload the `.nupkg` in dry-run mode;
- run publication only after the protected `release` environment is approved;
- request `id-token: write` only in the publication job;
- use `NuGet/login@v1` with `${{ secrets.NUGET_USER }}`;
- push the exact tested package to
  `https://api.nuget.org/v3/index.json`; and
- use concurrency key `nuget-lx-release`.

- [ ] **Step 4: Run workflow validation**

```bash
python3 -m unittest distribution.nuget.test_workflow -v
actionlint .github/workflows/nuget-release.yml
```

Expected: tests pass and `actionlint` exits 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/nuget-release.yml distribution/nuget
git commit -m "ci: 添加 NuGet OIDC 发布流程"
```

### Task 7: Add credential-free CI and credential setup documentation

**Files:**

- Create: `.github/workflows/distribution-ci.yml`
- Create: `docs/releasing/distribution-credentials.md`
- Modify: `README.md`

- [ ] **Step 1: Add a credential-free CI workflow**

Run Python unit tests, Ruby syntax validation, .NET tests, NuGet pack inspection,
and `actionlint` on changes to `distribution/**` or the three distribution
workflows. CI MUST use fake test artifacts and MUST NOT reference the `release`
environment or any secret.

- [ ] **Step 2: Write the operator credential guide**

Document these exact supported paths:

1. Create the public `tencent-lexiang/homebrew-tap` repository.
2. Create an organization-owned GitHub App with `Contents: Read and write`,
   install it only on `tencent-lexiang/homebrew-tap`, generate a private key,
   set `HOMEBREW_TAP_APP_ID` as a `release` environment variable, and store the
   PEM contents as the `release` environment secret
   `HOMEBREW_TAP_APP_PRIVATE_KEY`. The workflow mints a short-lived
   installation token for each publication.
3. Bootstrap alternative: create a fine-grained PAT scoped only to
   `tencent-lexiang/homebrew-tap` with `Contents: Read and write` and
   `Metadata: Read`; set an expiration and store it as the `release`
   environment secret `HOMEBREW_TAP_TOKEN`, then use the documented fallback
   workflow patch.
4. Set `HOMEBREW_TAP_REPOSITORY=tencent-lexiang/homebrew-tap` and
   `LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64` as `release` environment variables.
   Document that `mirrors.tencent.com` is the internal upload origin and MUST
   NOT be configured as a consumer download URL.
5. Create or join the nuget.org organization that will own
   `TencentLexiang.Cli`.
6. Create a nuget.org Trusted Publishing policy with GitHub owner
   `tencent-lexiang`, repository `lexiang-cli`, workflow file
   `nuget-release.yml`, and environment `release`.
7. Store the nuget.org profile name, not email, as the `release` environment
   secret `NUGET_USER`.
8. Configure required reviewers and prevent self-review on the GitHub
   `release` environment.
9. Only if Trusted Publishing is unavailable, create a scoped NuGet API key
   limited to pushing new versions of `TencentLexiang.Cli`, store it as
   `NUGET_API_KEY`, and use the documented fallback workflow patch.

Link directly to:

- <https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap>
- <https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens>
- <https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments>
- <https://learn.microsoft.com/en-us/nuget/nuget-org/trusted-publishing>
- <https://learn.microsoft.com/en-us/nuget/nuget-org/publish-a-package>

- [ ] **Step 3: Update public installation docs**

Add:

```bash
brew install tencent-lexiang/tap/lx
dotnet tool install --global TencentLexiang.Cli
```

State that both install the native CLI built by `lexiang-desktop` and that this
repository contains distribution adapters, not the canonical native source.

- [ ] **Step 4: Run documentation and workflow checks**

```bash
python3 -m unittest discover -s distribution -p 'test_*.py' -v
actionlint .github/workflows/*.yml
npx --yes markdownlint-cli2@0.17.2 README.md docs/releasing/*.md distribution/**/*.md
```

Expected: all commands exit 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/distribution-ci.yml docs/releasing README.md
git commit -m "docs: 说明 CLI 分发凭证配置"
```

### Task 8: Run final verification without publishing

**Files:**

- Modify: `openspec/changes/add-cli-distribution-workflows/tasks.md`

- [ ] **Step 1: Run the complete local test suite**

```bash
python3 -m unittest discover -s distribution -p 'test_*.py' -v
actionlint .github/workflows/*.yml
npx --yes markdownlint-cli2@0.17.2 README.md docs/releasing/*.md distribution/**/*.md
cargo fmt --all -- --check
cargo test --workspace
openspec validate add-cli-distribution-workflows --strict
git diff --check
```

Expected: all available commands pass. If `dotnet` is unavailable locally,
record the NuGet build and install checks as `not-run` until the GitHub Windows
CI job completes; do not claim local NuGet verification.

- [ ] **Step 2: Run both workflows in dry-run mode**

Manually dispatch Homebrew and NuGet workflows with `publish=false` against an
exact desktop CLI release.

Expected:

- Homebrew run uploads a tested `lx.rb`;
- NuGet run uploads a tested `.nupkg`;
- neither run requests publication credentials; and
- no external repository or registry changes.

- [ ] **Step 3: Inspect dry-run artifacts**

Verify:

```text
Formula/lx.rb contains the exact desktop URLs and digests.
TencentLexiang.Cli.<version>.nupkg contains only the managed launcher and lx.exe.
Both packaged lx commands report the requested CLI version.
```

- [ ] **Step 4: Update the OpenSpec task ledger**

Mark only evidence-backed tasks complete. Leave real Homebrew and NuGet
publication unchecked until credentials are configured and one approved
release succeeds.

- [ ] **Step 5: Commit final verification state**

```bash
git add openspec/changes/add-cli-distribution-workflows/tasks.md
git commit -m "test: 验证 CLI 分发工作流"
```
