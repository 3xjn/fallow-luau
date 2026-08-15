#!/usr/bin/env bash
# curl -fsSL https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.sh | bash
set -euo pipefail
REPO=3xjn/fallow-luau
DIR="${FALLOW_LUAU_INSTALL_DIR:-$HOME/.local/bin}"
arch=$(uname -m); case $arch in x86_64|amd64) arch=x86_64;; aarch64|arm64) arch=aarch64;; *) exit 1;; esac
case $(uname -s) in
  Linux*) t=$arch-unknown-linux-gnu;;
  Darwin*) t=$arch-apple-darwin;;
  *) echo "use install.ps1 on Windows" >&2; exit 1;;
esac
tag=$(curl -fsSL https://api.github.com/repos/$REPO/releases/latest | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
curl -fsSL "https://github.com/$REPO/releases/download/$tag/fallow-luau-${tag#v}-$t.tar.gz" | tar -xz -C "$tmp"
mkdir -p "$DIR"
install -m755 "$(find "$tmp" -type f -name fallow-luau | head -1)" "$DIR/fallow-luau"
echo "installed $DIR/fallow-luau ($tag)"
