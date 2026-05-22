// pure builder for the editable + navigation
// callbacks that `DocHeaderWidgetPortal` passes through to
// `DocHeaderShell`. Extracted from the portal so the per-action
// branches (no-view bail, dup-tag skip, missing-tag skip) get unit
// coverage without rendering the portal + the registry round-trip.
//
// The portal stays a thin React shell (createPortal +
// useSyncExternalStore) and delegates per-callback wiring here.

import type { EditorView } from "@codemirror/view";

import { saveActiveFileNow } from "@/lib/active-file-save";
import {
  dispatchDocReplace,
  returnFocusToBody,
  type DocHeaderEntry,
} from "@/lib/codemirror/cm-doc-header";
import type { TaskItem } from "@/lib/blocks/task-item";
import {
  extractPreflightChecks,
  updateFrontmatterPreflightChecks,
  type PreflightCheck,
} from "@/lib/blocks/preflight-checks";
import {
  updateFrontmatterAbstract,
  updateFrontmatterTasks,
  updateFrontmatterTags,
  updateFrontmatterTitle,
} from "@/lib/blocks/update-frontmatter";

export interface DocHeaderCallbacks {
  onTitleSave: (title: string) => void;
  onAbstractSave: (abstract: string) => void;
  onAddTag: (tag: string) => void;
  onRemoveTag: (tag: string) => void;
  onChecklistSave: (items: TaskItem[]) => void;
  onTitleNavigateToBody: () => void;
  /** append a typed pre-flight check to the
   *  `preflight:` block-list. */
  onAddPreflightCheck: (check: PreflightCheck) => void;
  /** replace the check at index `idx`. */
  onEditPreflightCheck: (idx: number, next: PreflightCheck) => void;
  /** drop the check at index `idx`. */
  onRemovePreflightCheck: (idx: number) => void;
}

/** The portal reads these helpers off `cm-doc-header`; tests can swap
 *  them via the second arg to avoid going through the live CM6
 *  module. */
export interface CallbackDeps {
  dispatchDocReplace: (view: EditorView, next: string) => void;
  returnFocusToBody: (instanceId: string) => void;
  /** Flush the active file's pending content to disk synchronously
   *  so derived UI (pill row, tag dots, etc.) can re-fetch without
   *  waiting for the auto-save debounce. */
  flushSave: () => void;
}

const defaultDeps: CallbackDeps = {
  dispatchDocReplace,
  returnFocusToBody,
  flushSave: saveActiveFileNow,
};

/** Build the six action callbacks for an entry/instance pair. The
 *  entry can be `undefined` (registry not seeded yet) — every editable
 *  callback then becomes a no-op, matching the portal's old behaviour
 *  of gating commits behind `entry.view` presence. */
export function buildDocHeaderCallbacks(
  entry: DocHeaderEntry | undefined,
  instanceId: string,
  deps: CallbackDeps = defaultDeps,
): DocHeaderCallbacks {
  const onTitleSave = (title: string) => {
    const v = entry?.view;
    if (!v) return;
    const next = updateFrontmatterTitle(v.state.doc.toString(), title);
    deps.dispatchDocReplace(v, next);
    deps.flushSave();
  };

  const onAbstractSave = (abstract: string) => {
    const v = entry?.view;
    if (!v) return;
    const next = updateFrontmatterAbstract(v.state.doc.toString(), abstract);
    deps.dispatchDocReplace(v, next);
    deps.flushSave();
  };

  const onAddTag = (tag: string) => {
    const v = entry?.view;
    if (!v) return;
    const trimmed = tag.trim();
    if (trimmed.length === 0) return;
    const current = entry?.frontmatter?.tags ?? [];
    if (current.includes(trimmed)) return;
    const next = updateFrontmatterTags(v.state.doc.toString(), [
      ...current,
      trimmed,
    ]);
    deps.dispatchDocReplace(v, next);
    deps.flushSave();
  };

  const onRemoveTag = (tag: string) => {
    const v = entry?.view;
    if (!v) return;
    const current = entry?.frontmatter?.tags ?? [];
    const next = current.filter((t) => t !== tag);
    if (next.length === current.length) return;
    const nextContent = updateFrontmatterTags(v.state.doc.toString(), next);
    deps.dispatchDocReplace(v, nextContent);
    deps.flushSave();
  };

  const onChecklistSave = (items: TaskItem[]) => {
    const v = entry?.view;
    if (!v) return;
    const next = updateFrontmatterTasks(v.state.doc.toString(), items);
    deps.dispatchDocReplace(v, next);
    deps.flushSave();
  };

  const onTitleNavigateToBody = () => {
    deps.returnFocusToBody(instanceId);
  };

  const onAddPreflightCheck = (check: PreflightCheck) => {
    const v = entry?.view;
    if (!v) return;
    const doc = v.state.doc.toString();
    const current = extractPreflightChecks(doc);
    const next = updateFrontmatterPreflightChecks(doc, [...current, check]);
    deps.dispatchDocReplace(v, next);
    deps.flushSave();
  };

  const onEditPreflightCheck = (idx: number, replacement: PreflightCheck) => {
    const v = entry?.view;
    if (!v) return;
    const doc = v.state.doc.toString();
    const current = extractPreflightChecks(doc);
    if (idx < 0 || idx >= current.length) return;
    const next = current.map((c, i) => (i === idx ? replacement : c));
    const nextDoc = updateFrontmatterPreflightChecks(doc, next);
    deps.dispatchDocReplace(v, nextDoc);
    deps.flushSave();
  };

  const onRemovePreflightCheck = (idx: number) => {
    const v = entry?.view;
    if (!v) return;
    const doc = v.state.doc.toString();
    const current = extractPreflightChecks(doc);
    if (idx < 0 || idx >= current.length) return;
    const next = current.filter((_, i) => i !== idx);
    const nextDoc = updateFrontmatterPreflightChecks(doc, next);
    deps.dispatchDocReplace(v, nextDoc);
    deps.flushSave();
  };

  return {
    onTitleSave,
    onAbstractSave,
    onAddTag,
    onRemoveTag,
    onChecklistSave,
    onTitleNavigateToBody,
    onAddPreflightCheck,
    onEditPreflightCheck,
    onRemovePreflightCheck,
  };
}
