# Release process

How a tagged release of **httui** is built, published, and
distributed. The pipeline is `.github/workflows/release.yml`,
triggered by pushing a `v*` tag.

> **Status:** httui is pre-stable (`0.x`); the first public release is
> `v0.4.0`. macOS and Windows builds are **unsigned** (no Apple
> Developer ID / Authenticode — decision 2026-05-17): macOS is ad-hoc
> signed, Windows triggers SmartScreen. The `APPLE_*` env is **not
> wired** in `release.yml`; enabling Developer ID signing later is a
> *deliberate workflow edit* (re-add the env) plus the secrets — see
> §5, not a secrets-only switch. Notarization, Homebrew/winget
> publishing, and the RC soak can only be verified with real certs,
> tokens, CI runs, and wall-clock time — they are **CI/cert-bound**
> and validated post-tag.

## 1. Required GitHub secrets

| Secret | Purpose | Unset behaviour |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign key signing the updater artifacts (`latest.json` + `.sig`). Public key is pinned in `tauri.conf.json` → `plugins.updater.pubkey`. | No updater artifacts; in-app auto-update is inert. Installers still ship. |
| `APPLE_CERTIFICATE` | Base64 of the Developer ID `.p12`. **Not referenced by the workflow** — see §5. | macOS build is **unsigned**, ad-hoc signed (see §5). |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` password. | — |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)`. | — |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization (app-specific password). | No notarization. |
| `HOMEBREW_TAP_TOKEN` | PAT with write access to `httuicom/homebrew-httui`. | Homebrew bump skipped (release still succeeds). |
| `WINGET_TOKEN` | Classic PAT that can fork `microsoft/winget-pkgs`. | winget submission skipped. |

`GITHUB_TOKEN` is provided automatically.

## 2. Tag conventions

- Stable: `vMAJOR.MINOR.PATCH` — e.g. `v0.4.0`.
- Pre-release: append `-rc.N`, `-beta.N`, or `-alpha.N`. Canonical
  form uses the dot (`v0.4.0-rc.1`); the bare form (`v0.4.0-rc1`) is
  also accepted by the gate.
- A pre-release tag ⇒ GitHub release flagged **prerelease** and
  excluded from the default auto-update channel (users opt in under
  **Settings → General → Software updates → Include pre-releases**).
- The `validate` job **fails the whole release** if `CHANGELOG.md`
  has no `## [VERSION]` section. A pre-release falls back to its base
  version section (`v0.4.0-rc.1` → `## [0.4.0]`). Notes are curated,
  never auto-derived.

## 3. Cutting a release

1. Curate `CHANGELOG.md`: move work from `## [Unreleased]` into a new
   `## [X.Y.Z] - YYYY-MM-DD` section. Add the `[X.Y.Z]` link ref.
2. Commit on `main` (or release branch).
3. Tag and push:
   ```bash
   git tag v0.4.1-rc.1
   git push origin v0.4.1-rc.1
   ```
4. Watch **Actions → Release**. Jobs: `validate` → `release`
   (macOS arm64, macOS x64, Linux, Windows in parallel) →
   `homebrew` + `winget` (stable only).
5. Verify the GitHub Release: `.dmg` ×2, `.app.tar.gz` + `.sig`,
   `.msi`, `.exe`, `.deb`, `.rpm`, `.AppImage`, `latest.json`.

## 4. RC soak (recommended)

For a significant release, ship one or more `-rc` tags first and let
them soak before cutting the stable `vX.Y.0`:

- A soak of ~1 week per RC with no release-blocking bug is the
  recommended bar for a feature release.
- Pre-stable `0.x` and isolated patch releases (`vX.Y.Z`, Z>0) may
  ship directly when the change is small and well-tested — `v0.4.0`
  itself shipped after a same-day RC pass.

When you do soak, record each RC and its window in the release notes.

## 5. macOS — unsigned build & Gatekeeper

The `.dmg` is **not** notarized, so Gatekeeper blocks a manually
downloaded `.app` on first open. The two supported install paths
clear this automatically — **the install script
(`https://httui.com/install.sh`) and the Homebrew cask both strip the
quarantine attribute on install**, so users hitting those never see a
prompt. Only a hand-downloaded `.dmg` needs a workaround:

- **Right-click → Open**, then confirm the dialog; or
- Clear the quarantine attribute:
  ```bash
  xattr -dr com.apple.quarantine /Applications/httui.app
  ```

macOS signing is **deliberately not wired** in v1: the `APPLE_*`
env is entirely absent from `release.yml`. Org/repo secrets and
variables only reach a workflow through a `${{ }}` expression, so
with no expression referencing them a lingering/broken **org-level**
`APPLE_CERTIFICATE` (which previously failed every macOS job at
`security import`) physically cannot reach the build. The build
ad-hoc signs and ships unsigned.

When an Apple Developer ID is acquired ($99/yr): re-add the six
`APPLE_*` env entries under the `Build and release` step (a
deliberate workflow edit) and set the corresponding secrets.
`Entitlements.plist` (hardened runtime) is already referenced from
`tauri.conf.json`. Re-tag — the same workflow signs and notarizes;
Gatekeeper then accepts the build with no user steps.

## 6. Windows

`.msi` and `.exe` (NSIS) ship **unsigned** — SmartScreen warns on
first run; users click **More info → Run anyway**. An Authenticode
cert is optional and out of scope for now.

## 7. Linux

```bash
sudo dpkg -i httui_0.4.0_amd64.deb        # Debian/Ubuntu
sudo rpm -i  httui-0.4.0-1.x86_64.rpm     # Fedora/RHEL
chmod +x httui_0.4.0_amd64.AppImage && ./httui_0.4.0_amd64.AppImage
```

## 7a. TUI distribution & unified `httui` launcher

Every release ships **three** binaries side by side inside the same
bundle:

| Binary | Role |
|---|---|
| `httui` | Launcher exposed on the `PATH`. Routes to the TUI by default; `httui desktop [args]` opens the desktop app. |
| `httui-tui` | Terminal UI binary (formerly `notes-tui`). |
| `httui-desktop` | Desktop main binary (Tauri). What the `.app` / `.exe` runs when launched from Finder / Start menu. |

How the launcher reaches the siblings: `httui` resolves
`current_exe().parent()` and looks for `httui-tui` (default) or
`httui-desktop` (`httui desktop`) in the same directory.

**Pipeline integration:** the `Stage TUI + launcher binaries for the
bundle` step in `release.yml` runs `scripts/prepare-bundle-bins.sh`
once per matrix entry. The script builds `httui-tui` + `httui-launcher`
in release mode for the target triple (matrix `target`, falling back to
the host triple when empty) and copies them to
`httui-desktop/src-tauri/binaries/<name>-<triple>` — the layout Tauri's
`bundle.externalBin` expects. The Tauri bundler then drops the suffix
on the destination side (`Contents/MacOS/httui-tui`, `/usr/bin/httui`).

**Per-platform install layout:**

- **macOS `.dmg` / cask:** all three binaries land in
  `httui.app/Contents/MacOS/`. The cask declares `binary
  "#{appdir}/httui.app/Contents/MacOS/httui", target: "httui"`, so
  `brew install --cask httui` creates `/usr/local/bin/httui`
  automatically — no postinstall needed.
- **Linux `.deb` / `.rpm`:** Tauri places all three under `/usr/bin/`
  (no PATH change required).
- **Linux `.AppImage`:** the `httui` launcher inside the AppImage
  expects siblings in `usr/bin/` of the squashed FS; running the
  AppImage executes the desktop main entry, so the TUI is reachable
  only by extracting the AppImage. Documented limitation —
  installation via `.deb`/`.rpm` is the supported path for terminal
  use.
- **Windows `.msi` / `.exe`:** the three binaries install to
  `%LOCALAPPDATA%\Programs\httui\`. Adding `%LOCALAPPDATA%\Programs\httui`
  to the user `PATH` automatically is **out of scope for v1** — users
  who want the terminal command run `httui` from that directory or add
  the folder to `PATH` manually. Follow-up issue tracks NSIS PATH
  injection.

**Local install (without Homebrew):**

```bash
make install        # builds bundle and copies httui.app to /Applications
make install-tui    # ln -s /Applications/httui.app/Contents/MacOS/httui /usr/local/bin/httui
```

**Smoke test (post-install)**

```bash
httui --help                    # launcher help
httui                           # opens the TUI
httui desktop                   # opens the desktop app
open /Applications/httui.app    # double-click equivalent
```

Opening the `.app` from the GUI keeps the original behavior
(`CFBundleExecutable` → `httui-desktop`); the launcher does not
intercept that path.

## 8. Homebrew

Prerequisite: the tap repo **`httuicom/homebrew-httui`** must exist
and `HOMEBREW_TAP_TOKEN` must be set. On each stable release the
`homebrew` job regenerates `Casks/httui.rb` (arm + intel dmg
sha256) and pushes it. Users:

```bash
brew tap httuicom/httui
brew install --cask httui
brew upgrade --cask httui
```

## 9. winget

The `winget` job submits a PR to `microsoft/winget-pkgs` via
`winget-releaser` (needs `WINGET_TOKEN`). The **first** manifest is
reviewed/merged manually by Microsoft; subsequent versions are
automated. After merge: `winget install httui.notes`.

## 10. Auto-update

`useAutoUpdate` calls the updater plugin against
`releases/latest/download/latest.json` (GitHub's "latest" excludes
prereleases server-side). A second client-side gate
(`shouldOfferUpdate`) hides pre-release versions unless the user
opted in. Requires `TAURI_SIGNING_PRIVATE_KEY` so the artifacts are
signed and `tauri.conf.json` → `bundle.createUpdaterArtifacts` is
`true` (already set).

## 11. Rollback

A bad release: delete the GitHub Release **and** its tag. The
updater reads the newest **stable** release's `latest.json`, so
removing the bad release reverts clients on their next check. Never
force-push over a published tag — cut a new patch tag instead.
