.PHONY: dev build release install install-deps install-app uninstall lint fmt check clean wipe-config test test-rust test-front test-tui front icons sidecar tui tui-build tui-help coverage-check coverage-rust coverage-fe size-check quality-check setup-hooks

# Development — frontend (Vite HMR) + backend (Rust rebuild on change).
# Trims Rust artifacts not touched in the last 7 days before each session
# so target/ stops inflating past 30 GB. Requires `cargo install cargo-sweep`;
# falls back silently if missing so dev never blocks on a tooling gap.
dev: sidecar
	@command -v cargo-sweep >/dev/null 2>&1 && cargo sweep --time 7 || true
	npm run tauri dev

# Run the terminal binary. Opens the active vault from the database;
# prompts on first run.
tui:
	cargo run -p httui-tui

tui-help:
	cargo run -p httui-tui -- --help

tui-build:
	cargo build -p httui-tui --release

test-tui:
	cargo test -p httui-tui

# Frontend only (sem janela desktop)
front:
	npm run dev

# Build do sidecar (JS bundle) — empacotado como recurso Tauri, executado via node
sidecar:
	@command -v bun >/dev/null 2>&1 || { \
		echo "Error: bun is required to build the sidecar."; \
		echo "Install with: curl -fsSL https://bun.sh/install | bash"; \
		exit 1; \
	}
	@mkdir -p httui-desktop/src-tauri/resources
	cd httui-sidecar && bun install && bun run build

# Build de producao (com bundle .app para macOS)
build: sidecar
	npm run tauri build -- --bundles app

# Instalar dependencias
install-deps:
	npm install
	cd httui-sidecar && (command -v bun >/dev/null 2>&1 && bun install || echo "skip: bun not installed")
	cargo fetch

# Build + instalar app em /Applications (macOS)
APP_NAME = httui
APP_BUNDLE = target/release/bundle/macos/$(APP_NAME).app
install: build
	@if [ ! -d "$(APP_BUNDLE)" ]; then \
		echo "Error: build failed — $(APP_BUNDLE) not found"; \
		exit 1; \
	fi
	@echo "Installing $(APP_NAME) to /Applications..."
	@rm -rf "/Applications/$(APP_NAME).app"
	@cp -R "$(APP_BUNDLE)" "/Applications/$(APP_NAME).app"
	@echo "Done. Open with: open '/Applications/$(APP_NAME).app'"

# Remover app de /Applications
uninstall:
	@echo "Removing $(APP_NAME) from /Applications..."
	@rm -rf "/Applications/$(APP_NAME).app"
	@echo "Done."

# Type check + clippy + fmt-check + prettier-check. Mirrors the CI gate.
check:
	./node_modules/.bin/tsc --noEmit -p httui-desktop/tsconfig.json
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings

# Lint TS/TSX in both frontend workspaces (eslint, not tsc).
lint:
	npm run lint --workspace httui-desktop
	npm run lint --workspace httui-web

# Format Rust + frontend in place.
fmt:
	cargo fmt --all
	./node_modules/.bin/prettier --write 'httui-desktop/src/**/*.{ts,tsx,js,jsx,css,md}' 'httui-web/src/**/*.{ts,tsx,js,jsx,css,md}' 'httui-sidecar/src/**/*.{ts,tsx,js,jsx}'

# Pre-release validation: every gate the CI runs, then the release build.
# Use this before tagging a version to catch regressions locally.
release: check test build
	@echo "Release artifacts ready in $(APP_BUNDLE)"

# Testes
test: test-rust test-tui test-front

test-rust:
	cargo test --workspace

test-front:
	npm run test --workspace httui-desktop -- --project unit

# Coverage gate — touched-files rule (≥80% per file changed).
# See CONTRIBUTING.md.
coverage-check:
	./scripts/coverage-check.sh

# Run the full Rust coverage report. Used by coverage-check; exposed
# for ad-hoc inspection (HTML report opens at target/llvm-cov/html/index.html).
coverage-rust:
	cargo llvm-cov --workspace --html

# Frontend coverage report (HTML at httui-desktop/coverage/index.html).
coverage-fe:
	cd httui-desktop && npm run test -- --project unit --coverage

# File-size gate — touched files must stay under MAX_LINES (default 600).
# SOLID nudge for SRP. See CONTRIBUTING.md.
size-check:
	./scripts/size-check.sh

# Combined gate — runs size-check then coverage-check. Same order as
# the pre-push hook so local runs match remote behavior.
quality-check: size-check coverage-check

# Symlink tracked git hooks into .git/hooks (idempotent).
setup-hooks:
	./scripts/setup-hooks.sh

# Limpar artifacts
clean:
	rm -rf httui-desktop/dist
	cargo clean

# Limpar estado persistente do app (configs + cache). Útil pra
# voltar ao empty state entre testes manuais. Mantém keychain
# (use `security delete-generic-password -s httui-notes` em loop
# se precisar limpar secrets também). Vaults no disco NÃO são
# tocados. Feche o app antes.
#
# Paths usados pelo app (productName=httui em tauri.conf.json):
#   ~/.config/httui                            (notes.db)
#   ~/Library/Application Support/httui        (user.toml — Mac)
#   ~/Library/Caches/httui-notes               (WebKit cache)
wipe-config:
	@echo "Wiping httui app config..."
	@rm -rf "$$HOME/.config/httui"
	@rm -rf "$$HOME/Library/Application Support/httui"
	@rm -rf "$$HOME/Library/Caches/httui-notes"
	@echo "Done. App opens with empty state on next launch."

# Gerar icones placeholder
icons:
	@mkdir -p httui-desktop/src-tauri/icons
	@python3 -c "\
	import struct, zlib; \
	def png(w,h,r,g,b): \
	    ch=lambda t,d: struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff); \
	    raw=b''.join(b'\x00'+bytes([r,g,b,255])*w for _ in range(h)); \
	    return b'\x89PNG\r\n\x1a\n'+ch(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6,0,0,0))+ch(b'IDAT',zlib.compress(raw))+ch(b'IEND',b''); \
	[open(f'httui-desktop/src-tauri/icons/{n}','wb').write(png(s,s,99,102,241)) for n,s in [('icon.png',256),('32x32.png',32),('128x128.png',128),('128x128@2x.png',256)]]; \
	print('icons generated')"
