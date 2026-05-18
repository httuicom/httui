# Release process

How a tagged release of **httui notes** is built, signed, published,
and distributed. The pipeline is `.github/workflows/release.yml`,
triggered by pushing a `v*` tag.

> **v1 status:** the macOS build is an **unsigned developer build**
> (no Apple Developer ID — decision 2026-05-17). The signing path is
> fully wired and gated on secrets; the day the cert exists, set the
> secrets and re-tag — no workflow edits. Some steps below
> (notarization, Homebrew/winget, the soak) can only be verified with
> real certs, tokens, CI runs, and wall-clock time — they are
> **CI/cert-bound** and validated post-tag, not in this repo.

## 1. Required GitHub secrets

| Secret | Purpose | Unset behaviour |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | minisign key signing the updater artifacts (`latest.json` + `.sig`). Public key is pinned in `tauri.conf.json` → `plugins.updater.pubkey`. | No updater artifacts; in-app auto-update is inert. Installers still ship. |
| `APPLE_CERTIFICATE` | Base64 of the Developer ID `.p12`. Only used when repo var `MACOS_SIGNING=true`. | macOS build is **unsigned**, ad-hoc signed (see §5). |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` password. | — |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)`. | — |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | Notarization (app-specific password). | No notarization. |
| `HOMEBREW_TAP_TOKEN` | PAT with write access to `httuicom/homebrew-httui`. | Homebrew bump skipped (release still succeeds). |
| `WINGET_TOKEN` | Classic PAT that can fork `microsoft/winget-pkgs`. | winget submission skipped. |

`GITHUB_TOKEN` is provided automatically.

## 2. Tag conventions

- Stable: `vMAJOR.MINOR.PATCH` — e.g. `v1.0.0`.
- Pre-release: append `-rc.N`, `-beta.N`, or `-alpha.N`. Canonical
  form uses the dot (`v1.0.0-rc.1`); the bare form (`v1.0.0-rc1`) is
  also accepted by the gate.
- A pre-release tag ⇒ GitHub release flagged **prerelease** and
  excluded from the default auto-update channel (users opt in under
  **Settings → General → Software updates → Include pre-releases**).
- The `validate` job **fails the whole release** if `CHANGELOG.md`
  has no `## [VERSION]` section. A pre-release falls back to its base
  version section (`v1.0.0-rc.1` → `## [1.0.0]`). Notes are curated,
  never auto-derived.

## 3. Cutting a release

1. Curate `CHANGELOG.md`: move work from `## [Unreleased]` into a new
   `## [X.Y.Z] - YYYY-MM-DD` section. Add the `[X.Y.Z]` link ref.
2. Commit on `main` (or release branch).
3. Tag and push:
   ```bash
   git tag v1.0.0-rc.1
   git push origin v1.0.0-rc.1
   ```
4. Watch **Actions → Release**. Jobs: `validate` → `release`
   (macOS arm64, macOS x64, Linux, Windows in parallel) →
   `homebrew` + `winget` (stable only).
5. Verify the GitHub Release: `.dmg` ×2, `.app.tar.gz` + `.sig`,
   `.msi`, `.exe`, `.deb`, `.rpm`, `.AppImage`, `latest.json`.

## 4. Two-tag soak (cenário 10)

Before the final stable `vX.Y.0`:

- **Minimum:** ship at least **one `-rc` tag and let it soak for ≥ 1
  week** with no release-blocking bug.
- **Recommended for a major:** two RCs (`-rc.1`, `-rc.2`), each
  soaked ≥ 1 week.
- Only then tag the stable `vX.Y.0`. Patch releases (`vX.Y.Z`,
  Z>0) may skip the soak when shipping an isolated fix.

Record each RC and its soak window in the PR / release notes.

## 5. macOS — unsigned dev build (v1) & Gatekeeper

Without the `APPLE_*` secrets the `.dmg` is **not** notarized.
Gatekeeper will refuse it on first open. Two user workarounds (put
these in the release notes / website):

- **Right-click → Open**, then confirm the dialog; or
- Clear the quarantine attribute:
  ```bash
  xattr -dr com.apple.quarantine /Applications/httui.app
  ```

macOS signing is gated by the repo **variable** `MACOS_SIGNING`. While
it is unset (or anything other than `true`) the workflow forces the
`APPLE_*` env to empty regardless of what secrets exist — so a
lingering/broken **org-level** `APPLE_CERTIFICATE` cannot reach the
build and break it. The build ad-hoc signs and ships unsigned.

When an Apple Developer ID is acquired ($99/yr): add the six
`APPLE_*` secrets **and** set the repo variable `MACOS_SIGNING=true`
(`gh variable set MACOS_SIGNING --repo httuicom/httui --body true`).
`Entitlements.plist` (hardened runtime) is already referenced from
`tauri.conf.json`. Re-tag — the same workflow signs and notarizes;
Gatekeeper then accepts the build with no user steps.

## 6. Windows

`.msi` and `.exe` (NSIS) ship **unsigned** for v1 — SmartScreen warns
on first run; users click **More info → Run anyway**. A code-signing
cert is optional and out of scope for v1.

## 7. Linux

```bash
sudo dpkg -i httui_1.0.0_amd64.deb        # Debian/Ubuntu
sudo rpm -i  httui-1.0.0-1.x86_64.rpm     # Fedora/RHEL
chmod +x httui_1.0.0_amd64.AppImage && ./httui_1.0.0_amd64.AppImage
```

## 8. Homebrew (cenário 5)

Prerequisite: the tap repo **`httuicom/homebrew-httui`** must exist
and `HOMEBREW_TAP_TOKEN` must be set. On each stable release the
`homebrew` job regenerates `Casks/httui.rb` (arm + intel dmg
sha256) and pushes it. Users:

```bash
brew tap httuicom/httui
brew install --cask httui
brew upgrade --cask httui
```

## 9. winget (cenário 6)

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
