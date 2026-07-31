import base64
import hashlib
import json
import os
import re
import subprocess
import tempfile
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, FrozenSet, Iterable
from urllib.parse import unquote, urlparse


DEFAULT_TIMEOUT_SECONDS = 30
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


class ManifestVerificationError(RuntimeError):
    """Raised when release metadata or its artifact cannot be trusted."""


@dataclass(frozen=True)
class FetchResult:
    data: bytes
    final_url: str


@dataclass(frozen=True)
class VerifiedArtifact:
    version: str
    platform: str
    arch: str
    url: str
    size: int
    sha256: str
    binary: Path


Fetcher = Callable[[str, int], FetchResult]
OpenSSLRunner = Callable[[Iterable[str]], subprocess.CompletedProcess]


def _fetch(url: str, timeout: int) -> FetchResult:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "tencent-lexiang-distribution/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return FetchResult(response.read(), response.geturl())
    except Exception as error:
        raise ManifestVerificationError(f"download failed for {url}: {error}") from error


def _run_openssl(args: Iterable[str]) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(
            list(args),
            check=False,
            capture_output=True,
        )
    except OSError as error:
        raise ManifestVerificationError(
            f"unable to execute OpenSSL signature verification: {error}"
        ) from error


def _normalized_hosts(hosts: Iterable[str]) -> FrozenSet[str]:
    normalized = frozenset(host.strip().lower() for host in hosts if host.strip())
    if not normalized:
        raise ManifestVerificationError("at least one approved host is required")
    return normalized


def _validate_https_url(url: str, allowed_hosts: FrozenSet[str], label: str) -> None:
    parsed = urlparse(url)
    if parsed.scheme != "https":
        raise ManifestVerificationError(f"{label} must use HTTPS")
    if not parsed.hostname or parsed.hostname.lower() not in allowed_hosts:
        raise ManifestVerificationError(f"{label} must use an approved host")
    if parsed.username or parsed.password:
        raise ManifestVerificationError(f"{label} must not contain credentials")


def _fetch_checked(
    url: str,
    *,
    allowed_hosts: FrozenSet[str],
    label: str,
    timeout: int,
    fetcher: Fetcher,
) -> FetchResult:
    _validate_https_url(url, allowed_hosts, label)
    try:
        result = fetcher(url, timeout)
    except ManifestVerificationError:
        raise
    except Exception as error:
        raise ManifestVerificationError(f"download failed for {label}: {error}") from error
    _validate_https_url(result.final_url, allowed_hosts, f"{label} redirect")
    return result


def _require_manifest_value(manifest, key, expected):
    actual = manifest.get(key)
    if actual != expected:
        raise ManifestVerificationError(
            f"manifest {key} mismatch: expected {expected!r}, got {actual!r}"
        )


def _verify_signature(
    *,
    raw_manifest: bytes,
    raw_signature: bytes,
    public_key: Path,
    work_dir: Path,
    openssl_runner: OpenSSLRunner,
) -> None:
    try:
        signature = base64.b64decode(raw_signature.strip(), validate=True)
    except (ValueError, TypeError) as error:
        raise ManifestVerificationError("manifest signature is not valid base64") from error
    if not signature:
        raise ManifestVerificationError("manifest signature is empty")

    manifest_path = work_dir / "manifest.json"
    signature_path = work_dir / "manifest.json.sig.bin"
    manifest_path.write_bytes(raw_manifest)
    signature_path.write_bytes(signature)
    result = openssl_runner(
        [
            "openssl",
            "pkeyutl",
            "-verify",
            "-pubin",
            "-inkey",
            str(public_key),
            "-rawin",
            "-in",
            str(manifest_path),
            "-sigfile",
            str(signature_path),
        ]
    )
    if result.returncode != 0:
        raise ManifestVerificationError("manifest signature verification failed")


def fetch_and_verify(
    *,
    manifest_url: str,
    signature_url: str,
    public_key: Path,
    expected_version: str,
    expected_platform: str,
    expected_arch: str,
    allowed_hosts: set[str],
    output_dir: Path,
    fetcher: Fetcher = _fetch,
    openssl_runner: OpenSSLRunner = _run_openssl,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
) -> VerifiedArtifact:
    approved_hosts = _normalized_hosts(allowed_hosts)
    public_key = Path(public_key)
    output_dir = Path(output_dir)
    if not public_key.is_file():
        raise ManifestVerificationError(f"public key does not exist: {public_key}")
    if timeout <= 0:
        raise ManifestVerificationError("download timeout must be positive")

    manifest_result = _fetch_checked(
        manifest_url,
        allowed_hosts=approved_hosts,
        label="manifest URL",
        timeout=timeout,
        fetcher=fetcher,
    )
    signature_result = _fetch_checked(
        signature_url,
        allowed_hosts=approved_hosts,
        label="signature URL",
        timeout=timeout,
        fetcher=fetcher,
    )

    try:
        manifest = json.loads(manifest_result.data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ManifestVerificationError("manifest is not valid JSON") from error
    if not isinstance(manifest, dict):
        raise ManifestVerificationError("manifest root must be an object")

    output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".verify-", dir=output_dir) as temporary:
        work_dir = Path(temporary)
        _verify_signature(
            raw_manifest=manifest_result.data,
            raw_signature=signature_result.data,
            public_key=public_key,
            work_dir=work_dir,
            openssl_runner=openssl_runner,
        )

        _require_manifest_value(manifest, "schemaVersion", 1)
        _require_manifest_value(manifest, "component", "lx-cli")
        _require_manifest_value(manifest, "version", expected_version)
        _require_manifest_value(manifest, "platform", expected_platform)
        _require_manifest_value(manifest, "arch", expected_arch)

        artifact_url = manifest.get("url")
        size = manifest.get("size")
        digest = manifest.get("sha256")
        if not isinstance(artifact_url, str):
            raise ManifestVerificationError("manifest url must be a string")
        _validate_https_url(artifact_url, approved_hosts, "artifact URL")
        if not isinstance(size, int) or isinstance(size, bool) or size < 0:
            raise ManifestVerificationError("manifest size must be a non-negative integer")
        if not isinstance(digest, str) or not SHA256_PATTERN.fullmatch(digest):
            raise ManifestVerificationError(
                "manifest SHA-256 must be 64 lowercase hexadecimal characters"
            )

        expected_name = "lx.exe" if expected_platform == "win32" else "lx"
        artifact_name = Path(unquote(urlparse(artifact_url).path)).name
        if artifact_name != expected_name:
            raise ManifestVerificationError(
                f"artifact filename mismatch: expected {expected_name!r}"
            )

        artifact_result = _fetch_checked(
            artifact_url,
            allowed_hosts=approved_hosts,
            label="artifact URL",
            timeout=timeout,
            fetcher=fetcher,
        )
        temporary_binary = work_dir / expected_name
        temporary_binary.write_bytes(artifact_result.data)
        if len(artifact_result.data) != size:
            raise ManifestVerificationError(
                f"artifact size mismatch: expected {size}, got {len(artifact_result.data)}"
            )
        actual_digest = hashlib.sha256(artifact_result.data).hexdigest()
        if actual_digest != digest:
            raise ManifestVerificationError(
                f"artifact SHA-256 mismatch: expected {digest}, got {actual_digest}"
            )

        final_binary = output_dir / expected_name
        os.replace(temporary_binary, final_binary)
        if expected_platform != "win32":
            final_binary.chmod(0o755)

    return VerifiedArtifact(
        version=expected_version,
        platform=expected_platform,
        arch=expected_arch,
        url=artifact_url,
        size=size,
        sha256=digest,
        binary=final_binary,
    )
