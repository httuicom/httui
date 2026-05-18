// Fuji design tokens — canvas-derived single source of truth.
// Mirrors the design canvas §0. Edits here
// must keep the canvas in sync.
//
// Consumed by `lib/theme.ts` (Chakra system) and any component that
// needs raw values outside the Chakra prop system (CM6 themes,
// inline styles).

// --- Type stacks --------------------------------------------------------

export const FONT_SANS =
  '"Geist", -apple-system, "Segoe UI", system-ui, sans-serif';
export const FONT_MONO =
  '"Geist Mono", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace';
export const FONT_SERIF =
  '"Source Serif 4", "Source Serif Pro", "Iowan Old Style", Georgia, serif';
export const FONT_MARKDOWN_BODY =
  '"Latin Modern Roman", "Source Serif 4", Georgia, serif';

export const FONT_FEATURES_SANS = '"ss01", "cv11"';
export const FONT_FEATURES_MONO = '"zero", "ss01"';

// --- Type scale (px values per canvas §0) -------------------------------

export const TYPE_SCALE = {
  xs: "11px",
  sm: "12px",
  base: "13px",
  md: "14px",
  lg: "16px",
  xl: "20px",
  "2xl": "28px",
} as const;

export type TypeScaleKey = keyof typeof TYPE_SCALE;

// --- Method colors (oklch) ---------------------------------------------

export const METHOD_COLORS = {
  get: "oklch(0.78 0.07 215)", // sky
  post: "oklch(0.62 0.10 145)", // moss
  put: "oklch(0.78 0.13 80)", // canola pale
  patch: "oklch(0.74 0.15 60)", // ochre
  delete: "oklch(0.66 0.18 15)", // sunset red
  head: "oklch(0.64 0 0)", // neutral fallback (canvas: --fg-2)
  options: "oklch(0.64 0 0)", // neutral fallback
  sql: "oklch(0.74 0.10 280)", // lavender
  mongo: "oklch(0.62 0.10 145)", // moss
  ws: "oklch(0.78 0.07 215)", // sky
  gql: "oklch(0.74 0.16 330)", // magenta-rose
  sh: "oklch(0.82 0.012 80)", // neutral light fg
} as const;

export type MethodKey = keyof typeof METHOD_COLORS;

// Method-pill atom (canvas §0). The bg uses currentColor color-mix at
// runtime; consumers set `color: METHOD_COLORS[method]` on the pill.
export const METHOD_PILL_STYLE = {
  font: `600 10px/1 ${FONT_MONO}`,
  letterSpacing: "0.04em",
  padding: "3px 6px",
  borderRadius: "3px",
  background: "color-mix(in oklab, currentColor 16%, transparent)",
} as const;

// --- State colors (oklch) ----------------------------------------------

export const STATE_COLORS = {
  ok: "oklch(0.66 0.11 145)", // moss
  warn: "oklch(0.78 0.15 75)", // canola
  err: "oklch(0.66 0.18 15)", // sunset
  info: "oklch(0.74 0.07 215)", // sky
} as const;

export type StateKey = keyof typeof STATE_COLORS;

// --- Themes ------------------------------------------------------------

// "Fuji at dusk" (default) — extracted from a Mt. Fuji photograph at
// twilight: deep stone-blue ground, warm snow fg, canola gold accent.
export const THEME_DARK = {
  bg: "oklch(0.16 0.012 230)",
  bg1: "oklch(0.185 0.012 230)",
  bg2: "oklch(0.215 0.012 230)",
  bg3: "oklch(0.245 0.012 230)",
  bgHi: "oklch(0.295 0.012 230)",
  line: "oklch(0.285 0.012 230)",
  lineSoft: "oklch(0.235 0.012 230)",
  fg: "oklch(0.96 0.008 80)",
  fg1: "oklch(0.82 0.008 80)",
  fg2: "oklch(0.64 0.008 80)",
  fg3: "oklch(0.50 0.008 80)",
  accent: "oklch(0.84 0.16 90)",
  accentFg: "oklch(0.18 0.04 90)",
  accentSoft: "oklch(0.32 0.06 90)",
  sel: "oklch(0.42 0.10 220 / 0.45)",
} as const;

// "Fuji photo" — daylight Fuji palette. Light bg, deep blue ink fg,
// canola yellow accent (use sparingly; meant for small punchy areas).
export const THEME_LIGHT = {
  bg: "oklch(0.985 0.006 90)",
  bg1: "oklch(0.965 0.006 90)",
  bg2: "oklch(0.935 0.006 90)",
  bg3: "oklch(0.905 0.006 90)",
  bgHi: "oklch(0.870 0.006 90)",
  line: "oklch(0.835 0.006 90)",
  lineSoft: "oklch(0.895 0.006 90)",
  fg: "oklch(0.20 0.040 240)",
  fg1: "oklch(0.32 0.040 240)",
  fg2: "oklch(0.48 0.030 240)",
  fg3: "oklch(0.62 0.020 240)",
  accent: "oklch(0.78 0.16 95)",
  accentFg: "oklch(0.18 0.04 95)",
  accentSoft: "oklch(0.92 0.06 95)",
  sel: "oklch(0.85 0.12 95 / 0.50)",
} as const;

export type ThemePalette = typeof THEME_DARK;
export type ThemeKey = keyof ThemePalette;

// --- Atoms (canvas §0) -------------------------------------------------

export const ATOMS = {
  kbd: {
    minWidth: "18px",
    height: "18px",
    padding: "0 5px",
    font: `500 10px/1 ${FONT_MONO}`,
    borderWidth: "1px",
    borderBottomWidth: "2px",
    borderRadius: "4px",
  },
  dot: {
    size: "6px",
  },
  btn: {
    height: "24px",
    padding: "0 10px",
    borderRadius: "4px",
  },
  input: {
    height: "24px",
    font: `400 12px/1 ${FONT_MONO}`,
  },
  statusbar: {
    height: "22px",
    font: `400 11px/1 ${FONT_MONO}`,
    gap: "14px",
  },
  tabbar: {
    height: "32px",
    accentLinePosition: "top" as const,
  },
  scrollbar: {
    thumbWidth: "10px",
    thumbBorder: "2px solid transparent",
  },
} as const;

// --- Spacing scale (4px base; canvas atom paddings drive the multiples) -

export const SPACING = {
  px: "1px",
  "0.5": "2px",
  "1": "4px",
  "1.5": "6px",
  "2": "8px",
  "2.5": "10px",
  "3": "12px",
  "3.5": "14px",
  "4": "16px",
  "5": "20px",
  "6": "24px",
  "7": "28px",
  "8": "32px",
  "10": "40px",
  "12": "48px",
} as const;

export type SpacingKey = keyof typeof SPACING;

// --- Radii -------------------------------------------------------------

export const RADII = {
  none: "0",
  sm: "2px",
  base: "4px",
  md: "6px",
  lg: "8px",
  xl: "12px",
  "2xl": "16px",
  full: "9999px",
} as const;

// --- Fuji watercolor backgrounds ---------------------------------------
// CSS-gradient fallback per the spec's "if Tauri webview can't render
// large PNGs efficiently, fall back to a CSS gradient that approximates
// the Fuji palette" — no PNG assets required. Mount on empty states
// only, never behind dense data tables.

export const FUJI_BG_DARK = `
  radial-gradient(ellipse at 22% 18%, oklch(0.32 0.03 230 / 0.55) 0%, transparent 55%),
  radial-gradient(ellipse at 78% 24%, oklch(0.36 0.04 220 / 0.40) 0%, transparent 60%),
  linear-gradient(180deg,
    oklch(0.22 0.02 230) 0%,
    oklch(0.18 0.012 230) 35%,
    oklch(0.16 0.012 230) 65%,
    oklch(0.18 0.02 95) 100%
  )
`.trim();

export const FUJI_BG_LIGHT = `
  radial-gradient(ellipse at 22% 18%, oklch(0.92 0.03 220 / 0.55) 0%, transparent 55%),
  radial-gradient(ellipse at 78% 28%, oklch(0.94 0.04 95 / 0.40) 0%, transparent 60%),
  linear-gradient(180deg,
    oklch(0.97 0.012 220) 0%,
    oklch(0.985 0.006 90) 38%,
    oklch(0.99 0.008 95) 70%,
    oklch(0.94 0.05 95) 100%
  )
`.trim();
