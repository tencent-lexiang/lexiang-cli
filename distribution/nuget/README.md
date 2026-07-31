# NuGet CLI distribution

`TencentLexiang.Cli` is a thin .NET 8 global-tool launcher for the verified
Windows x64 `lx.exe` produced by `lexiang-desktop`.

It does not implement authentication, updates, protocols, or CLI behavior. It
forwards every argument with `ProcessStartInfo.ArgumentList`, inherits standard
streams, waits for the native process, and returns its exit code.

Install the published tool with:

```bash
dotnet tool install --global TencentLexiang.Cli
```

Run the launcher tests:

```bash
dotnet test \
  distribution/nuget/TencentLexiang.Cli.Tests/TencentLexiang.Cli.Tests.csproj
```

Packing requires an explicit verified native executable:

```bash
dotnet pack \
  distribution/nuget/TencentLexiang.Cli/TencentLexiang.Cli.csproj \
  --configuration Release \
  -p:PackageVersion=0.0.0-test \
  -p:LxBinaryPath=/absolute/path/to/lx.exe \
  --output /tmp/lexiang-nuget
```

The pack fails when `LxBinaryPath` is missing or does not exist. The production
workflow verifies the signed Windows manifest before passing the binary to
MSBuild, installs the generated package into a temporary tool directory, and
runs `lx version`.

Normal publication uses nuget.org Trusted Publishing. Only the protected
publish job requests `id-token: write`; `NuGet/login@v1` exchanges that OIDC
identity for a temporary API key immediately before pushing the exact tested
package.
