#!/usr/bin/env bash
# Builds httui-tui and httui-launcher in release mode, stages the
# httui-lsp language server, and places everything under
# httui-desktop/src-tauri/binaries/<name>-<triple> so the Tauri
# bundler picks them up via `bundle.externalBin` in tauri.conf.json.
#
# Used by the Makefile (`make build`) and the GitHub Actions release
# workflow. Pass a target triple as $1 to cross-compile; with no
# argument the host triple is detected via rustc.
#
# httui-lsp is an OCaml binary released by httuicom/httui-lang; it is
# downloaded at the version pinned below. Set HTTUI_LSP_BUNDLE_BIN to a
# local build (e.g. httui-lang/_build/default/bin/httui-lsp/httui_lsp.exe)
# to skip the download. Windows targets skip the server entirely — the
# desktop degrades gracefully without it.

set -euo pipefail

# Pinned httui-lang release providing the httui-lsp assets.
HTTUI_LANG_VERSION="0.1.0"

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

if [[ "$TARGET" == *windows* ]]; then
  echo "skipping httui-lsp: no Windows build yet (desktop degrades gracefully)"
else
  LSP_DEST="$OUT_DIR/httui-lsp-$TARGET"
  # rm first: dune outputs are read-only and block in-place overwrite.
  rm -f "$LSP_DEST"
  if [[ -n "${HTTUI_LSP_BUNDLE_BIN:-}" ]]; then
    echo "staging httui-lsp from HTTUI_LSP_BUNDLE_BIN: $HTTUI_LSP_BUNDLE_BIN"
    cp "$HTTUI_LSP_BUNDLE_BIN" "$LSP_DEST"
  else
    LSP_URL="https://github.com/httuicom/httui-lang/releases/download/v${HTTUI_LANG_VERSION}/httui-lsp-${TARGET}"
    echo "downloading httui-lsp ${HTTUI_LANG_VERSION} for $TARGET"
    # -f: a missing asset must fail the release, never ship without lsp.
    curl -fL --retry 3 -o "$LSP_DEST" "$LSP_URL"
  fi
  chmod 755 "$LSP_DEST"
fi

echo "staged:"
echo "  $OUT_DIR/httui-tui-$TARGET$SUFFIX"
echo "  $OUT_DIR/httui-$TARGET$SUFFIX"
if [[ "$TARGET" != *windows* ]]; then
  echo "  $OUT_DIR/httui-lsp-$TARGET"
fi
