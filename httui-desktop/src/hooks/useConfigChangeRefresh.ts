import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

/**
 * Subscribe to the backend `config-changed` event and invoke
 * `onChange` whenever the emitted `category` matches. The backend
 * emits this when a watched config file (or its `.local` sibling)
 * changes on disk.
 *
 * Handles the async-listen race: if the component unmounts before
 * `listen()` resolves, the resolved unlisten is invoked immediately
 * so no stale subscription survives.
 *
 * Extracted from the verbatim effect that lived in
 * Connections/Variables/Environments PageContainers (audit 01 §8).
 */
export function useConfigChangeRefresh(
  category: string,
  onChange: () => void,
): void {
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    void (async () => {
      const fn = await listen<{ category: string }>("config-changed", (e) => {
        if (e.payload.category === category) {
          onChange();
        }
      });
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [category, onChange]);
}
