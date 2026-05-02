// V2 / cenário 4.5 — React mount for the DocHeader CM6 block widget.
//
// Mirrors `HttpWidgetPortals.tsx`: subscribes to the registry maintained
// by `cm-doc-header.tsx` and `createPortal`s a `<DocHeaderShell>` into
// the entry that matches this editor's `instanceId`.
//
// Owns the editable callbacks (title / abstract / tags / checklist).
// They dispatch directly into the CM6 view via `dispatchDocReplace`
// and let CM6's normal update cycle propagate the change — that fires
// the editor's onChange (which writes to the pane store's content
// map) AND triggers the `cm-doc-header` StateField (which re-parses
// the frontmatter and refreshes this portal). Going through CM6 keeps
// the React tree off the non-reactive `editorContents` Map: tags /
// checklist / title additions show up instantly without a reload.

import { useCallback, useMemo, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";

import {
  dispatchDocReplace,
  getDocHeaderEntries,
  getDocHeaderPortalVersion,
  subscribeToDocHeaderPortals,
} from "@/lib/codemirror/cm-doc-header";
import { DocHeaderContext } from "@/components/layout/docheader/doc-header-context";
import {
  DocHeaderShell,
  type DocHeaderShellProps,
} from "@/components/layout/docheader/DocHeaderShell";
import type { PreflightItem } from "@/lib/blocks/preflight-item";
import {
  updateFrontmatterAbstract,
  updateFrontmatterPreflight,
  updateFrontmatterTags,
  updateFrontmatterTitle,
} from "@/lib/blocks/update-frontmatter";

/** Subset of `DocHeaderShellProps` provided by the consumer
 *  (`DocHeaderedEditor`) — ambient metadata that doesn't depend on
 *  the live doc state. The portal merges these with editable
 *  callbacks + live frontmatter pulled from the CM6 entry. */
export type InlineDocHeader = Omit<
  DocHeaderShellProps,
  | "frontmatter"
  | "onTitleSave"
  | "onAbstractSave"
  | "onAddTag"
  | "onRemoveTag"
  | "onChecklistSave"
>;

interface DocHeaderWidgetPortalProps {
  instanceId: string;
  inlineHeader: InlineDocHeader;
}

function useDocHeaderPortalVersion(): number {
  return useSyncExternalStore(
    subscribeToDocHeaderPortals,
    getDocHeaderPortalVersion,
  );
}

export function DocHeaderWidgetPortal({
  instanceId,
  inlineHeader,
}: DocHeaderWidgetPortalProps) {
  const version = useDocHeaderPortalVersion();

  const entry = useMemo(
    () => getDocHeaderEntries().get(instanceId),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [version, instanceId],
  );

  const ctx = useMemo(() => ({ instanceId }), [instanceId]);

  // The view ref is read fresh on every callback invocation so a late
  // remount (e.g. file switch) doesn't leave us holding a stale view.
  const view = entry?.view ?? null;
  const liveFrontmatter = entry?.frontmatter ?? null;

  const onTitleSave = useCallback(
    (title: string) => {
      const v = entry?.view;
      if (!v) return;
      const next = updateFrontmatterTitle(v.state.doc.toString(), title);
      dispatchDocReplace(v, next);
    },
    [entry],
  );

  const onAbstractSave = useCallback(
    (abstract: string) => {
      const v = entry?.view;
      if (!v) return;
      const next = updateFrontmatterAbstract(v.state.doc.toString(), abstract);
      dispatchDocReplace(v, next);
    },
    [entry],
  );

  const onAddTag = useCallback(
    (tag: string) => {
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
      dispatchDocReplace(v, next);
    },
    [entry],
  );

  const onRemoveTag = useCallback(
    (tag: string) => {
      const v = entry?.view;
      if (!v) return;
      const current = entry?.frontmatter?.tags ?? [];
      const next = current.filter((t) => t !== tag);
      if (next.length === current.length) return;
      const nextContent = updateFrontmatterTags(v.state.doc.toString(), next);
      dispatchDocReplace(v, nextContent);
    },
    [entry],
  );

  const onChecklistSave = useCallback(
    (items: PreflightItem[]) => {
      const v = entry?.view;
      if (!v) return;
      const next = updateFrontmatterPreflight(v.state.doc.toString(), items);
      dispatchDocReplace(v, next);
    },
    [entry],
  );

  if (!entry) return null;
  return createPortal(
    <DocHeaderContext.Provider value={ctx}>
      <DocHeaderShell
        {...inlineHeader}
        frontmatter={liveFrontmatter}
        onTitleSave={view ? onTitleSave : undefined}
        onAbstractSave={view ? onAbstractSave : undefined}
        onAddTag={view ? onAddTag : undefined}
        onRemoveTag={view ? onRemoveTag : undefined}
        onChecklistSave={view ? onChecklistSave : undefined}
      />
    </DocHeaderContext.Provider>,
    entry.container,
  );
}
