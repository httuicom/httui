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

import { useMemo, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";

import {
  getDocHeaderEntries,
  getDocHeaderPortalVersion,
  subscribeToDocHeaderPortals,
} from "@/lib/codemirror/cm-doc-header";
import { DocHeaderContext } from "@/components/layout/docheader/doc-header-context";
import {
  DocHeaderShell,
  type DocHeaderShellProps,
} from "@/components/layout/docheader/DocHeaderShell";
import { buildDocHeaderCallbacks } from "./doc-header-callbacks";

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
  | "onTitleNavigateToBody"
  | "onAddPreflightCheck"
  | "onEditPreflightCheck"
  | "onRemovePreflightCheck"
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

  // The action callbacks live in `doc-header-callbacks.ts` so the
  // per-action branches (no-view bail, dup-tag skip, missing-tag skip,
  // navigate-to-body) get unit coverage without rendering the portal +
  // round-tripping through the registry.
  const callbacks = useMemo(
    () => buildDocHeaderCallbacks(entry, instanceId),
    [entry, instanceId],
  );

  const view = entry?.view ?? null;
  const liveFrontmatter = entry?.frontmatter ?? null;

  if (!entry) return null;
  return createPortal(
    <DocHeaderContext.Provider value={ctx}>
      <DocHeaderShell
        {...inlineHeader}
        frontmatter={liveFrontmatter}
        blockCount={entry.blockCount}
        onTitleSave={view ? callbacks.onTitleSave : undefined}
        onAbstractSave={view ? callbacks.onAbstractSave : undefined}
        onAddTag={view ? callbacks.onAddTag : undefined}
        onRemoveTag={view ? callbacks.onRemoveTag : undefined}
        onChecklistSave={view ? callbacks.onChecklistSave : undefined}
        onTitleNavigateToBody={view ? callbacks.onTitleNavigateToBody : undefined}
        onAddPreflightCheck={view ? callbacks.onAddPreflightCheck : undefined}
        onEditPreflightCheck={view ? callbacks.onEditPreflightCheck : undefined}
        onRemovePreflightCheck={
          view ? callbacks.onRemovePreflightCheck : undefined
        }
      />
    </DocHeaderContext.Provider>,
    entry.container,
  );
}
