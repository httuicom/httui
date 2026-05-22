// Bridges `useSettingsStore.colorMode` to Chakra/next-themes. The store
// can't call `setColorMode` directly (it's a hook), so this component
// watches the persisted preference and applies it to the html class attribute.

import { useEffect } from "react";
import { useTheme } from "next-themes";

import { useSettingsStore } from "@/stores/settings";

export function ColorModeSync() {
  const colorMode = useSettingsStore((s) => s.colorMode);
  const loaded = useSettingsStore((s) => s.loaded);
  const { setTheme } = useTheme();

  useEffect(() => {
    if (!loaded) return;
    // Use next-themes directly — Chakra's `useColorMode` wrapper narrows
    // the type to "light"|"dark", losing the "system" sentinel.
    setTheme(colorMode);
  }, [colorMode, loaded, setTheme]);

  return null;
}
