import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

import { resetGitStore } from "@/stores/git";

// V10.1 — the git store is a module singleton (status/remotes/commits
// are polled once and shared by every consumer). Without a reset
// between specs, a `status` from a prior test would leak into the
// next render — the V10 per-hook `useState` was naturally fresh per
// render, so we restore that contract globally here. Also tears down
// any leaked poll interval so timers never cross test boundaries.
afterEach(() => {
  resetGitStore();
});

// ResizeObserver mock — jsdom doesn't provide one
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// matchMedia mock — Chakra UI v3 (via next-themes) reads it at module
// load to detect the OS color-mode preference. jsdom doesn't ship one
// and the import side-effect throws `matchMedia is not a function`
// before any test runs. Always returns `matches: false` (light mode).
if (typeof window !== "undefined" && typeof window.matchMedia === "undefined") {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}
