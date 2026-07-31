# Distribution credentials

The Formula, Cask, and NuGet workflows build and test candidates without
publication credentials. Secrets are available only to jobs attached to the
protected GitHub `release` Environment.

## Public artifact boundary

Consumers download only from:

```text
https://static.lexiang-asset.com/download/app/lexiang-desktop
```

`mirrors.tencent.com` is the internal Desktop upload origin. Do not configure
it in Homebrew or NuGet consumer workflows.

The Desktop release system owns the Ed25519 signing private key. This
repository receives only the public key used to verify CLI manifests.

## Repository variables

Configure these as repository-level Actions variables in
`tencent-lexiang/lexiang-cli` so credential-free jobs can read public
verification material:

| Variable | Value |
| --- | --- |
| `HOMEBREW_TAP_REPOSITORY` | `tencent-lexiang/homebrew-tap` |
| `HOMEBREW_TAP_APP_ID` | GitHub App numeric ID |
| `LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64` | Base64 of the Ed25519 public-key PEM |

Example:

```bash
gh variable set HOMEBREW_TAP_REPOSITORY \
  --repo tencent-lexiang/lexiang-cli \
  --body tencent-lexiang/homebrew-tap
gh variable set HOMEBREW_TAP_APP_ID \
  --repo tencent-lexiang/lexiang-cli \
  --body '<github-app-id>'
base64 < /absolute/path/to/cli-release-public-key.pem | tr -d '\n' |
  gh variable set LEXIANG_CLI_RELEASE_PUBLIC_KEY_B64 \
    --repo tencent-lexiang/lexiang-cli
```

If no signing pair exists yet, generate it in the secured Desktop release
environment:

```bash
openssl genpkey -algorithm Ed25519 -out cli-release-private-key.pem
openssl pkey \
  -in cli-release-private-key.pem \
  -pubout \
  -out cli-release-public-key.pem
```

Store `cli-release-private-key.pem` only in the Desktop release system. Never
upload it to this repository, the tap, a workflow artifact, or a GitHub
variable.

## Protected release Environment

In `tencent-lexiang/lexiang-cli`, open **Settings → Environments** and create
`release`.

Configure:

- required reviewers from the release-maintainer team;
- prevention of self-review;
- deployment branches restricted to the protected `main` branch; and
- no administrator bypass for normal releases.

Environment secrets are unavailable until a reviewer approves the job. See
[GitHub deployment environments](https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments).

## Homebrew GitHub App

Create an organization-owned GitHub App such as
`lexiang-homebrew-publisher`.

Configuration:

- Webhooks: inactive.
- Repository permission `Contents`: **Read and write**.
- All other repository and organization permissions: none unless GitHub
  requires read-only metadata.
- Installation target: only `tencent-lexiang/homebrew-tap`.

Generate a private key, download the PEM once, and store it directly as an
Environment secret:

```bash
gh secret set HOMEBREW_TAP_APP_PRIVATE_KEY \
  --repo tencent-lexiang/lexiang-cli \
  --env release \
  < /absolute/path/to/github-app-private-key.pem
```

Record its App ID in `HOMEBREW_TAP_APP_ID`. The workflows use
`actions/create-github-app-token@v2` to mint a short-lived installation token
for each approved publication. GitHub's built-in `GITHUB_TOKEN` cannot write
the separate tap repository.

Rotate the App private key by generating a new key, replacing the Environment
secret, performing a dry run, and then revoking the previous key.

GitHub documents this cross-repository flow in
[Making authenticated API requests with a GitHub App](https://docs.github.com/en/enterprise-cloud%40latest/apps/creating-github-apps/authenticating-with-a-github-app/making-authenticated-api-requests-with-a-github-app-in-a-github-actions-workflow).

### Temporary PAT fallback

If organization policy temporarily blocks GitHub App creation, create a
fine-grained personal access token with:

- resource owner `tencent-lexiang`;
- repository access limited to `homebrew-tap`;
- `Contents: Read and write`;
- `Metadata: Read`; and
- the shortest operational expiration.

Store it as the `release` Environment secret `HOMEBREW_TAP_TOKEN`. Before use,
review and deliberately patch the publication job to replace the GitHub App
token step. Do not configure both paths simultaneously. Return to the GitHub
App path and revoke the PAT as soon as possible. See
[Managing fine-grained personal access tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).

## NuGet Trusted Publishing

Create or join the nuget.org organization that will own
`TencentLexiang.Cli`. In nuget.org **Trusted Publishing**, add a GitHub Actions
policy owned by that organization:

| Field | Value |
| --- | --- |
| Repository owner | `tencent-lexiang` |
| Repository | `lexiang-cli` |
| Workflow file | `nuget-release.yml` |
| Environment | `release` |

Enter only the workflow filename, not `.github/workflows/nuget-release.yml`.
Pending policies can be activated by the first matching publication.

Store the nuget.org profile name, not its email address:

```bash
gh secret set NUGET_USER \
  --repo tencent-lexiang/lexiang-cli \
  --env release \
  --body '<nuget-profile-name>'
```

The protected job requests `id-token: write`, and `NuGet/login@v1` exchanges
the GitHub OIDC identity for a temporary API key immediately before
`dotnet nuget push`. See
[NuGet Trusted Publishing](https://learn.microsoft.com/en-us/nuget/nuget-org/trusted-publishing).

### Scoped API-key fallback

Only when Trusted Publishing is unavailable, create a nuget.org API key with
`Push` permission and package glob `TencentLexiang.Cli`, set an expiration, and
store it as the `release` Environment secret `NUGET_API_KEY`. Deliberately
patch the workflow to use that key, then remove the patch and revoke the key
after Trusted Publishing becomes available. See
[Publishing NuGet packages](https://learn.microsoft.com/en-us/nuget/nuget-org/publish-a-package).

## Safe rollout

1. Publish signed CLI artifacts and notarized Desktop DMGs to the public CDN.
2. Run all three workflows with `publish=false`.
3. Inspect `Formula/lx.rb`, `Casks/lexiang.rb`, and the `.nupkg`.
4. Configure the GitHub App and NuGet Trusted Publishing.
5. Approve one release at a time.
6. Verify clean Homebrew and NuGet installations.

Homebrew rollback is a tap commit revert. NuGet versions are immutable; unlist
a faulty version and publish a corrected version.
