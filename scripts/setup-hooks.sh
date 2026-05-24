#!/usr/bin/env bash
# Symlink the tracked git hooks into .git/hooks/. Idempotent.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

mkdir -p .git/hooks
for hook in pre-push pre-commit commit-msg; do
    src="$REPO_ROOT/scripts/hooks/$hook"
    dst="$REPO_ROOT/.git/hooks/$hook"
    if [ -e "$dst" ] && [ ! -L "$dst" ]; then
        echo "setup-hooks: $dst exists and is not a symlink — leaving alone."
        echo "  (delete it manually or run: rm $dst && ./scripts/setup-hooks.sh)"
        continue
    fi
    ln -sf "$src" "$dst"
    chmod +x "$src"
    echo "setup-hooks: linked $hook"
done

echo
echo "setup-hooks: done. Pre-push will block on coverage; pre-commit reports only."
echo "Need cargo-llvm-cov?"
echo "  rustup component add llvm-tools-preview"
echo "  cargo install cargo-llvm-cov"
