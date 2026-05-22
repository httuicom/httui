/**
 * Loads and persists per-block settings from the SQLite `block_settings` table
 * keyed by `(file_path, block_alias)`. Mirrors the row in component state so
 * the drawer renders without a round-trip on every keystroke.
 *
 * Returns `[settings, setSettings]`. `setSettings` accepts a partial patch;
 * pass `undefined` for a field to revert to the default.
 */
/* eslint-disable react-hooks/set-state-in-effect --
 * The settings live outside React (SQLite). The effect mirrors the
 * external row into local state for synchronous reads — exactly the case
 * the rule warns about but cannot avoid.
 */
import { useCallback, useEffect, useState } from "react";

import {
  getBlockSettings,
  upsertBlockSettings,
  type HttpBlockSettings,
} from "@/lib/tauri/commands";

export type SetBlockSettings = (patch: Partial<HttpBlockSettings>) => void;

const EMPTY: HttpBlockSettings = {};

export function useBlockSettings(
  filePath: string,
  alias: string | undefined,
): [HttpBlockSettings, SetBlockSettings] {
  const [settings, setLocal] = useState<HttpBlockSettings>(EMPTY);

  // Without an alias there's no stable key — fall back to defaults and skip I/O.
  useEffect(() => {
    let cancelled = false;
    if (!alias) {
      setLocal((prev) => (prev === EMPTY ? prev : EMPTY));
      return () => {
        cancelled = true;
      };
    }
    (async () => {
      try {
        const loaded = await getBlockSettings(filePath, alias);
        if (!cancelled) setLocal(loaded ?? EMPTY);
      } catch {
        if (!cancelled) setLocal((prev) => (prev === EMPTY ? prev : EMPTY));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [filePath, alias]);

  const setSettings = useCallback<SetBlockSettings>(
    (patch) => {
      setLocal((prev) => {
        const next = { ...prev, ...patch };
        // Best-effort upsert; errors fall back to in-memory state silently.
        if (alias) {
          void upsertBlockSettings(filePath, alias, next).catch(() => {});
        }
        return next;
      });
    },
    [filePath, alias],
  );

  return [settings, setSettings];
}
