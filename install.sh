#!/bin/sh
# Installs a prebuilt rsepuh binary (no Rust toolchain needed).
#   curl -fsSL https://raw.githubusercontent.com/igun997/rsepuh/main/install.sh | sh
# rsepuh drives EffectCraft (and Blender) on a remote sandbox through rcargo.
set -eu
repo="igun997/rsepuh"
os=$(uname -s); arch=$(uname -m)
case "$os-$arch" in
  Linux-x86_64)  target=x86_64-unknown-linux-gnu ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu ;;
  Darwin-arm64)  target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *) echo "rsepuh: no prebuilt binary for $os $arch. Build from source: cargo install --git https://github.com/$repo" >&2; exit 1 ;;
esac
dest="${RSEPUH_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$dest"
url="https://github.com/$repo/releases/latest/download/rsepuh-$target.tar.gz"
echo "Downloading $url"
curl -fsSL "$url" | tar -xz -C "$dest" rsepuh
chmod +x "$dest/rsepuh"
echo "Installed $dest/rsepuh"
case ":$PATH:" in *":$dest:"*) ;; *) echo "Add $dest to your PATH:  export PATH=\"$dest:\$PATH\"" ;; esac
if ! command -v rcargo >/dev/null 2>&1; then
  echo "rcargo is required (it provides the remote machine). Install it:"
  echo "  curl -fsSL https://raw.githubusercontent.com/igun997/rcargo/main/install.sh | sh"
fi
if ! command -v rblender >/dev/null 2>&1; then
  echo "Optional, for 3D (Blender) scenes:"
  echo "  curl -fsSL https://raw.githubusercontent.com/igun997/rblender/main/install.sh | sh"
fi
echo "Next: go to an empty folder for your video project and run:  rsepuh init"
