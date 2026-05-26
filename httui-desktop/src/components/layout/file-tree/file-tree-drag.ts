// Pure resolver for the DnD drag-end event in the file tree. Returns
// `null` when the drop should be ignored (no over target, dropped on
// self, missing data, descendant drop) or `{ sourcePath, targetDir }`
// when `handleMoveFile` should fire. Extracted from `FileTree.tsx`
// so the branch logic gets unit coverage without simulating DnD
// pointer events.

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

  // Prevent dropping into self or descendant.
  if (sourcePath === targetDir) return null;
  if (targetDir.startsWith(sourcePath + "/")) return null;

  return { sourcePath, targetDir };
}
