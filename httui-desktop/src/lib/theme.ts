import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";

import {
  FONT_MARKDOWN_BODY,
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
// in `<button>` backgrounds, `Menu.Content`, and `_hover`.
const pair = (dark: string, light: string) => ({
  value: { _dark: dark, _light: light },
});

const config = defineConfig({
  theme: {
    tokens: {
      fonts: {
        body: { value: FONT_SANS },
        heading: { value: FONT_SERIF },
        markdown: { value: FONT_MARKDOWN_BODY },
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
        // Sobrescreve os semantic tokens DEFAULT do Chakra v3 com a
        // ramp Fuji. Vantagem: todos os recipes internos (Menu,
        // Popover, Tooltip, Card, Badge, etc.) já referenciam esses
        // nomes — nossas cores fluem sem nenhum override por slot.
        // Mapping:
        //   bg          (canvas)   ← THEME.bg
        //   bg.subtle   (sutil)    ← THEME.bg1
        //   bg.muted    (médio)    ← THEME.bg2
        //   bg.emphasized          ← THEME.bg3
        //   bg.panel    (popups)   ← THEME.bgHi
        //   fg          (texto)    ← THEME.fg
        //   fg.muted               ← THEME.fg2
        //   fg.subtle              ← THEME.fg3
        //   border                 ← THEME.line
        //   border.muted           ← THEME.lineSoft
        //   brand.fg               ← THEME.accent     (era token "accent")
        //   brand.contrast         ← THEME.accentFg   (era token "accent.fg")
        //   brand.subtle           ← THEME.accentSoft (era token "accent.soft")
        // Chakra v3 não inclui `brand` nos semantic tokens defaults.
        // Definimos aqui com fallback Fuji; lib/theme/apply.ts
        // sobrescreve via --chakra-colors-brand-* quando o user
        // customiza accent palette.
        bg: {
          DEFAULT: { value: pair(THEME_DARK.bg, THEME_LIGHT.bg) },
          subtle: { value: pair(THEME_DARK.bg1, THEME_LIGHT.bg1) },
          muted: { value: pair(THEME_DARK.bg2, THEME_LIGHT.bg2) },
          emphasized: { value: pair(THEME_DARK.bg3, THEME_LIGHT.bg3) },
          panel: { value: pair(THEME_DARK.bgHi, THEME_LIGHT.bgHi) },
        },
        fg: {
          DEFAULT: { value: pair(THEME_DARK.fg, THEME_LIGHT.fg) },
          muted: { value: pair(THEME_DARK.fg2, THEME_LIGHT.fg2) },
          subtle: { value: pair(THEME_DARK.fg3, THEME_LIGHT.fg3) },
        },
        border: {
          DEFAULT: { value: pair(THEME_DARK.line, THEME_LIGHT.line) },
          muted: {
            value: pair(THEME_DARK.lineSoft, THEME_LIGHT.lineSoft),
          },
        },
        brand: {
          fg: { value: pair(THEME_DARK.accent, THEME_LIGHT.accent) },
          contrast: {
            value: pair(THEME_DARK.accentFg, THEME_LIGHT.accentFg),
          },
          subtle: {
            value: pair(THEME_DARK.accentSoft, THEME_LIGHT.accentSoft),
          },
          muted: {
            value: pair(THEME_DARK.accentSoft, THEME_LIGHT.accentSoft),
          },
          emphasized: {
            value: pair(THEME_DARK.accent, THEME_LIGHT.accent),
          },
          solid: { value: pair(THEME_DARK.accent, THEME_LIGHT.accent) },
          focusRing: {
            value: pair(THEME_DARK.accent, THEME_LIGHT.accent),
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
