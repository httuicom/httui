// vault-wide tag index store.
//
// Map of `tag → Set<file_path>` plus per-file reverse index. The
// consumer (vault-open Tauri walker) pushes tags via
// `setTagsForFile(filePath, tags)` after parsing each `.md` file's
// frontmatter; the file watcher calls the same mutation
// on save. `removeFile(filePath)` drops a file's tags on delete /
// rename-away. The Zustand state itself uses plain records (not
// `Map`/`Set`) so React reactivity works without manual replace.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

import { extractFrontmatter } from "@/lib/blocks/extract-frontmatter-tags";
import { scanVaultTags } from "@/lib/tauri/tags";

interface TagIndexState {
  /** tag → record of file_path → true (use as a set; values are
   *  `true` so insertion is `bag[path] = true` cheap). */
  byTag: Readonly<Record<string, Readonly<Record<string, true>>>>;
  /** file_path → tags currently applied; lets us compute the diff
   *  cheaply on `setTagsForFile`. */
  byFile: Readonly<Record<string, ReadonlyArray<string>>>;
  /** files whose frontmatter has `status: archived`.
   *  Consulted by the file tree to hide the row by default; the
   *  Sidebar toggle reveals them with an `archived` badge. Updated
   *  alongside `byFile` on save (per-edit) and on vault open (the
   *  Rust scanner extension that lands in a follow-up). */
  archivedFiles: Readonly<Record<string, true>>;
  setTagsForFile: (filePath: string, tags: ReadonlyArray<string>) => void;
  /** flip the per-file archived flag without touching
   *  tags. The save flow funnels through `refreshTagsForFile`; this
   *  setter is exported for tests + future watcher integration. */
  setArchivedForFile: (filePath: string, archived: boolean) => void;
  /** Per-save shortcut: parses `content` with `extractFrontmatter`
   *  and forwards to `setTagsForFile` + `setArchivedForFile`. Called
   *  from the editor save flow + the Tauri file-watcher hook so the
   *  index stays in sync without an IPC round-trip through the Rust
   *  walker. Returns the freshly-applied tag list so the consumer can
   *  log / surface it. */
  refreshTagsForFile: (filePath: string, content: string) => string[];
  removeFile: (filePath: string) => void;
  clearAll: () => void;
  /** Bootstrap from the Rust vault walker. Calls
   *  `scan_vault_tags_cmd` for `vaultPath`, replaces every existing
   *  entry, and returns the number of files indexed. Used at
   *  vault-open / vault-switch. Per-file refreshes (file watcher,
   *  post-save) keep using `setTagsForFile`. */
  loadFromVault: (vaultPath: string) => Promise<number>;
  /** Read accessors — return arrays for clean consumer ergonomics. */
  getFilesByTag: (tag: string) => string[];
  getAllTags: () => string[];
  /** true when the file's frontmatter has
   *  `status: archived`. Stable accessor for non-reactive readers. */
  isArchived: (filePath: string) => boolean;
}

export const useTagIndexStore = create<TagIndexState>()(
  devtools(
    (set, get) => ({
      byTag: {},
      byFile: {},
      archivedFiles: {},

      setTagsForFile: (filePath, tags) =>
        set(
          (state) => {
            const oldTags = state.byFile[filePath] ?? [];
            const oldSet = new Set(oldTags);
            const newSet = new Set(tags);
            const toRemove: string[] = [];
            const toAdd: string[] = [];
            for (const t of oldSet) if (!newSet.has(t)) toRemove.push(t);
            for (const t of newSet) if (!oldSet.has(t)) toAdd.push(t);

            const nextByTag: Record<string, Record<string, true>> = {};
            for (const [tag, paths] of Object.entries(state.byTag)) {
              nextByTag[tag] = { ...paths };
            }
            for (const tag of toRemove) {
              const bag = nextByTag[tag];
              if (!bag) continue;
              const rest = { ...bag };
              delete rest[filePath];
              if (Object.keys(rest).length === 0) {
                delete nextByTag[tag];
              } else {
                nextByTag[tag] = rest;
              }
            }
            for (const tag of toAdd) {
              const bag = nextByTag[tag] ?? {};
              nextByTag[tag] = { ...bag, [filePath]: true };
            }

            const nextByFile = { ...state.byFile, [filePath]: tags.slice() };
            return { byTag: nextByTag, byFile: nextByFile };
          },
          undefined,
          "tag-index/setTagsForFile",
        ),

      refreshTagsForFile: (filePath, content) => {
        const fm = extractFrontmatter(content);
        get().setTagsForFile(filePath, fm.tags);
        get().setArchivedForFile(filePath, fm.status === "archived");
        return fm.tags;
      },

      setArchivedForFile: (filePath, archived) =>
        set(
          (state) => {
            const wasArchived = state.archivedFiles[filePath] === true;
            if (archived === wasArchived) return state;
            if (archived) {
              return {
                archivedFiles: { ...state.archivedFiles, [filePath]: true },
              };
            }
            const rest = { ...state.archivedFiles };
            delete rest[filePath];
            return { archivedFiles: rest };
          },
          undefined,
          "tag-index/setArchivedForFile",
        ),

      removeFile: (filePath) =>
        set(
          (state) => {
            const wasIndexed =
              state.byFile[filePath] !== undefined ||
              state.archivedFiles[filePath] === true;
            if (!wasIndexed) return state;
            const nextByTag: Record<string, Record<string, true>> = {};
            for (const [tag, paths] of Object.entries(state.byTag)) {
              if (!paths[filePath]) {
                nextByTag[tag] = paths;
                continue;
              }
              const rest = { ...paths };
              delete rest[filePath];
              if (Object.keys(rest).length === 0) {
                continue;
              }
              nextByTag[tag] = rest;
            }
            const nextByFile = { ...state.byFile };
            delete nextByFile[filePath];
            const nextArchived = { ...state.archivedFiles };
            delete nextArchived[filePath];
            return {
              byTag: nextByTag,
              byFile: nextByFile,
              archivedFiles: nextArchived,
            };
          },
          undefined,
          "tag-index/removeFile",
        ),

      clearAll: () =>
        set(
          { byTag: {}, byFile: {}, archivedFiles: {} },
          undefined,
          "tag-index/clearAll",
        ),

      loadFromVault: async (vaultPath) => {
        const entries = await scanVaultTags(vaultPath);
        // Build maps in one pass to avoid N individual store updates.
        const byTag: Record<string, Record<string, true>> = {};
        const byFile: Record<string, ReadonlyArray<string>> = {};
        for (const entry of entries) {
          if (
            !entry ||
            typeof entry.path !== "string" ||
            !Array.isArray(entry.tags)
          ) {
            continue;
          }
          // Dedup tags within a file before indexing.
          const uniq = Array.from(new Set(entry.tags));
          byFile[entry.path] = uniq;
          for (const tag of uniq) {
            const bag = byTag[tag] ?? {};
            bag[entry.path] = true;
            byTag[tag] = bag;
          }
        }
        set({ byTag, byFile }, undefined, "tag-index/loadFromVault");
        return Object.keys(byFile).length;
      },

      getFilesByTag: (tag) => Object.keys(get().byTag[tag] ?? {}).sort(),
      getAllTags: () => Object.keys(get().byTag).sort(),
      isArchived: (filePath) => get().archivedFiles[filePath] === true,
    }),
    { name: "tag-index" },
  ),
);
