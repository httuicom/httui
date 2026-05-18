# Vendored fonts — licenses & provenance

All font files in this directory are bundled with the desktop app so it
loads zero fonts over the network at runtime (offline-safe). Every font
below permits redistribution.

## SIL Open Font License 1.1 (OFL-1.1)

Fetched as `.woff2` from Fontsource via the jsDelivr npm CDN at the pinned
versions noted. Pattern:
`https://cdn.jsdelivr.net/npm/@fontsource/<slug>@<ver>/files/<slug>-latin-<weight>-<style>.woff2`

| Family | Fontsource slug | Version | Files |
|---|---|---|---|
| Geist | `geist-sans` | 5.2.5 | 400, 500, 600, 700 |
| Geist Mono | `geist-mono` | 5.2.8 | 400, 500, 600 |
| Source Serif 4 | `source-serif-4` | 5.2.9 | 400, 500, 600, 700, 400-italic |
| Figtree | `figtree` | 5.2.10 | 400, 500, 600 |
| JetBrains Mono | `jetbrains-mono` | 5.2.8 | 400, 500 |
| Inter | `inter` | 5.2.8 | 400, 500, 600 |
| Fira Code | `fira-code` | 5.2.7 | 400, 500 |
| Source Code Pro | `source-code-pro` | 5.2.7 | 400, 500 |

OFL-1.1 full text: <https://openfontlicense.org/open-font-license-official-text/>
Each upstream Fontsource package ships its own `LICENSE` (OFL-1.1).

## GUST Font License (GFL — LPPL-like, redistribution permitted)

`lmroman-{regular,bold,italic,bolditalic}.woff2` — **genuine GUST e-foundry
Latin Modern Roman**. No `.woff2` is published upstream (GUST/CTAN ships
OTF only; the web project below ships genuine GUST `.woff`). Acquired from
`sugina-dev/latin-modern-web` tag `1.0.1`:

`https://cdn.jsdelivr.net/gh/sugina-dev/latin-modern-web@1.0.1/font/lmroman-<face>-webfont.woff`

then transcoded `.woff` → `.woff2` locally with `fonttools` (lossless
re-container; glyph count verified identical, 234 glyphs per face). No
glyph data was modified.

GUST Font License: <https://www.gust.org.pl/projects/e-foundry/licenses>
