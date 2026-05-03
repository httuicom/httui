// V6 / cenário 3 — pure builder for the editable + navigation
// callbacks that `DocHeaderWidgetPortal` passes through to
// `DocHeaderShell`. Extracted from the portal so the per-action
// branches (no-view bail, dup-tag skip, missing-tag skip) get unit
// coverage without rendering the portal + the registry round-trip.
//
// The portal stays a thin React shell (createPortal +
// useSyncExternalStore) and delegates per-callback wiring here.

import type { EditorView } from "@codemirror/view";

import {
  dispatchDocReplace,
  returnFocusToBody,
  type DocHeaderEntry,
} from "@/lib/codemirror/cm-doc-header";
import type { PreflightItem } from "@/lib/blocks/preflight-item";
import {
  updateFrontmatterAbstract,
  updateFrontmatterPreflight,
  updateFrontmatterTags,
  updateFrontmatterTitle,
} from "@/lib/blocks/update-frontmatter";

export interface DocHeaderCallbacks {
  onTitleSave: (title: string) => void;
  onAbstractSave: (abstract: string) => void;
  onAddTag: (tag: string) => void;
  onRemoveTag: (tag: string) => void;
  onChecklistSave: (items: PreflightItem[]) => void;
  onTitleNavigateToBody: () => void;
}

/** The portal reads these helpers off `cm-doc-header`; tests can swap
 *  them via the second arg to avoid going through the live CM6
 *  module. */
export interface CallbackDeps {
  dispatchDocReplace: (view: EditorView, next: string) => void;
  returnFocusToBody: (instanceId: string) => void;
}

const defaultDeps: CallbackDeps = {
  dispatchDocReplace,
  returnFocusToBody,
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
  };

  const onAbstractSave = (abstract: string) => {
    const v = entry?.view;
    if (!v) return;
    const next = updateFrontmatterAbstract(v.state.doc.toString(), abstract);
    deps.dispatchDocReplace(v, next);
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
  };

  const onRemoveTag = (tag: string) => {
    const v = entry?.view;
    if (!v) return;
    const current = entry?.frontmatter?.tags ?? [];
    const next = current.filter((t) => t !== tag);
    if (next.length === current.length) return;
    const nextContent = updateFrontmatterTags(v.state.doc.toString(), next);
    deps.dispatchDocReplace(v, nextContent);
  };

  const onChecklistSave = (items: PreflightItem[]) => {
    const v = entry?.view;
    if (!v) return;
    const next = updateFrontmatterPreflight(v.state.doc.toString(), items);
    deps.dispatchDocReplace(v, next);
  };

  // V6 / cenário 3 — clicking the static H1 returns the cursor to the
  // first body line. Same path used by Enter / ArrowDown / Escape on
  // the editable input (V2 cenário 4.5 / M3).
  const onTitleNavigateToBody = () => {
    deps.returnFocusToBody(instanceId);
  };

  return {
    onTitleSave,
    onAbstractSave,
    onAddTag,
    onRemoveTag,
    onChecklistSave,
    onTitleNavigateToBody,
  };
}
