# Homebrew distribution

The public organization tap is
[`tencent-lexiang/homebrew-tap`](https://github.com/tencent-lexiang/homebrew-tap).
This repository renders and tests candidates; the protected publication jobs
commit the exact tested Ruby files to the tap.

## CLI Formula

`Formula/lx.rb` installs the native macOS CLI built and signed by
`lexiang-desktop`:

```bash
brew install tencent-lexiang/tap/lx
```

Render a syntax-only fixture:

```bash
python3 -m distribution.homebrew.render_formula \
  --version 0.0.0-test \
  --arm64-url https://static.lexiang-asset.com/test/arm64/lx \
  --arm64-sha256 0000000000000000000000000000000000000000000000000000000000000000 \
  --x64-url https://static.lexiang-asset.com/test/x64/lx \
  --x64-sha256 1111111111111111111111111111111111111111111111111111111111111111 \
  --output /tmp/lx.rb
ruby -c /tmp/lx.rb
```

The release workflow retrieves `manifest.json` and its detached Ed25519
signature from the public CDN. It publishes only after both architecture
binaries pass manifest, size, digest, Formula audit, installation, and
`lx version` checks.

## Desktop Cask

`Casks/lexiang.rb` installs the macOS Desktop App:

```bash
brew install --cask tencent-lexiang/tap/lexiang
```

The Cask workflow takes an exact semantic version and eight-digit build date.
Before rendering it mounts both public CDN DMGs and requires:

- strict Developer ID verification;
- successful Gatekeeper assessment;
- a valid stapled notarization ticket on `TencentLexiang.app`; and
- a valid stapled notarization ticket on the DMG.

The current Desktop builder has notarization disabled. Code and syntax tests
can run, but a real Cask dry run and publication remain blocked until the
Desktop release pipeline produces notarized public DMGs. The workflow never
uses a quarantine bypass.

Run the local contracts with:

```bash
python3 -m unittest \
  distribution.homebrew.test_render_formula \
  distribution.homebrew.test_render_cask \
  distribution.homebrew.test_workflow \
  -v
actionlint \
  .github/workflows/homebrew-release.yml \
  .github/workflows/homebrew-cask-release.yml
```
