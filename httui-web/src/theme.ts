import { createSystem, defaultConfig, defineConfig } from "@chakra-ui/react";

/**
 * httui — "Fuji" design system
 *
 * Palette extracted from a photograph of Mount Fuji:
 *   - turquoise sky                → sky / info accent
 *   - warm snow on the peak        → fg in dark / paper-warm bg in light
 *   - deep stone-blue mountain     → bg in dark / fg ink in light
 *   - moss-green forest            → ok / success
 *   - canola yellow flower field   → primary accent
 *   - sunset rose                  → error
 *
 * Headings use Source Serif 4 (LaTeX-style book serif).
 * UI uses Geist; code uses Geist Mono.
 */

const config = defineConfig({
  theme: {
    tokens: {
      fonts: {
        body: { value: "'Geist', system-ui, -apple-system, sans-serif" },
        heading: {
          value:
            "'Source Serif 4', 'Source Serif Pro', 'Iowan Old Style', Georgia, serif",
        },
        mono: {
          value: "'Geist Mono', 'JetBrains Mono', 'SF Mono', monospace",
        },
      },
      letterSpacings: {
        tighter: { value: "-0.03em" },
        tight: { value: "-0.02em" },
        snug: { value: "-0.01em" },
        wide: { value: "0.06em" },
        wider: { value: "0.08em" },
      },
      colors: {
        // Stone-blue mountain (dark base) — mirrors design's --bg/--bg-1/-2/-3/-hi
        stone: {
          50: { value: "oklch(0.96 0.005 230)" },
          100: { value: "oklch(0.90 0.008 230)" },
          200: { value: "oklch(0.78 0.010 230)" },
          300: { value: "oklch(0.62 0.012 230)" },
          400: { value: "oklch(0.46 0.014 230)" },
          500: { value: "oklch(0.295 0.020 230)" }, // --bg-hi
          600: { value: "oklch(0.245 0.017 230)" }, // --bg-3
          700: { value: "oklch(0.215 0.015 230)" }, // --bg-2
          800: { value: "oklch(0.185 0.013 230)" }, // --bg-1
          900: { value: "oklch(0.16 0.012 230)" }, // --bg
          950: { value: "oklch(0.12 0.010 230)" },
        },
        // Paper-warm (light base + warm snow)
        paper: {
          50: { value: "oklch(0.99 0.004 90)" },
          100: { value: "oklch(0.985 0.006 90)" },
          200: { value: "oklch(0.972 0.008 90)" },
          300: { value: "oklch(0.948 0.010 90)" },
          400: { value: "oklch(0.910 0.012 90)" },
          500: { value: "oklch(0.870 0.014 90)" },
          600: { value: "oklch(0.78 0.016 90)" },
          700: { value: "oklch(0.62 0.018 90)" },
          800: { value: "oklch(0.46 0.020 90)" },
          900: { value: "oklch(0.30 0.022 90)" },
        },
        // Deep Fuji-blue ink (light fg)
        ink: {
          50: { value: "oklch(0.96 0.010 240)" },
          100: { value: "oklch(0.90 0.014 240)" },
          200: { value: "oklch(0.80 0.020 240)" },
          300: { value: "oklch(0.66 0.014 240)" },
          400: { value: "oklch(0.50 0.022 240)" },
          500: { value: "oklch(0.34 0.030 240)" },
          600: { value: "oklch(0.20 0.040 240)" },
          700: { value: "oklch(0.16 0.040 240)" },
          800: { value: "oklch(0.12 0.038 240)" },
          900: { value: "oklch(0.08 0.030 240)" },
        },
        // Canola — primary accent
        canola: {
          50: { value: "oklch(0.98 0.020 95)" },
          100: { value: "oklch(0.95 0.060 95)" },
          200: { value: "oklch(0.93 0.100 95)" },
          300: { value: "oklch(0.88 0.130 95)" },
          400: { value: "oklch(0.84 0.160 90)" },
          500: { value: "oklch(0.78 0.160 95)" },
          600: { value: "oklch(0.66 0.150 90)" },
          700: { value: "oklch(0.52 0.130 90)" },
          800: { value: "oklch(0.40 0.110 90)" },
          900: { value: "oklch(0.32 0.060 90)" },
        },
        // Sky — turquoise highlight
        sky: {
          100: { value: "oklch(0.95 0.030 210)" },
          200: { value: "oklch(0.90 0.060 210)" },
          300: { value: "oklch(0.78 0.070 215)" },
          400: { value: "oklch(0.72 0.100 210)" },
          500: { value: "oklch(0.60 0.120 215)" },
          600: { value: "oklch(0.46 0.090 215)" },
          700: { value: "oklch(0.34 0.040 215)" },
        },
        // Moss — forest
        moss: {
          200: { value: "oklch(0.86 0.060 145)" },
          300: { value: "oklch(0.74 0.090 145)" },
          400: { value: "oklch(0.66 0.110 145)" },
          500: { value: "oklch(0.62 0.100 145)" },
          600: { value: "oklch(0.50 0.110 150)" },
          700: { value: "oklch(0.42 0.100 155)" },
          800: { value: "oklch(0.32 0.080 155)" },
        },
        // Sunset — error
        sunset: {
          300: { value: "oklch(0.78 0.130 25)" },
          400: { value: "oklch(0.72 0.160 18)" },
          500: { value: "oklch(0.66 0.180 15)" },
          600: { value: "oklch(0.55 0.160 18)" },
          700: { value: "oklch(0.44 0.130 20)" },
        },
      },
      radii: {
        sm: { value: "4px" },
        md: { value: "8px" },
        lg: { value: "12px" },
        xl: { value: "16px" },
        "2xl": { value: "20px" },
      },
    },

    semanticTokens: {
      colors: {
        // Surfaces
        bg: {
          DEFAULT: {
            value: { base: "{colors.paper.100}", _dark: "{colors.stone.900}" },
          },
          surface: {
            value: { base: "{colors.paper.200}", _dark: "{colors.stone.800}" },
          },
          elevated: {
            value: { base: "{colors.paper.300}", _dark: "{colors.stone.700}" },
          },
          subtle: {
            value: { base: "{colors.paper.400}", _dark: "{colors.stone.600}" },
          },
          muted: {
            value: { base: "{colors.paper.500}", _dark: "{colors.stone.500}" },
          },
        },
        // Borders — design --line / --line-soft
        border: {
          DEFAULT: {
            // light = oklch(0.870 0.010 90) ≈ paper.500; dark = oklch(0.285 0.014 230)
            value: {
              base: "{colors.paper.500}",
              _dark: "oklch(0.285 0.014 230)",
            },
          },
          subtle: {
            // light = oklch(0.928 0.008 90) (literal); dark = oklch(0.235 0.012 230)
            value: {
              base: "oklch(0.928 0.008 90)",
              _dark: "oklch(0.235 0.012 230)",
            },
          },
        },
        // Foreground
        fg: {
          DEFAULT: {
            value: { base: "{colors.ink.600}", _dark: "{colors.paper.100}" },
          },
          muted: {
            value: { base: "{colors.ink.500}", _dark: "{colors.paper.300}" },
          },
          subtle: {
            value: { base: "{colors.ink.400}", _dark: "{colors.stone.200}" },
          },
          disabled: {
            value: { base: "{colors.ink.300}", _dark: "{colors.stone.300}" },
          },
        },
        // Accent (canola)
        accent: {
          DEFAULT: {
            value: {
              base: "{colors.canola.500}",
              _dark: "{colors.canola.400}",
            },
          },
          fg: {
            // Design --accent-fg light = oklch(0.22 0.040 240); dark = oklch(0.18 0.04 90)
            value: {
              base: "oklch(0.22 0.040 240)",
              _dark: "oklch(0.18 0.040 90)",
            },
          },
          subtle: {
            // Design --accent-soft light = oklch(0.93 0.10 95); dark = oklch(0.32 0.06 90)
            value: {
              base: "{colors.canola.200}",
              _dark: "{colors.canola.900}",
            },
          },
          emphasized: {
            value: {
              base: "{colors.canola.600}",
              _dark: "{colors.canola.300}",
            },
          },
        },
        // Sky highlight
        sky: {
          DEFAULT: {
            value: { base: "{colors.sky.400}", _dark: "{colors.sky.300}" },
          },
          subtle: {
            value: { base: "{colors.sky.200}", _dark: "{colors.sky.700}" },
          },
        },
        // Moss / success
        moss: {
          DEFAULT: {
            value: { base: "{colors.moss.700}", _dark: "{colors.moss.400}" },
          },
          subtle: {
            value: { base: "{colors.moss.200}", _dark: "{colors.moss.800}" },
          },
        },
        ok: {
          DEFAULT: { value: "oklch(0.66 0.11 145)" }, // --ok
        },
        warn: {
          DEFAULT: { value: "oklch(0.78 0.15 75)" }, // --warn
        },
        err: {
          DEFAULT: { value: "oklch(0.66 0.18 15)" }, // --err
        },
        info: {
          DEFAULT: { value: "oklch(0.74 0.07 215)" }, // --info
        },
        method: {
          get: { value: "oklch(0.78 0.07 215)" }, // --m-get  (sky)
          post: { value: "oklch(0.62 0.10 145)" }, // --m-post (moss)
          put: { value: "oklch(0.78 0.13 80)" }, // --m-put  (canola pale)
          patch: { value: "oklch(0.74 0.15 60)" }, // --m-patch (ochre)
          delete: { value: "oklch(0.66 0.18 15)" }, // --m-del  (sunset)
        },
      },
      shadows: {
        // Soft photo shadow for the workbench preview / feature cards
        photo: {
          value: {
            base: "0 40px 100px -20px oklch(0.20 0.04 230 / 0.18), 0 12px 30px -10px oklch(0.20 0.04 230 / 0.12)",
            _dark:
              "0 40px 100px -20px oklch(0.05 0.02 230 / 0.7), 0 12px 30px -10px oklch(0.05 0.02 230 / 0.5)",
          },
        },
        card: {
          value: {
            base: "0 12px 40px -16px oklch(0.20 0.04 230 / 0.14)",
            _dark: "0 12px 40px -16px oklch(0.05 0.02 230 / 0.55)",
          },
        },
      },
    },
  },
  // next-themes uses class="dark" / class="light" on <html>
  conditions: {
    dark: ".dark &",
    light: ".light &",
  },
  globalCss: {
    "html, body": {
      bg: "bg",
      color: "fg",
      fontFamily: "body",
      ...({
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      } as Record<string, string>),
    },
    body: {
      fontFeatureSettings: "'ss01', 'cv11'",
    },
    "::selection": {
      bg: "accent.subtle",
      color: "fg",
    },
    code: {
      fontFamily: "mono",
    },
  },
});

export const system = createSystem(defaultConfig, config);
