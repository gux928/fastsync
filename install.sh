#!/usr/bin/env sh
set -e

REPO_OWNER="gux928"
REPO_NAME="fastsync"
ASSET_LINUX_X64="fastsync-linux-x64.tar.gz"
INSTALL_BIN="fastsync"

PREFIX="/usr/local/bin"
VERSION="latest"

usage() {
  cat <<'EOF'
fastsync installer

Usage:
  curl -fsSL https://raw.githubusercontent.com/gux928/fastsync/master/install.sh | sh
  curl -fsSL https://raw.githubusercontent.com/gux928/fastsync/master/install.sh | sh -s -- --prefix ~/.local/bin
  curl -fsSL https://raw.githubusercontent.com/gux928/fastsync/master/install.sh | sh -s -- --version v0.1.11-test3

Options:
  --prefix <dir>   Install directory (default: /usr/local/bin)
  --version <tag>  Release tag (default: latest)
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix)
      PREFIX="$2"
      shift 2
      ;;
    --version)
      VERSION="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$OS" != "linux" ]; then
  echo "Unsupported OS: $OS (only linux is supported)." >&2
  exit 1
fi

if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "amd64" ]; then
  echo "Unsupported arch: $ARCH (only x86_64/amd64 is supported)." >&2
  exit 1
fi

if [ "$VERSION" = "latest" ]; then
  DOWNLOAD_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${ASSET_LINUX_X64}"
else
  DOWNLOAD_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${VERSION}/${ASSET_LINUX_X64}"
fi

TMP_DIR="$(mktemp -d)"
TARBALL="${TMP_DIR}/${ASSET_LINUX_X64}"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Downloading ${DOWNLOAD_URL}"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TARBALL"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TARBALL" "$DOWNLOAD_URL"
else
  echo "Neither curl nor wget is available." >&2
  exit 1
fi

tar -xzf "$TARBALL" -C "$TMP_DIR"

if [ ! -f "${TMP_DIR}/${INSTALL_BIN}" ]; then
  echo "Binary not found in archive." >&2
  exit 1
fi

mkdir -p "$PREFIX"
if [ -w "$PREFIX" ]; then
  mv "${TMP_DIR}/${INSTALL_BIN}" "${PREFIX}/${INSTALL_BIN}"
else
  echo "No write permission to ${PREFIX}, using sudo."
  sudo mv "${TMP_DIR}/${INSTALL_BIN}" "${PREFIX}/${INSTALL_BIN}"
fi

echo "Installed ${INSTALL_BIN} to ${PREFIX}/${INSTALL_BIN}"
