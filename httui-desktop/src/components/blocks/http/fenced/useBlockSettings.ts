/**
 * Loads and persists per-block settings (Onda 1).
 *
 * Settings live in the SQLite `block_settings` table keyed by
 * `(file_path, block_alias)`. We mirror that row in component state so the
 * drawer renders without an extra round-trip on every keystroke; updates
 * are debounced via React's batching but each call still issues an
 * upsert (the table row is small, the user toggles infrequently).
 *
 * Returns `[settings, setSettings]`. `setSettings` accepts a *partial*
 * patch — fields you omit keep their current value. Pass `undefined` for
 * a field to revert it to the default.
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

  // Reload whenever the (file, alias) target changes. Without an alias the
  // settings have no stable key — fall back to defaults and skip I/O.
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
        // Best-effort upsert. Errors silently fall back to in-memory state —
        // the user can re-toggle if the write failed (rare; SQLite local).
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
