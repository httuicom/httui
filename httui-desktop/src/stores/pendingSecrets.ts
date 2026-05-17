// V1 vertical 1, cenário 4 — first-run secrets modal store.
//
// Holds the list of `MissingRef`s scanned right after a vault is
// opened. The AppShell mounts a single modal driven by this store;
// per-entry Save calls remove that ref from `pending`, Skip
// dismisses the modal but keeps the refs around so a topbar/status
// indicator can re-open it later.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

import type { MissingRef } from "@/lib/tauri/commands";

interface PendingSecretsState {
  /** Refs we still need a value for. */
  pending: MissingRef[];
  /** Whether the modal is currently visible. */
  modalOpen: boolean;

  /** Replace the list and (when non-empty) auto-open the modal. */
  setPending: (refs: MissingRef[]) => void;
  /** Drop a single ref by its keychain_key (after Save succeeded). */
  removePending: (keychainKey: string) => void;
  /** Hide modal, keep refs so the indicator can re-open it. */
  dismiss: () => void;
  /** Re-open the modal if there are still pending refs. */
  reopen: () => void;
  /** Clear everything — used by tests / vault swap. */
  reset: () => void;
}

export const usePendingSecretsStore = create<PendingSecretsState>()(
  devtools(
    (set) => ({
      pending: [],
      modalOpen: false,

      setPending: (refs) => set({ pending: refs, modalOpen: refs.length > 0 }),

      removePending: (keychainKey) =>
        set((state) => {
          const next = state.pending.filter(
            (r) => r.keychain_key !== keychainKey,
          );
          return {
            pending: next,
            modalOpen: state.modalOpen && next.length > 0,
          };
        }),

      dismiss: () => set({ modalOpen: false }),

      reopen: () =>
        set((state) => ({
          modalOpen: state.pending.length > 0,
        })),

      reset: () => set({ pending: [], modalOpen: false }),
    }),
    { name: "pending-secrets-store" },
  ),
);
