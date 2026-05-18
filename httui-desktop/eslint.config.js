import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "src/components/ui"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,

      // The two stable react-hooks rules stay as errors:
      //   - rules-of-hooks
      //   - exhaustive-deps
      // The remaining rules from the v6 recommended preset are React
      // Compiler diagnostics. Several of them flag intentional patterns
      // in this codebase (refs mutated during render to keep callbacks
      // referentially stable, setState in an effect to derive a clock,
      // etc). Demote to warnings so they stay visible without gating
      // CI; revisit in epic 04 when the compiler config is decided.
      "react-hooks/static-components": "warn",
      "react-hooks/use-memo": "warn",
      "react-hooks/preserve-manual-memoization": "warn",
      "react-hooks/incompatible-library": "warn",
      "react-hooks/immutability": "warn",
      "react-hooks/globals": "warn",
      "react-hooks/refs": "warn",
      "react-hooks/set-state-in-effect": "warn",
      "react-hooks/error-boundaries": "warn",
      "react-hooks/purity": "warn",
      "react-hooks/set-state-in-render": "warn",
      "react-hooks/unsupported-syntax": "warn",
      "react-hooks/config": "warn",
      "react-hooks/gating": "warn",

      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],

      // SOLID nudges at function granularity. Companion to the file-size
      // gate (`scripts/size-check.sh` ≤600 L) which only catches whole
      // files. These four rules surface SRP/complexity smells inside a
      // file (a 400-line component made of 5 fat hooks slips past the
      // size gate but should still flag for review).
      //
      // All as `warn`: the existing monoliths (HttpFencedPanel,
      // DbFencedPanel, AuditSection, MarkdownEditor — all on
      // tech-debt.md) generate ~90 hits today. Flipping to `error`
      // would either block every PR touching those files or force a
      // wave of `// eslint-disable-next-line` graffiti. Warnings keep
      // the signal during code review without gating CI; a later
      // refactor is where they get retired.
      //
      // Thresholds chosen pragmatically:
      // - complexity 15: clippy uses cognitive_complexity = 50 in
      //   Rust; cyclomatic 15 is the JS-side equivalent (lower because
      //   cyclomatic counts simpler branches than cognitive).
      // - max-lines-per-function 150: React function components with
      //   JSX legitimately reach 80-120 L; 150 is roughly "two screens
      //   of code", which is the SRP smell threshold.
      // - max-params 5: Sandi Metz rule. Tauri RPC wrappers
      //   occasionally violate (Rust positional signatures mirror to
      //   TS); fix by promoting the params to an object on both sides.
      // - max-depth 4: rare to need more without an extracted helper.
      complexity: ["warn", 15],
      "max-lines-per-function": [
        "warn",
        { max: 150, skipBlankLines: true, skipComments: true, IIFEs: true },
      ],
      "max-params": ["warn", 5],
      "max-depth": ["warn", 4],
    },
  },
  // Test files: large `describe(() => { ... })` arrows are an
  // idiomatic vitest layout, not SRP debt. Mirrors the test-file
  // exclusion in scripts/size-check.sh.
  {
    files: [
      "**/__tests__/**",
      "**/*.test.ts",
      "**/*.test.tsx",
      "**/*.spec.ts",
      "**/*.spec.tsx",
      "**/test/**",
    ],
    rules: {
      "max-lines-per-function": "off",
      complexity: "off",
      "max-depth": "off",
      "max-params": "off",
    },
  },
);
