// Tiny UI store for the ⌘E env switcher (V11 cenário 1).
//
// The dropdown lives in the StatusBar (`EnvSwitcher`) but the ⌘E
// shortcut is wired in AppShell — they're far apart in the tree, so
// the open flag is shared state rather than a prop drill. In-memory
// only; never persisted, never crosses a Tauri command.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

interface EnvSwitcherState {
  /** Whether the ⌘E env-switcher dropdown is open. */
  open: boolean;
  /** Open the dropdown (⌘E). */
  openSwitcher: () => void;
  /** Close the dropdown (selection / Esc / outside-click). */
  closeSwitcher: () => void;
  /** Controlled-component bridge for Chakra `Menu.Root.onOpenChange`. */
  setOpen: (open: boolean) => void;
}

export const useEnvSwitcherStore = create<EnvSwitcherState>()(
  devtools(
    (set) => ({
      open: false,
      openSwitcher: () => set({ open: true }, false, "envSwitcher/open"),
      closeSwitcher: () => set({ open: false }, false, "envSwitcher/close"),
      setOpen: (open) => set({ open }, false, "envSwitcher/setOpen"),
    }),
    { name: "env-switcher-store" },
  ),
);
