// Bridge between `useSettingsStore.colorMode` and Chakra/next-themes.
// `useColorMode().setColorMode` is React-only (hook), so the store
// can't call it directly. Mount this component once near the root —
// it watches the persisted preference and applies it to the
// `<html class="dark|light">` attribute that `lib/theme.ts`
// semanticTokens react to.

import { useEffect } from "react";
import { useTheme } from "next-themes";

import { useSettingsStore } from "@/stores/settings";

export function ColorModeSync() {
  const colorMode = useSettingsStore((s) => s.colorMode);
  const loaded = useSettingsStore((s) => s.loaded);
  const { setTheme } = useTheme();

  useEffect(() => {
    if (!loaded) return;
    // next-themes accepts the literal "system" sentinel; Chakra's
    // `useColorMode` wrapper narrows the type to "light" | "dark", so
    // we go through `useTheme()` directly to keep "system" usable.
    setTheme(colorMode);
  }, [colorMode, loaded, setTheme]);

  return null;
}
