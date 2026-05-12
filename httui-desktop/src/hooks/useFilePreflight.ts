// V6 / cenário 9 — fetches the pre-flight evaluation for the active
// file and exposes the `PreflightPillItem[]` the DocHeader pill row
// expects.
//
// Re-runs on:
//  - file switch (`filePath` change)
//  - every `saveSignal` bump from the pane store (auto-save resolved).
//    We can't observe `dirty` directly because `unsavedFiles` is a
//    mutated Set for keystroke perf and so doesn't notify React.
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
import { usePaneStore } from "@/stores/pane";

interface UseFilePreflightArgs {
  filePath: string;
  vaultPath: string | null;
}

export interface UseFilePreflightResult {
  items: PreflightPillItem[];
  rechecking: boolean;
  recheck: () => void;
}

export function useFilePreflight({
  filePath,
  vaultPath,
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

  // Re-fetch after every auto-save resolves. We can't observe the
  // dirty → clean edge through `dirty` because `unsavedFiles` is a
  // mutated Set (perf — see pane store). The auto-save flow bumps
  // `saveSignal` after `writeNote` lands; subscribing to it gives us
  // a reactive signal without changing the keystroke fast path.
  const saveSignal = usePaneStore((s) => s.saveSignal);
  const isFirstSignalRef = useRef(true);
  useEffect(() => {
    // Skip the first invocation — the initial-fetch effect above
    // already runs on mount; we only want the subsequent save bumps.
    if (isFirstSignalRef.current) {
      isFirstSignalRef.current = false;
      return;
    }
    void run();
  }, [saveSignal, run]);

  return { items, rechecking, recheck: run };
}

function toPillItem(raw: EvaluatedPreflightItem, idx: number): PreflightPillItem {
  // `unknown` items came from YAML the parser couldn't recognize — they
  // can't round-trip back to a valid `PreflightCheck` so the pill stays
  // read-only (kind/value undefined keeps `canEdit` false in
  // PreflightPills, falling back to suggestion-only behavior).
  const editable = raw.kind !== "unknown";
  return {
    id: `${idx}-${raw.kind}-${raw.label}`,
    label: raw.label,
    result: raw.result,
    suggestion: suggestionFor(raw.kind, raw.label),
    kind: editable ? (raw.kind as Exclude<PreflightItemKind, "unknown">) : undefined,
    value: editable ? raw.label : undefined,
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
