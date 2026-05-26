// ─── Theme configuration types ──────────────────────────────

export interface ModeColors {
  bg: string;
  bgSubtle: string;
  fg: string;
  fgMuted: string;
  border: string;
}

export interface CustomColorOverrides {
  light: ModeColors | null;
  dark: ModeColors | null;
}

export interface ThemeConfig {
  accentColor: string;
  grayTone: string;
  borderRadius: number;
  borderWidth: number;
  fontBody: string;
  fontMono: string;
  fontSize: number;
  density: "compact" | "default" | "comfortable";
  shadow: "none" | "subtle" | "medium";
  customColors: CustomColorOverrides | null;
}

export const DEFAULT_THEME: ThemeConfig = {
  accentColor: "amber",
  grayTone: "warm",
  borderRadius: 6,
  borderWidth: 1,
  fontBody: "figtree",
  fontMono: "jetbrains",
  fontSize: 14,
  density: "default",
  shadow: "subtle",
  customColors: null,
};

// ─── Accent color palettes (Tailwind-inspired, 11 shades) ──

export interface ColorScale {
  50: string;
  100: string;
  200: string;
  300: string;
  400: string;
  500: string;
  600: string;
  700: string;
  800: string;
  900: string;
  950: string;
}

export const ACCENT_PALETTES: Record<
  string,
  { label: string; swatch: string; scale: ColorScale }
> = {
  amber: {
    label: "Amber",
    swatch: "#e48320",
    scale: {
      50: "#fef7ed",
      100: "#fcecd4",
      200: "#f7d5a8",
      300: "#f2b870",
      400: "#ec9a38",
      500: "#e48320",
      600: "#c56416",
      700: "#a44a15",
      800: "#853b18",
      900: "#6d3318",
      950: "#3a170a",
    },
  },
  blue: {
    label: "Blue",
    swatch: "#3b82f6",
    scale: {
      50: "#eff6ff",
      100: "#dbeafe",
      200: "#bfdbfe",
      300: "#93c5fd",
      400: "#60a5fa",
      500: "#3b82f6",
      600: "#2563eb",
      700: "#1d4ed8",
      800: "#1e40af",
      900: "#1e3a8a",
      950: "#172554",
    },
  },
  violet: {
    label: "Violet",
    swatch: "#8b5cf6",
    scale: {
      50: "#f5f3ff",
      100: "#ede9fe",
      200: "#ddd6fe",
      300: "#c4b5fd",
      400: "#a78bfa",
      500: "#8b5cf6",
      600: "#7c3aed",
      700: "#6d28d9",
      800: "#5b21b6",
      900: "#4c1d95",
      950: "#2e1065",
    },
  },
  teal: {
    label: "Teal",
    swatch: "#14b8a6",
    scale: {
      50: "#f0fdfa",
      100: "#ccfbf1",
      200: "#99f6e4",
      300: "#5eead4",
      400: "#2dd4bf",
      500: "#14b8a6",
      600: "#0d9488",
      700: "#0f766e",
      800: "#115e59",
      900: "#134e4a",
      950: "#042f2e",
    },
  },
  emerald: {
    label: "Emerald",
    swatch: "#10b981",
    scale: {
      50: "#ecfdf5",
      100: "#d1fae5",
      200: "#a7f3d0",
      300: "#6ee7b7",
      400: "#34d399",
      500: "#10b981",
      600: "#059669",
      700: "#047857",
      800: "#065f46",
      900: "#064e3b",
      950: "#022c22",
    },
  },
  rose: {
    label: "Rose",
    swatch: "#f43f5e",
    scale: {
      50: "#fff1f2",
      100: "#ffe4e6",
      200: "#fecdd3",
      300: "#fda4af",
      400: "#fb7185",
      500: "#f43f5e",
      600: "#e11d48",
      700: "#be123c",
      800: "#9f1239",
      900: "#881337",
      950: "#4c0519",
    },
  },
  orange: {
    label: "Orange",
    swatch: "#f97316",
    scale: {
      50: "#fff7ed",
      100: "#ffedd5",
      200: "#fed7aa",
      300: "#fdba74",
      400: "#fb923c",
      500: "#f97316",
      600: "#ea580c",
      700: "#c2410c",
      800: "#9a3412",
      900: "#7c2d12",
      950: "#431407",
    },
  },
  pink: {
    label: "Pink",
    swatch: "#ec4899",
    scale: {
      50: "#fdf2f8",
      100: "#fce7f3",
      200: "#fbcfe8",
      300: "#f9a8d4",
      400: "#f472b6",
      500: "#ec4899",
      600: "#db2777",
      700: "#be185d",
      800: "#9d174d",
      900: "#831843",
      950: "#500724",
    },
  },
  indigo: {
    label: "Indigo",
    swatch: "#6366f1",
    scale: {
      50: "#eef2ff",
      100: "#e0e7ff",
      200: "#c7d2fe",
      300: "#a5b4fc",
      400: "#818cf8",
      500: "#6366f1",
      600: "#4f46e5",
      700: "#4338ca",
      800: "#3730a3",
      900: "#312e81",
      950: "#1e1b4e",
    },
  },
  cyan: {
    label: "Cyan",
    swatch: "#06b6d4",
    scale: {
      50: "#ecfeff",
      100: "#cffafe",
      200: "#a5f3fc",
      300: "#67e8f9",
      400: "#22d3ee",
      500: "#06b6d4",
      600: "#0891b2",
      700: "#0e7490",
      800: "#155e75",
      900: "#164e63",
      950: "#083344",
    },
  },
};

// ─── Gray tone palettes ─────────────────────────────────────

export const GRAY_PALETTES: Record<
  string,
  { label: string; swatch: string; scale: ColorScale }
> = {
  warm: {
    label: "Warm",
    swatch: "#8a847c",
    scale: {
      50: "#faf9f7",
      100: "#f0eeeb",
      200: "#e0ddd8",
      300: "#c8c4be",
      400: "#a8a39b",
      500: "#8a847c",
      600: "#6b665f",
      700: "#514d47",
      800: "#363330",
      900: "#1f1d1b",
      950: "#13120f",
    },
  },
  slate: {
    label: "Slate",
    swatch: "#64748b",
    scale: {
      50: "#f8fafc",
      100: "#f1f5f9",
      200: "#e2e8f0",
      300: "#cbd5e1",
      400: "#94a3b8",
      500: "#64748b",
      600: "#475569",
      700: "#334155",
      800: "#1e293b",
      900: "#0f172a",
      950: "#020617",
    },
  },
  zinc: {
    label: "Zinc",
    swatch: "#71717a",
    scale: {
      50: "#fafafa",
      100: "#f4f4f5",
      200: "#e4e4e7",
      300: "#d4d4d8",
      400: "#a1a1aa",
      500: "#71717a",
      600: "#52525b",
      700: "#3f3f46",
      800: "#27272a",
      900: "#18181b",
      950: "#09090b",
    },
  },
  neutral: {
    label: "Neutral",
    swatch: "#737373",
    scale: {
      50: "#fafafa",
      100: "#f5f5f5",
      200: "#e5e5e5",
      300: "#d4d4d4",
      400: "#a3a3a3",
      500: "#737373",
      600: "#525252",
      700: "#404040",
      800: "#262626",
      900: "#171717",
      950: "#0a0a0a",
    },
  },
  stone: {
    label: "Stone",
    swatch: "#78716c",
    scale: {
      50: "#fafaf9",
      100: "#f5f5f4",
      200: "#e7e5e4",
      300: "#d6d3d1",
      400: "#a8a29e",
      500: "#78716c",
      600: "#57534e",
      700: "#44403c",
      800: "#292524",
      900: "#1c1917",
      950: "#0c0a09",
    },
  },
};

// ─── Font families ──────────────────────────────────────────

export const FONT_BODY_OPTIONS: Record<
  string,
  { label: string; value: string }
> = {
  figtree: {
    label: "Figtree",
    value: "'Figtree', system-ui, -apple-system, sans-serif",
  },
  system: {
    label: "System",
    value: "system-ui, -apple-system, 'Segoe UI', sans-serif",
  },
  inter: {
    label: "Inter",
    value: "'Inter', system-ui, -apple-system, sans-serif",
  },
};

export const FONT_MONO_OPTIONS: Record<
  string,
  { label: string; value: string }
> = {
  jetbrains: {
    label: "JetBrains Mono",
    value: "'JetBrains Mono', 'SF Mono', monospace",
  },
  fira: { label: "Fira Code", value: "'Fira Code', 'SF Mono', monospace" },
  source: {
    label: "Source Code Pro",
    value: "'Source Code Pro', 'SF Mono', monospace",
  },
  system: {
    label: "System Mono",
    value: "'SF Mono', 'Cascadia Code', 'Consolas', monospace",
  },
};

// ─── Density scales ─────────────────────────────────────────

export const DENSITY_SCALES: Record<
  string,
  { label: string; description: string; multiplier: number }
> = {
  compact: {
    label: "Compact",
    description: "Tighter spacing, more content visible",
    multiplier: 0.8,
  },
  default: { label: "Default", description: "Balanced spacing", multiplier: 1 },
  comfortable: {
    label: "Comfortable",
    description: "More breathing room",
    multiplier: 1.2,
  },
};

// ─── Shadow presets ─────────────────────────────────────────

export const SHADOW_OPTIONS: Record<string, { label: string; value: string }> =
  {
    none: { label: "None", value: "none" },
    subtle: { label: "Subtle", value: "0 1px 2px rgba(0,0,0,0.06)" },
    medium: { label: "Medium", value: "0 2px 8px rgba(0,0,0,0.12)" },
  };
