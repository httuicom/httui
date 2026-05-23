import type { VarUseEntry } from "@/lib/tauri/var-uses";

export interface UsedInBlocksHit {
  line: number;
  snippet: string;
}

export interface UsedInBlocksGroup {
  filePath: string;
  hits: ReadonlyArray<UsedInBlocksHit>;
}

export function groupVarUsesByFile(
  entries: ReadonlyArray<VarUseEntry>,
): ReadonlyArray<UsedInBlocksGroup> {
  const groups: UsedInBlocksGroup[] = [];
  let current: { filePath: string; hits: UsedInBlocksHit[] } | null = null;
  for (const entry of entries) {
    if (!current || current.filePath !== entry.file_path) {
      current = { filePath: entry.file_path, hits: [] };
      groups.push(current);
    }
    current.hits.push({ line: entry.line, snippet: entry.snippet });
  }
  return groups;
}
