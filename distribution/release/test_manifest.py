import base64
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from distribution.release.manifest import (
    FetchResult,
    ManifestVerificationError,
    fetch_and_verify,
)


MANIFEST_URL = (
    "https://static.lexiang-asset.com/download/app/"
    "lexiang-desktop/mac-arm64/cli/manifest.json"
)
SIGNATURE_URL = f"{MANIFEST_URL}.sig"
ARTIFACT_URL = (
    "https://static.lexiang-asset.com/download/app/"
    "lexiang-desktop/mac-arm64/cli/1.2.3/lx"
)


class FakeFetcher:
    def __init__(self, responses):
        self.responses = responses
        self.requests = []

    def __call__(self, url, timeout):
        self.requests.append((url, timeout))
        response = self.responses[url]
        if isinstance(response, Exception):
            raise response
        return response


def successful_openssl(_args):
    return subprocess.CompletedProcess([], 0, stdout=b"", stderr=b"")


class ManifestVerificationTests(unittest.TestCase):
    def setUp(self):
        self.artifact = b"desktop-owned-lx"
        self.manifest = {
            "schemaVersion": 1,
            "component": "lx-cli",
            "version": "1.2.3",
            "platform": "darwin",
            "arch": "arm64",
            "url": ARTIFACT_URL,
            "size": len(self.artifact),
            "sha256": hashlib.sha256(self.artifact).hexdigest(),
        }
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.public_key = self.root / "public.pem"
        self.public_key.write_text("test public key", encoding="utf-8")

    def fetcher(self, *, manifest=None, artifact=None, final_artifact_url=None):
        raw_manifest = json.dumps(
            manifest or self.manifest, separators=(",", ":")
        ).encode()
        return FakeFetcher(
            {
                MANIFEST_URL: FetchResult(raw_manifest, MANIFEST_URL),
                SIGNATURE_URL: FetchResult(
                    base64.b64encode(b"detached-signature") + b"\n",
                    SIGNATURE_URL,
                ),
                ARTIFACT_URL: FetchResult(
                    self.artifact if artifact is None else artifact,
                    final_artifact_url or ARTIFACT_URL,
                ),
            }
        )

    def verify(self, *, fetcher=None, openssl_runner=successful_openssl):
        return fetch_and_verify(
            manifest_url=MANIFEST_URL,
            signature_url=SIGNATURE_URL,
            public_key=self.public_key,
            expected_version="1.2.3",
            expected_platform="darwin",
            expected_arch="arm64",
            allowed_hosts={"static.lexiang-asset.com"},
            output_dir=self.root / "output",
            fetcher=fetcher or self.fetcher(),
            openssl_runner=openssl_runner,
        )

    def assert_rejected(self, pattern, **kwargs):
        with self.assertRaisesRegex(ManifestVerificationError, pattern):
            self.verify(**kwargs)
        output = self.root / "output"
        self.assertFalse(output.exists() and any(output.iterdir()))

    def test_rejects_http_artifact_url(self):
        manifest = {**self.manifest, "url": ARTIFACT_URL.replace("https://", "http://")}
        self.assert_rejected("HTTPS", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_unapproved_artifact_host(self):
        redirected = "https://downloads.example.invalid/lx"
        self.assert_rejected(
            "approved host",
            fetcher=self.fetcher(final_artifact_url=redirected),
        )

    def test_rejects_wrong_schema(self):
        manifest = {**self.manifest, "schemaVersion": 2}
        self.assert_rejected("schemaVersion", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_wrong_component(self):
        manifest = {**self.manifest, "component": "desktop-app"}
        self.assert_rejected("component", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_wrong_version(self):
        manifest = {**self.manifest, "version": "9.9.9"}
        self.assert_rejected("version", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_wrong_platform_or_arch(self):
        for field, value in (("platform", "linux"), ("arch", "x64")):
            with self.subTest(field=field):
                manifest = {**self.manifest, field: value}
                self.assert_rejected(field, fetcher=self.fetcher(manifest=manifest))

    def test_rejects_size_mismatch(self):
        manifest = {**self.manifest, "size": len(self.artifact) + 1}
        self.assert_rejected("size", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_sha256_mismatch(self):
        manifest = {**self.manifest, "sha256": "0" * 64}
        self.assert_rejected("SHA-256", fetcher=self.fetcher(manifest=manifest))

    def test_rejects_invalid_signature(self):
        def failing_openssl(_args):
            return subprocess.CompletedProcess(
                [], 1, stdout=b"", stderr=b"invalid signature"
            )

        self.assert_rejected("signature", openssl_runner=failing_openssl)

    def test_accepts_verified_artifact(self):
        fetcher = self.fetcher()

        verified = self.verify(fetcher=fetcher)

        self.assertEqual(verified.version, "1.2.3")
        self.assertEqual(verified.platform, "darwin")
        self.assertEqual(verified.arch, "arm64")
        self.assertEqual(verified.url, ARTIFACT_URL)
        self.assertEqual(verified.size, len(self.artifact))
        self.assertEqual(verified.sha256, self.manifest["sha256"])
        self.assertEqual(verified.binary, self.root / "output" / "lx")
        self.assertEqual(verified.binary.read_bytes(), self.artifact)
        self.assertEqual(
            [url for url, _timeout in fetcher.requests],
            [MANIFEST_URL, SIGNATURE_URL, ARTIFACT_URL],
        )
        self.assertTrue(all(timeout > 0 for _url, timeout in fetcher.requests))


if __name__ == "__main__":
    unittest.main()
