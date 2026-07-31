import re
from urllib.parse import urlparse


SEMVER_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-.][0-9A-Za-z.-]+)?$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
DATE_STAMP_PATTERN = re.compile(r"^[0-9]{8}$")
CDN_HOST = "static.lexiang-asset.com"


def validate_version(version: str) -> None:
    if not SEMVER_PATTERN.fullmatch(version):
        raise ValueError("version must be a semantic version")


def validate_sha256(value: str, label: str) -> None:
    if not SHA256_PATTERN.fullmatch(value):
        raise ValueError(f"{label} SHA-256 must be 64 lowercase hexadecimal characters")


def validate_cdn_url(url: str, label: str) -> None:
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.hostname != CDN_HOST:
        raise ValueError(f"{label} must use the Lexiang public CDN")
    if parsed.username or parsed.password:
        raise ValueError(f"{label} must not contain credentials")


def validate_date_stamp(date_stamp: str) -> None:
    if not DATE_STAMP_PATTERN.fullmatch(date_stamp):
        raise ValueError("date stamp must contain exactly eight digits")
