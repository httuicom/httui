import { searchFiles } from "@/lib/tauri/commands";
import type { SearchResult } from "@/lib/tauri/commands";
import {
  applyTagQuery,
  parseQuickOpenQuery,
} from "@/lib/blocks/quick-open-query";
import { useTagIndexStore } from "@/stores/tagIndex";
import { useDebounceSearch } from "./useDebounceSearch";

interface UseFileSearchOpts {
  vaultPath: string | null;
  onSelect: (filePath: string) => void;
  onClose: () => void;
}

/** Strip directory prefix and `.md` extension to produce the leaf
 *  label shown next to the tag-mode result. POSIX separators only —
 *  vaults are normalized on open. */
function leafFromPath(path: string): string {
  const lastSlash = path.lastIndexOf("/");
  const base = lastSlash === -1 ? path : path.slice(lastSlash + 1);
  return base.endsWith(".md") ? base.slice(0, -3) : base;
}

export function useFileSearch({
  vaultPath,
  onSelect,
  onClose,
}: UseFileSearchOpts) {
  const runQuery = (q: string): Promise<SearchResult[]> | null => {
    const parsed = parseQuickOpenQuery(q);
    if (parsed.kind === "tag" || parsed.kind === "tag-bool") {
      // Tag mode is fully synchronous — pull straight from the index
      // store without an IPC round-trip. The store is bootstrapped on
      // vault-open (`useTagIndexStore.loadFromVault`) and kept fresh
      // per-save by `useEditorSession`'s `refreshTagsForFile` call.
      const byTag = useTagIndexStore.getState().getFilesByTag;
      const paths = applyTagQuery(parsed, byTag);
      const results: SearchResult[] = paths.map((path) => ({
        path,
        name: leafFromPath(path),
        score: 0,
      }));
      return Promise.resolve(results);
    }
    if (!vaultPath) return null;
    return searchFiles(vaultPath, q);
  };

  return useDebounceSearch<SearchResult>({
    searchFn: runQuery,
    loadOnMount: vaultPath ? () => searchFiles(vaultPath, "") : undefined,
    loadOnMountDeps: [vaultPath],
    debounceMs: 100,
    onSelect: (r) => onSelect(r.path),
    onClose,
  });
}
