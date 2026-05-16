// Tiny UI store for the ⌘⇧V new-variable popover (V11 cenário 4).
//
// The shortcut is wired in AppShell; the popover is mounted there
// too. Shared open flag (in-memory only, never persisted) so the
// two stay decoupled — mirrors useEnvSwitcherStore.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

interface NewVariablePopoverState {
  open: boolean;
  openForm: () => void;
  closeForm: () => void;
  setOpen: (open: boolean) => void;
}

export const useNewVariablePopoverStore = create<NewVariablePopoverState>()(
  devtools(
    (set) => ({
      open: false,
      openForm: () => set({ open: true }, false, "newVar/open"),
      closeForm: () => set({ open: false }, false, "newVar/close"),
      setOpen: (open) => set({ open }, false, "newVar/setOpen"),
    }),
    { name: "new-variable-popover-store" },
  ),
);
