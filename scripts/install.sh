#!/usr/bin/env bash
# Install fallow-luau from GitHub Releases (or build from source as fallback).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- --version v0.1.0
set -euo pipefail

REPO="${FALLOW_LUAU_REPO:-3xjn/fallow-luau}"
INSTALL_DIR="${FALLOW_LUAU_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${FALLOW_LUAU_VERSION:-}"
PREFERRED_LIBC="${FALLOW_LUAU_LIBC:-}" # gnu | musl

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version|-v)
      VERSION="$2"
      shift 2
      ;;
    --dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    --musl)
      PREFERRED_LIBC="musl"
      shift
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "fallow-luau installer: missing required command: $1" >&2
    exit 1
  }
}

need curl
need tar
need uname
need mktemp

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  aarch64|arm64) arch="aarch64" ;;
  *)
    echo "fallow-luau installer: unsupported arch: $arch" >&2
    exit 1
    ;;
esac

triple=""
case "$os" in
  linux)
    libc="gnu"
    if [[ "$PREFERRED_LIBC" == "musl" ]] || { [[ -z "$PREFERRED_LIBC" ]] && ldd --version 2>&1 | grep -qi musl; }; then
      libc="musl"
    fi
    triple="${arch}-unknown-linux-${libc}"
    ;;
  darwin)
    triple="${arch}-apple-darwin"
    ;;
  *)
    echo "fallow-luau installer: unsupported OS: $os (use install.ps1 on Windows)" >&2
    exit 1
    ;;
esac

api="https://api.github.com/repos/${REPO}/releases"
if [[ -n "$VERSION" ]]; then
  tag="$VERSION"
  [[ "$tag" == v* ]] || tag="v${tag}"
  release_url="${api}/tags/${tag}"
else
  release_url="${api}/latest"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "fallow-luau: fetching release metadata…"
if ! meta="$(curl -fsSL -H "Accept: application/vnd.github+json" -H "User-Agent: fallow-luau-install" "$release_url")"; then
  echo "fallow-luau: no GitHub Release found; trying cargo install --git…" >&2
  if command -v cargo >/dev/null 2>&1; then
    cargo install --git "https://github.com/${REPO}" --locked fallow-luau
    echo "fallow-luau: installed via cargo ($(command -v fallow-luau || true))"
    fallow-luau --version || true
    exit 0
  fi
  echo "fallow-luau: push a tag (v0.1.0) to create Releases, or install Rust and re-run." >&2
  exit 1
fi

tag="$(printf '%s' "$meta" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
asset="fallow-luau-${tag#v}-${triple}.tar.gz"
# Prefer exact asset; some releases may omit the version in the name — try both.
asset_url="$(printf '%s' "$meta" | sed -n "s/.*\"browser_download_url\":[[:space:]]*\"\\([^\"]*${asset}\\)\".*/\\1/p" | head -n1)"
if [[ -z "$asset_url" ]]; then
  asset="fallow-luau-${triple}.tar.gz"
  asset_url="$(printf '%s' "$meta" | sed -n "s/.*\"browser_download_url\":[[:space:]]*\"\\([^\"]*${asset}\\)\".*/\\1/p" | head -n1)"
fi

if [[ -z "$asset_url" ]]; then
  echo "fallow-luau: no asset for ${triple} in ${tag}" >&2
  echo "fallow-luau: available assets are listed on https://github.com/${REPO}/releases/tag/${tag}" >&2
  exit 1
fi

echo "fallow-luau: downloading ${asset_url}"
curl -fsSL -o "${tmp}/pkg.tar.gz" "$asset_url"
tar -xzf "${tmp}/pkg.tar.gz" -C "$tmp"

bin=""
if [[ -x "${tmp}/fallow-luau" ]]; then
  bin="${tmp}/fallow-luau"
else
  bin="$(find "$tmp" -type f -name fallow-luau | head -n1 || true)"
fi
if [[ -z "$bin" || ! -f "$bin" ]]; then
  echo "fallow-luau: archive did not contain fallow-luau binary" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 755 "$bin" "${INSTALL_DIR}/fallow-luau"

echo "fallow-luau: installed ${INSTALL_DIR}/fallow-luau (${tag})"
case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "fallow-luau: add to PATH, e.g.  export PATH=\"${INSTALL_DIR}:\$PATH\"" >&2
    ;;
esac
"${INSTALL_DIR}/fallow-luau" --version
