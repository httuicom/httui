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

  // Re-fetch on every auto-save. `unsavedFiles` is a mutated Set (perf), so
  // we can't observe the dirty→clean edge directly; `saveSignal` is the
  // reactive proxy bumped by `notifySaved` after each `writeNote`.
  const saveSignal = usePaneStore((s) => s.saveSignal);
  const isFirstSignalRef = useRef(true);
  useEffect(() => {
    // Skip mount — the effect above already ran.
    if (isFirstSignalRef.current) {
      isFirstSignalRef.current = false;
      return;
    }
    void run();
  }, [saveSignal, run]);

  return { items, rechecking, recheck: run };
}

function toPillItem(
  raw: EvaluatedPreflightItem,
  idx: number,
): PreflightPillItem {
  // `unknown` kind can't round-trip to a valid PreflightCheck; pill stays read-only.
  const editable = raw.kind !== "unknown";
  return {
    id: `${idx}-${raw.kind}-${raw.label}`,
    label: raw.label,
    result: raw.result,
    suggestion: suggestionFor(raw.kind, raw.label),
    kind: editable
      ? (raw.kind as Exclude<PreflightItemKind, "unknown">)
      : undefined,
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
    case "file_exists":
      return `Create or restore file at "${label}"`;
    case "command":
      return `Install "${label}" or add it to PATH`;
    case "unknown":
      return undefined;
  }
}
