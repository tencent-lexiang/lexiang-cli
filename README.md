# Lexiang CLI Distribution

This repository contains the public distribution configuration for `lx` and
the VS Code extension source while it is being migrated.

The native CLI is built and published by `lexiang-desktop`. Release workflows
in this repository download immutable binaries from the configured release
origin.

## Install

macOS:

```bash
brew install tencent-lexiang/tap/lx
```

Windows:

```powershell
dotnet tool install --global TencentLexiang.Cli
```

## Release workflows

- `homebrew-release.yml` downloads both macOS architectures, generates and
  audits `Formula/lx.rb`, installs it, and optionally updates
  `tencent-lexiang/homebrew-tap`.
- `nuget-release.yml` downloads Windows `lx.exe`, builds and installs the
  `TencentLexiang.Cli` global tool, and optionally publishes it to nuget.org.

Both workflows default to `publish=false`. Packaging and installation checks
run without publication credentials.

## GitHub configuration

Create a protected GitHub Environment named `release`.

Configure repository Actions secret `LEXIANG_CLI_CDN_ROOT` with the CLI release
root. It is used only by manually dispatched packaging jobs and is never
printed by the workflows.

For Homebrew, create a GitHub App installed only on
`tencent-lexiang/homebrew-tap` with `Contents: Read and write`, then configure:

- repository variable `HOMEBREW_TAP_APP_ID`;
- `release` Environment secret `HOMEBREW_TAP_APP_PRIVATE_KEY`.

For NuGet, create a nuget.org Trusted Publishing policy for:

- owner: `tencent-lexiang`;
- repository: `lexiang-cli`;
- workflow: `nuget-release.yml`;
- environment: `release`.

Configure the `release` Environment secret `NUGET_USER` with the nuget.org
profile name.
