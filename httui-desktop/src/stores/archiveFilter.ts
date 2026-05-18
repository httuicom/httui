// UI-only toggle: should the file tree show notes with `status:
// archived` in their frontmatter?
//
// Persisted lazily in localStorage so the user's preference survives
// reloads. The store is intentionally tiny — just one boolean — but
// lives in its own module so the file-tree consumer can subscribe with
// fine-grained `useArchiveFilterStore((s) => s.showArchived)` reads.

import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";

interface ArchiveFilterState {
  showArchived: boolean;
  toggleShowArchived: () => void;
  setShowArchived: (show: boolean) => void;
}

export const useArchiveFilterStore = create<ArchiveFilterState>()(
  devtools(
    persist(
      (set) => ({
        showArchived: false,
        toggleShowArchived: () =>
          set(
            (s) => ({ showArchived: !s.showArchived }),
            undefined,
            "archive-filter/toggle",
          ),
        setShowArchived: (show) =>
          set({ showArchived: show }, undefined, "archive-filter/set"),
      }),
      { name: "archive-filter" },
    ),
    { name: "archive-filter" },
  ),
);
