// V6 / cenário 9 — fetches the pre-flight evaluation for the active
// file and exposes the `PreflightPillItem[]` the DocHeader pill row
// expects.
//
// Re-runs on:
//  - file switch (`filePath` change)
//  - dirty → clean rising edge (a save just succeeded; same heuristic
//    `DocHeaderedEditor` uses for the mtime chip).
//  - explicit `recheck()` call from the consumer (Re-check button).
//
// Errors fall through silently — the hook returns an empty list so
// the pill row hides itself, matching the V2 cenário 4.5 contract.

import { useCallback, useEffect, useRef, useState } from "react";

import {
  evaluatePreflight,
  type EvaluatedPreflightItem,
  type PreflightItemKind,
} from "@/lib/tauri/preflight";
import type { PreflightPillItem } from "@/components/blocks/preflight/PreflightPills";

interface UseFilePreflightArgs {
  filePath: string;
  vaultPath: string | null;
  /** When the editor flips from dirty to clean (auto-save resolved),
   *  the consumer passes the new value here so the hook can re-fetch
   *  without needing to inspect the file content. */
  dirty: boolean;
}

export interface UseFilePreflightResult {
  items: PreflightPillItem[];
  rechecking: boolean;
  recheck: () => void;
}

export function useFilePreflight({
  filePath,
  vaultPath,
  dirty,
}: UseFilePreflightArgs): UseFilePreflightResult {
  const [items, setItems] = useState<PreflightPillItem[]>([]);
  const [rechecking, setRechecking] = useState(false);

  const cancelRef = useRef(false);

  const run = useCallback(async () => {
    if (!filePath || !vaultPath) {
      setItems([]);
      return;
    }
    cancelRef.current = false;
    setRechecking(true);
    try {
      const raw = await evaluatePreflight(filePath, vaultPath);
      if (cancelRef.current) return;
      setItems(raw.map(toPillItem));
    } catch {
      if (!cancelRef.current) setItems([]);
    } finally {
      if (!cancelRef.current) setRechecking(false);
    }
  }, [filePath, vaultPath]);

  // Initial fetch + re-fetch on file switch.
  useEffect(() => {
    cancelRef.current = false;
    void run();
    return () => {
      cancelRef.current = true;
    };
  }, [run]);

  // Re-fetch on the dirty → clean rising edge (save resolved).
  const prevDirtyRef = useRef(dirty);
  useEffect(() => {
    if (prevDirtyRef.current && !dirty) {
      void run();
    }
    prevDirtyRef.current = dirty;
  }, [dirty, run]);

  return { items, rechecking, recheck: run };
}

function toPillItem(raw: EvaluatedPreflightItem, idx: number): PreflightPillItem {
  return {
    id: `${idx}-${raw.kind}-${raw.label}`,
    label: raw.label,
    result: raw.result,
    suggestion: suggestionFor(raw.kind, raw.label),
  };
}

function suggestionFor(
  kind: PreflightItemKind,
  label: string,
): string | undefined {
  switch (kind) {
    case "connection":
      return `Add connection "${label}" in Connections`;
    case "env_var":
      return `Set env var "${label}" in the active environment`;
    case "branch":
      return `Switch to branch "${label}" in Git panel`;
    case "keychain":
      return `Add keychain entry "${label}"`;
    case "file_exists":
      return `Create or restore file at "${label}"`;
    case "command":
      return `Install "${label}" or add it to PATH`;
    case "unknown":
      return undefined;
  }
}
