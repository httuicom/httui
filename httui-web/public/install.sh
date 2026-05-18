#!/bin/sh
# httui installer — https://httui.com/install.sh
#
#   curl -fsSL https://httui.com/install.sh | sh
#
# Downloads the latest stable release from GitHub and installs it.
# macOS: copies httui.app into /Applications (or ~/Applications) and
# strips the quarantine attribute so the (ad-hoc-signed) build opens
# without a Gatekeeper prompt. Linux: drops the AppImage in
# ~/.local/bin/httui. Override the target with HTTUI_PREFIX.
set -eu

REPO="httuicom/httui"
API="https://api.github.com/repos/${REPO}/releases/latest"

say() { printf '%s\n' "$*"; }
err() { printf 'install: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || err "curl is required"

OS="$(uname -s)"
ARCH="$(uname -m)"

# releases/latest excludes pre-releases, so this is always a stable tag.
TAG="$(curl -fsSL "$API" | grep '"tag_name"' | head -1 \
  | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')"
[ -n "${TAG:-}" ] || err "could not resolve the latest release"
VER="${TAG#v}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64 | aarch64) ASSET="httui_${VER}_aarch64.dmg" ;;
      x86_64) ASSET="httui_${VER}_x64.dmg" ;;
      *) err "unsupported macOS arch: $ARCH" ;;
    esac
    URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
    say "Downloading httui ${VER} ($ASSET)..."
    curl -fL --progress-bar "$URL" -o "$tmp/httui.dmg" || err "download failed: $URL"

    mnt="$tmp/mnt"
    mkdir -p "$mnt"
    hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$tmp/httui.dmg" >/dev/null \
      || err "could not mount the disk image"
    app="$(find "$mnt" -maxdepth 1 -name '*.app' -print 2>/dev/null | head -1)"
    if [ -z "${app:-}" ]; then
      hdiutil detach "$mnt" >/dev/null 2>&1 || true
      err "no .app found inside the disk image"
    fi

    dest="${HTTUI_PREFIX:-/Applications}"
    [ -w "$dest" ] 2>/dev/null || dest="$HOME/Applications"
    mkdir -p "$dest"
    say "Installing to $dest ..."
    rm -rf "$dest/httui.app"
    cp -R "$app" "$dest/"
    hdiutil detach "$mnt" >/dev/null 2>&1 || true
    # Ad-hoc-signed build: clear quarantine so Gatekeeper doesn't block it.
    xattr -dr com.apple.quarantine "$dest/httui.app" 2>/dev/null || true
    say ""
    say "Installed httui ${VER} → $dest/httui.app"
    say "Launch:  open \"$dest/httui.app\""
    ;;
  Linux)
    case "$ARCH" in
      x86_64 | amd64) ASSET="httui_${VER}_amd64.AppImage" ;;
      *) err "unsupported Linux arch: $ARCH (only x86_64 AppImage is built)" ;;
    esac
    URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
    bin="${HTTUI_PREFIX:-$HOME/.local/bin}"
    mkdir -p "$bin"
    say "Downloading httui ${VER} ($ASSET)..."
    curl -fL --progress-bar "$URL" -o "$bin/httui" || err "download failed: $URL"
    chmod +x "$bin/httui"
    say ""
    say "Installed httui ${VER} → $bin/httui"
    case ":$PATH:" in
      *":$bin:"*) say "Launch:  httui" ;;
      *) say "Add $bin to your PATH, then run: httui" ;;
    esac
    ;;
  *)
    err "unsupported OS: $OS (use the GitHub releases page)"
    ;;
esac
