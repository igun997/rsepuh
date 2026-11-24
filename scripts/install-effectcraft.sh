#!/usr/bin/env bash
# Install the headless EffectCraft CLI into $EFFECTCRAFT_HOME (default ~/effectcraft).
# Idempotent. Linux x86_64 and aarch64 (Debian/Ubuntu sandboxes: instabox, Daytona, any VPS).
#
#   EFFECTCRAFT_VERSION=0.25.1 bash scripts/install-effectcraft.sh
#
# Only bin/effectcraft-cli is kept (the desktop app and icons are not needed to render).
set -euo pipefail

VERSION="${EFFECTCRAFT_VERSION:-0.25.1}"
HOME_DIR="${EFFECTCRAFT_HOME:-$HOME/effectcraft}"
case "$(uname -m)" in
  x86_64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "install-effectcraft: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
NAME="effectcraft-${VERSION}-linux-${ARCH}"
BASE="https://github.com/storytold/effectcraft/releases/download/v${VERSION}"

if [ -x "$HOME_DIR/bin/effectcraft-cli" ] && "$HOME_DIR/bin/effectcraft-cli" --empty info 2>/dev/null | grep -q "EffectCraft ${VERSION}"; then
  echo "install-effectcraft: EffectCraft ${VERSION} already installed at $HOME_DIR"
  exit 0
fi

SUDO=""
if [ "$(id -u)" != "0" ]; then
  if command -v sudo >/dev/null && sudo -n true 2>/dev/null; then SUDO="sudo -n"; fi
fi
need=""
for c in curl tar sha256sum; do command -v "$c" >/dev/null || need="$need $c"; done
if [ -n "$need" ]; then
  [ -n "$SUDO" ] || [ "$(id -u)" = "0" ] || { echo "install-effectcraft: missing:$need and no root/sudo" >&2; exit 1; }
  $SUDO env DEBIAN_FRONTEND=noninteractive apt-get update -qq
  $SUDO env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends ca-certificates curl coreutils tar >/dev/null
fi

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
echo "install-effectcraft: downloading ${NAME}.tar.gz"
curl -fsSL --retry 3 -o "$TMP/$NAME.tar.gz" "$BASE/$NAME.tar.gz"
curl -fsSL --retry 3 -o "$TMP/sums" "$BASE/SHA256SUMS.txt"
( cd "$TMP" && grep " ${NAME}.tar.gz\$" sums | sha256sum -c - )

rm -rf "$HOME_DIR"; mkdir -p "$HOME_DIR/bin"
tar -xzf "$TMP/$NAME.tar.gz" -C "$HOME_DIR/bin" --strip-components=2 "$NAME/bin/effectcraft-cli"
chmod +x "$HOME_DIR/bin/effectcraft-cli"
"$HOME_DIR/bin/effectcraft-cli" --empty info | head -1
echo "install-effectcraft: ok -> $HOME_DIR/bin/effectcraft-cli"
