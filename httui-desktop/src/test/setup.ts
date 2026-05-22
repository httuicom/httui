import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";

import { resetGitStore } from "@/stores/git";

// The git store is a module singleton. Without a reset between specs,
// status from a prior test would leak into the next render. Also tears
// down any leaked poll interval so timers never cross test boundaries.
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
