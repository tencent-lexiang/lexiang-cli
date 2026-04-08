#!/bin/sh
# lx installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/tencent-lexiang/lexiang-cli/main/scripts/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/tencent-lexiang/lexiang-cli/main/scripts/install.sh | sh -s -- --dir /usr/local/bin
#   curl -fsSL https://raw.githubusercontent.com/tencent-lexiang/lexiang-cli/main/scripts/install.sh | sh -s -- --version 0.0.1-delta

set -eu

REPO="tencent-lexiang/lexiang-cli"
BINARY_NAME="lx"
INSTALL_DIR="${HOME}/.local/bin"
VERSION="latest"

info() {
  printf '[INFO]  %s\n' "$*"
}

warn() {
  printf '[WARN]  %s\n' "$*" >&2
}

error() {
  printf '[ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: install.sh [--dir <install-dir>] [--version <version>]

Options:
  --dir <path>       Install directory (default: ~/.local/bin)
  --version <ver>    Install a specific release version, e.g. 0.0.1-delta
  -h, --help         Show this help message
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dir)
      [ "$#" -ge 2 ] || error "--dir requires a value"
      INSTALL_DIR="$2"
      shift 2
      ;;
    --dir=*)
      INSTALL_DIR=${1#--dir=}
      shift
      ;;
    --version)
      [ "$#" -ge 2 ] || error "--version requires a value"
      VERSION="$2"
      shift 2
      ;;
    --version=*)
      VERSION=${1#--version=}
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      error "unknown argument: $1"
      ;;
  esac
done

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || error "missing required command: $1"
}

sha256_file() {
  file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
    return
  fi

  error "missing SHA256 tool: sha256sum or shasum"
}

detect_asset() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Darwin)
      echo "lx-macos-universal"
      return
      ;;
    Linux)
      ;;
    *)
      error "unsupported operating system: $os"
      ;;
  esac

  case "$arch" in
    x86_64|amd64)
      echo "lx-linux-x86_64"
      ;;
    aarch64|arm64)
      echo "lx-linux-arm64"
      ;;
    *)
      error "unsupported architecture: $arch"
      ;;
  esac
}

build_tag_from_version() {
  version="$1"

  case "$version" in
    cli-v*)
      printf '%s' "$version"
      ;;
    *)
      printf 'cli-v%s' "$version"
      ;;
  esac
}

build_download_url() {
  asset="$1"

  if [ "$VERSION" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download/%s' "$REPO" "$asset"
    return
  fi

  tag=$(build_tag_from_version "$VERSION")

  printf 'https://github.com/%s/releases/download/%s/%s' "$REPO" "$tag" "$asset"
}

build_checksums_url() {
  if [ "$VERSION" = "latest" ]; then
    printf 'https://github.com/%s/releases/latest/download/SHA256SUMS.txt' "$REPO"
    return
  fi

  tag=$(build_tag_from_version "$VERSION")

  printf 'https://github.com/%s/releases/download/%s/SHA256SUMS.txt' "$REPO" "$tag"
}

verify_checksum() {
  asset="$1"
  file="$2"
  checksums_file="$3"

  expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$checksums_file")
  [ -n "$expected" ] || error "checksum for $asset not found in SHA256SUMS.txt"

  actual=$(sha256_file "$file")
  [ "$expected" = "$actual" ] || error "checksum mismatch for $asset"
}

main() {
  need_cmd curl
  need_cmd uname
  need_cmd awk
  need_cmd chmod
  need_cmd mkdir
  need_cmd mv
  need_cmd rm
  need_cmd mktemp

  asset=$(detect_asset)
  download_url=$(build_download_url "$asset")
  checksums_url=$(build_checksums_url)

  info "detected asset: $asset"
  info "downloading binary..."

  tmp_dir=$(mktemp -d)
  trap 'rm -rf "$tmp_dir"' EXIT INT TERM

  binary_path="$tmp_dir/$asset"
  checksums_path="$tmp_dir/SHA256SUMS.txt"

  curl --fail --location --silent --show-error "$download_url" -o "$binary_path"
  curl --fail --location --silent --show-error "$checksums_url" -o "$checksums_path"

  info "verifying checksum..."
  verify_checksum "$asset" "$binary_path" "$checksums_path"

  mkdir -p "$INSTALL_DIR"
  target="$INSTALL_DIR/$BINARY_NAME"
  mv "$binary_path" "$target"
  chmod +x "$target"

  info "installed to: $target"

  case ":$PATH:" in
    *":$INSTALL_DIR:"*)
      ;;
    *)
      warn "$INSTALL_DIR is not in PATH"
      warn "add it to your shell profile, for example: export PATH=\"$INSTALL_DIR:\$PATH\""
      ;;
  esac

  info "run 'lx version' to verify installation"
}

main
