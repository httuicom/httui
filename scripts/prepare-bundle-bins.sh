#!/usr/bin/env bash
# Builds httui-tui and httui-launcher in release mode and stages them
# under httui-desktop/src-tauri/binaries/<name>-<triple> so the Tauri
# bundler picks them up via `bundle.externalBin` in tauri.conf.json.
#
# Used by the Makefile (`make build`) and the GitHub Actions release
# workflow. Pass a target triple as $1 to cross-compile; with no
# argument the host triple is detected via rustc.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$REPO_ROOT/httui-desktop/src-tauri/binaries"

TARGET="${1:-}"
if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -vV | sed -n 's|host: ||p')"
fi

if [[ -z "$TARGET" ]]; then
  echo "error: could not determine target triple" >&2
  exit 1
fi

echo "preparing bundle binaries for target: $TARGET"

CARGO_ARGS=(--release --target "$TARGET")

cargo build -p httui-tui "${CARGO_ARGS[@]}"
cargo build -p httui-launcher "${CARGO_ARGS[@]}"

mkdir -p "$OUT_DIR"

# Windows binaries carry the .exe suffix in both source and destination.
if [[ "$TARGET" == *windows* ]]; then
  SUFFIX=".exe"
else
  SUFFIX=""
fi

SRC_DIR="$REPO_ROOT/target/$TARGET/release"

cp "$SRC_DIR/httui-tui$SUFFIX" "$OUT_DIR/httui-tui-$TARGET$SUFFIX"
cp "$SRC_DIR/httui$SUFFIX"     "$OUT_DIR/httui-$TARGET$SUFFIX"

# Belt + suspenders: Tauri's bundler propagates the source file mode
# into Contents/MacOS/ (and /usr/bin/ on Linux). If the staged file
# loses its exec bit for any reason (interrupted build, tarball
# extraction, etc.) the launcher in the final bundle ships
# non-executable and `httui` in $PATH errors with "Permission denied".
chmod +x "$OUT_DIR/httui-tui-$TARGET$SUFFIX" "$OUT_DIR/httui-$TARGET$SUFFIX"

echo "staged:"
echo "  $OUT_DIR/httui-tui-$TARGET$SUFFIX"
echo "  $OUT_DIR/httui-$TARGET$SUFFIX"
