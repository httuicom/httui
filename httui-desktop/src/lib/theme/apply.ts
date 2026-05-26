import type { ThemeConfig } from "./config";
import {
  ACCENT_PALETTES,
  GRAY_PALETTES,
  FONT_BODY_OPTIONS,
  FONT_MONO_OPTIONS,
  DENSITY_SCALES,
  SHADOW_OPTIONS,
} from "./config";

const SHADE_KEYS = [
  "50",
  "100",
  "200",
  "300",
  "400",
  "500",
  "600",
  "700",
  "800",
  "900",
  "950",
] as const;

const STYLE_ID = "httui-theme-overrides";

/**
 * Build a <style> block that maps the gray palette to Chakra's semantic tokens
 * for both light and dark modes. This is necessary because Chakra resolves
 * semantic tokens (bg, fg, border) at build time, so overriding --chakra-colors-gray-*
 * alone doesn't cascade to them.
 */
function buildSemanticStyleSheet(config: ThemeConfig): string {
  const gray = GRAY_PALETTES[config.grayTone];
  const accent = ACCENT_PALETTES[config.accentColor];
  if (!gray) return "";

  const g = gray.scale;
  const a = accent?.scale;
  const cc = config.customColors;

  // Light mode: use custom overrides if set, else derive from gray palette
  const lightBg = cc?.light?.bg ?? "#ffffff";
  const lightBgSubtle = cc?.light?.bgSubtle ?? g[50];
  const lightFg = cc?.light?.fg ?? g[950];
  const lightFgMuted = cc?.light?.fgMuted ?? g[500];
  const lightBorder = cc?.light?.border ?? g[200];

  // Dark mode: use custom overrides if set, else derive from gray palette
  const darkBg = cc?.dark?.bg ?? g[950];
  const darkBgSubtle = cc?.dark?.bgSubtle ?? g[900];
  const darkFg = cc?.dark?.fg ?? g[50];
  const darkFgMuted = cc?.dark?.fgMuted ?? g[400];
  const darkBorder = cc?.dark?.border ?? g[800];

  return `
    .light {
      --chakra-colors-bg: ${lightBg};
      --chakra-colors-bg-subtle: ${lightBgSubtle};
      --chakra-colors-bg-muted: ${g[100]};
      --chakra-colors-bg-emphasized: ${g[200]};
      --chakra-colors-fg: ${lightFg};
      --chakra-colors-fg-muted: ${lightFgMuted};
      --chakra-colors-fg-subtle: ${g[400]};
      --chakra-colors-border: ${lightBorder};
      --chakra-colors-border-muted: ${g[100]};
      ${
        a
          ? `
      --chakra-colors-brand-fg: ${a[600]};
      --chakra-colors-brand-subtle: ${a[50]};
      --chakra-colors-brand-muted: ${a[200]};
      --chakra-colors-brand-emphasized: ${a[600]};
      --chakra-colors-brand-solid: ${a[600]};
      --chakra-colors-brand-contrast: #ffffff;
      --chakra-colors-brand-focusRing: ${a[500]};
      `
          : ""
      }
    }
    .dark {
      --chakra-colors-bg: ${darkBg};
      --chakra-colors-bg-subtle: ${darkBgSubtle};
      --chakra-colors-bg-muted: ${g[800]};
      --chakra-colors-bg-emphasized: ${g[700]};
      --chakra-colors-fg: ${darkFg};
      --chakra-colors-fg-muted: ${darkFgMuted};
      --chakra-colors-fg-subtle: ${g[600]};
      --chakra-colors-border: ${darkBorder};
      --chakra-colors-border-muted: ${g[900]};
      ${
        a
          ? `
      --chakra-colors-brand-fg: ${a[400]};
      --chakra-colors-brand-subtle: color-mix(in srgb, ${a[950]} 80%, transparent);
      --chakra-colors-brand-muted: color-mix(in srgb, ${a[800]} 60%, transparent);
      --chakra-colors-brand-emphasized: ${a[400]};
      --chakra-colors-brand-solid: ${a[500]};
      --chakra-colors-brand-contrast: #ffffff;
      --chakra-colors-brand-focusRing: ${a[500]};
      `
          : ""
      }
    }
  `;
}

/**
 * Apply a ThemeConfig to the document by setting CSS custom properties.
 * Uses both inline style overrides and an injected <style> block for
 * mode-aware semantic tokens.
 */
export function applyTheme(config: ThemeConfig): void {
  const root = document.documentElement;
  const s = (prop: string, val: string) => root.style.setProperty(prop, val);

  // ─── Accent color (override brand palette) ────────────
  const accent = ACCENT_PALETTES[config.accentColor];
  if (accent) {
    for (const shade of SHADE_KEYS) {
      s(`--chakra-colors-brand-${shade}`, accent.scale[shade]);
    }
  }

  // ─── Gray tone (override gray palette) ────────────────
  const gray = GRAY_PALETTES[config.grayTone];
  if (gray) {
    for (const shade of SHADE_KEYS) {
      s(`--chakra-colors-gray-${shade}`, gray.scale[shade]);
    }
  }

  // ─── Semantic tokens (mode-aware via <style> injection) ─
  let styleEl = document.getElementById(STYLE_ID) as HTMLStyleElement | null;
  if (!styleEl) {
    styleEl = document.createElement("style");
    styleEl.id = STYLE_ID;
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = buildSemanticStyleSheet(config);

  // ─── Border radius ────────────────────────────────────
  const r = `${config.borderRadius}px`;
  s("--chakra-radii-sm", `${Math.max(config.borderRadius - 2, 0)}px`);
  s("--chakra-radii-md", r);
  s("--chakra-radii-lg", `${config.borderRadius + 2}px`);
  s("--chakra-radii-xl", `${config.borderRadius + 6}px`);
  s("--chakra-radii-2xl", `${config.borderRadius + 10}px`);

  // ─── Border width ─────────────────────────────────────
  s("--theme-border-width", `${config.borderWidth}px`);

  // ─── Font family ──────────────────────────────────────
  const bodyFont = FONT_BODY_OPTIONS[config.fontBody];
  if (bodyFont) {
    s("--chakra-fonts-body", bodyFont.value);
    s("--chakra-fonts-heading", bodyFont.value);
  }

  const monoFont = FONT_MONO_OPTIONS[config.fontMono];
  if (monoFont) {
    s("--chakra-fonts-mono", monoFont.value);
  }

  // ─── Font size ────────────────────────────────────────
  s("--theme-font-size", `${config.fontSize}px`);

  // ─── Density ──────────────────────────────────────────
  const density = DENSITY_SCALES[config.density];
  if (density) {
    s("--theme-density", String(density.multiplier));
  }

  // ─── Shadow ───────────────────────────────────────────
  const shadow = SHADOW_OPTIONS[config.shadow];
  if (shadow) {
    s("--theme-shadow", shadow.value);
  }
}

/**
 * Remove all theme overrides, reverting to Chakra defaults.
 */
export function clearTheme(): void {
  const root = document.documentElement;
  const r = (prop: string) => root.style.removeProperty(prop);

  for (const shade of SHADE_KEYS) {
    r(`--chakra-colors-brand-${shade}`);
    r(`--chakra-colors-gray-${shade}`);
  }

  r("--chakra-radii-sm");
  r("--chakra-radii-md");
  r("--chakra-radii-lg");
  r("--chakra-radii-xl");
  r("--chakra-radii-2xl");
  r("--theme-border-width");
  r("--chakra-fonts-body");
  r("--chakra-fonts-heading");
  r("--chakra-fonts-mono");
  r("--theme-font-size");
  r("--theme-density");
  r("--theme-shadow");

  // Remove injected style block
  const styleEl = document.getElementById(STYLE_ID);
  if (styleEl) styleEl.remove();
}
