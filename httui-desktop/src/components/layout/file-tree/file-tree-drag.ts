// Resolves a DnD drag-end event to `{ sourcePath, targetDir }` or `null`
// (no target, self-drop, missing data, descendant drop).
// Extracted from FileTree.tsx so the logic gets unit coverage without DnD pointer events.

import type { DragEndEvent } from "@dnd-kit/core";

export interface ResolvedDrop {
  sourcePath: string;
  targetDir: string;
}

export function resolveFileTreeDrop(event: DragEndEvent): ResolvedDrop | null {
  const { active, over } = event;
  if (!over || active.id === over.id) return null;

  const sourcePath = active.data.current?.path as string | undefined;
  const targetDir = over.data.current?.dirPath as string | undefined;
  if (!sourcePath || targetDir === undefined) return null;

  if (sourcePath === targetDir) return null;
  if (targetDir.startsWith(sourcePath + "/")) return null;

  return { sourcePath, targetDir };
}
