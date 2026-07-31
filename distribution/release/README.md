# Desktop CLI release contract

This directory verifies native `lx` artifacts produced and signed by
`lexiang-desktop`. It never builds or modifies the native executable.

The production release root is:

```text
https://static.lexiang-asset.com/download/app/lexiang-desktop
```

Target mapping:

| Consumer | Directory | Platform | Architecture | Binary |
| --- | --- | --- | --- | --- |
| Homebrew Apple Silicon | `mac-arm64` | `darwin` | `arm64` | `lx` |
| Homebrew Intel | `mac-x64` | `darwin` | `x64` | `lx` |
| NuGet Windows | `win` | `win32` | `x64` | `lx.exe` |

Each directory publishes `cli/manifest.json` and
`cli/manifest.json.sig`. The detached signature is base64-encoded Ed25519 over
the exact manifest bytes. A versioned manifest contains:

```json
{
  "schemaVersion": 1,
  "component": "lx-cli",
  "version": "1.2.3",
  "platform": "darwin",
  "arch": "arm64",
  "url": "https://static.lexiang-asset.com/download/app/lexiang-desktop/mac-arm64/cli/1.2.3/lx",
  "size": 123,
  "sha256": "64 lowercase hexadecimal characters"
}
```

Verification fails closed unless the signature, schema, component, requested
version and target, HTTPS host, redirect destinations, byte size, and SHA-256
all match. The only production download host is
`static.lexiang-asset.com`; `mirrors.tencent.com` is an internal upload origin
and is not an accepted consumer URL.

Run the contract tests with:

```bash
python3 -m unittest distribution.release.test_manifest -v
```
