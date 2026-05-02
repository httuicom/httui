import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";

import {
  FONT_MONO,
  FONT_SANS,
  FONT_SERIF,
  METHOD_COLORS,
  STATE_COLORS,
  THEME_DARK,
  THEME_LIGHT,
  TYPE_SCALE,
} from "@/theme/tokens";

// Semantic token entry: same shape as Chakra v3's built-in
// `defineSemanticTokens.colors` (see node_modules/@chakra-ui/react/
// dist/esm/theme/semantic-tokens/colors.js). `_dark` / `_light` map
// to Chakra's default conditions:
//   _dark  → ".dark &, .dark .chakra-theme:not(.light) &"
//   _light → ":root &, .light &"
// We rely on those defaults — overriding `conditions` previously
// caused dotted scale tokens (`bg.1`, `accent.soft`, `sel`) to fail
// in `<button>` backgrounds, `Menu.Content`, and `_hover`. See
// docs-llm/v2/known-issues.md#7 for the audit.
const pair = (dark: string, light: string) => ({
  value: { _dark: dark, _light: light },
});

const config = defineConfig({
  theme: {
    tokens: {
      fonts: {
        body: { value: FONT_SANS },
        heading: { value: FONT_SERIF },
        mono: { value: FONT_MONO },
        serif: { value: FONT_SERIF },
      },
      fontSizes: {
        xs: { value: TYPE_SCALE.xs },
        sm: { value: TYPE_SCALE.sm },
        base: { value: TYPE_SCALE.base },
        md: { value: TYPE_SCALE.md },
        lg: { value: TYPE_SCALE.lg },
        xl: { value: TYPE_SCALE.xl },
        "2xl": { value: TYPE_SCALE["2xl"] },
      },
      colors: {
        method: {
          get: { value: METHOD_COLORS.get },
          post: { value: METHOD_COLORS.post },
          put: { value: METHOD_COLORS.put },
          patch: { value: METHOD_COLORS.patch },
          delete: { value: METHOD_COLORS.delete },
          head: { value: METHOD_COLORS.head },
          options: { value: METHOD_COLORS.options },
          sql: { value: METHOD_COLORS.sql },
          mongo: { value: METHOD_COLORS.mongo },
          ws: { value: METHOD_COLORS.ws },
          gql: { value: METHOD_COLORS.gql },
          sh: { value: METHOD_COLORS.sh },
        },
        state: {
          ok: { value: STATE_COLORS.ok },
          warn: { value: STATE_COLORS.warn },
          err: { value: STATE_COLORS.err },
          info: { value: STATE_COLORS.info },
        },
      },
    },
    semanticTokens: {
      colors: {
        // Backgrounds (Fuji ramp). Nested-scale form is what Chakra
        // v3's built-in tokens use; flat dotted keys like "bg.1"
        // half-work and break in `<button>` bg / Menu / _hover paths.
        bg: {
          DEFAULT: { value: pair(THEME_DARK.bg, THEME_LIGHT.bg) },
          "1": { value: pair(THEME_DARK.bg1, THEME_LIGHT.bg1) },
          "2": { value: pair(THEME_DARK.bg2, THEME_LIGHT.bg2) },
          "3": { value: pair(THEME_DARK.bg3, THEME_LIGHT.bg3) },
          hi: { value: pair(THEME_DARK.bgHi, THEME_LIGHT.bgHi) },
        },
        // Lines.
        line: {
          DEFAULT: { value: pair(THEME_DARK.line, THEME_LIGHT.line) },
          soft: {
            value: pair(THEME_DARK.lineSoft, THEME_LIGHT.lineSoft),
          },
        },
        // Foregrounds.
        fg: {
          DEFAULT: { value: pair(THEME_DARK.fg, THEME_LIGHT.fg) },
          "1": { value: pair(THEME_DARK.fg1, THEME_LIGHT.fg1) },
          "2": { value: pair(THEME_DARK.fg2, THEME_LIGHT.fg2) },
          "3": { value: pair(THEME_DARK.fg3, THEME_LIGHT.fg3) },
        },
        // Accent.
        accent: {
          DEFAULT: { value: pair(THEME_DARK.accent, THEME_LIGHT.accent) },
          fg: {
            value: pair(THEME_DARK.accentFg, THEME_LIGHT.accentFg),
          },
          soft: {
            value: pair(THEME_DARK.accentSoft, THEME_LIGHT.accentSoft),
          },
        },
        // Selection.
        sel: { value: pair(THEME_DARK.sel, THEME_LIGHT.sel) },
      },
    },
    slotRecipes: {
      // Override Chakra v3's Menu recipe content slot — default `bg`
      // resolves to `colors.bg.panel` which falls back to gray.950 /
      // white from the Chakra default palette (não bate com nossa
      // ramp Fuji `bg.hi`). Apontamos `--menu-bg` direto pro nosso
      // token de contraste forte. Mesmo padrão recomendado em
      // chakra-ui.com/docs/theming/recipes#extending-recipes.
      menu: {
        slots: ["content"],
        base: {
          content: {
            "--menu-bg": "colors.bg.hi",
          },
        },
      },
    },
  },
  // No custom `conditions`: Chakra v3 defaults already match
  // `.dark` (and the LightMode/DarkMode wrapper class
  // `.chakra-theme.light/.dark`). Overriding broke nested-scale
  // resolution.
});

export const system = createSystem(defaultConfig, config);
