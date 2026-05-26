import { useState, useCallback, useEffect, useRef } from "react";
import { useEscapeClose } from "./useEscapeClose";

interface UseDebounceSearchOpts<T> {
  /**
   * Run a search for `query`. Return a Promise of results, or `null` to
   * clear results without hitting any backend (useful for empty-query
   * branches like "do nothing when query is whitespace").
   */
  searchFn: (query: string) => Promise<T[]> | null;
  /**
   * Optional: when set, runs once on mount (no debounce). Re-runs whenever
   * any value in `loadOnMountDeps` changes. Return `null` to skip.
   */
  loadOnMount?: () => Promise<T[]> | null;
  loadOnMountDeps?: ReadonlyArray<unknown>;
  /** Debounce window in ms. Default 150. */
  debounceMs?: number;
  /** Called with the selected result item. */
  onSelect: (result: T) => void;
  /** Called on Escape (via `useEscapeClose`) and right before `onSelect`. */
  onClose: () => void;
}

/**
 * Generic debounced search hook with keyboard navigation.
 *
 * Owns the query/result/selectedIndex state and handles:
 *   - debounced backend calls (caller controls how a query maps to results),
 *   - optional mount-load (immediate, no debounce),
 *   - keyboard nav (ArrowUp/Down with wrap, Enter to select),
 *   - Escape-to-close (via `useEscapeClose`).
 *
 * Concrete hooks (`useFileSearch`, `useContentSearch`) wrap this and decide:
 *   - what `searchFn` returns for empty/whitespace queries,
 *   - whether to load-on-mount,
 *   - the debounce window.
 */
export function useDebounceSearch<T>({
  searchFn,
  loadOnMount,
  loadOnMountDeps = [],
  debounceMs = 150,
  onSelect,
  onClose,
}: UseDebounceSearchOpts<T>) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<T[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEscapeClose(onClose);

  // Mount-load — runs immediately (no debounce) when deps change.
  // The function is intentionally not in the deps list to mirror the
  // behaviour of the original hooks, which only re-loaded on the
  // semantic dep (e.g. vaultPath).
  useEffect(() => {
    if (!loadOnMount) return;
    const promise = loadOnMount();
    if (!promise) return;
    promise.then(setResults).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, loadOnMountDeps);

  const handleSearch = useCallback(
    (q: string) => {
      setQuery(q);
      setSelectedIndex(0);
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => {
        const promise = searchFn(q);
        if (promise === null) {
          setResults([]);
          return;
        }
        promise.then(setResults).catch(() => setResults([]));
      }, debounceMs);
    },
    [searchFn, debounceMs],
  );

  const handleSelect = useCallback(
    (index: number) => {
      const result = results[index];
      if (result) {
        onClose();
        onSelect(result);
      }
    },
    [results, onSelect, onClose],
  );

  const safeIndex =
    results.length === 0 ? 0 : Math.min(selectedIndex, results.length - 1);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((safeIndex + 1) % results.length);
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((safeIndex + results.length - 1) % results.length);
      }
      if (e.key === "Enter") {
        e.preventDefault();
        handleSelect(safeIndex);
      }
    },
    [safeIndex, results.length, handleSelect],
  );

  return {
    query,
    results,
    selectedIndex,
    safeIndex,
    setSelectedIndex,
    handleSearch,
    handleSelect,
    handleKeyDown,
  };
}
