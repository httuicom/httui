// V2 / cenário 4.5 — React mount for the DocHeader CM6 block widget.
//
// Mirrors `HttpWidgetPortals.tsx`: subscribes to the registry maintained
// by `cm-doc-header.tsx` and `createPortal`s a `<DocHeaderShell>` into
// the entry that matches this editor's `instanceId`. The instanceId is
// what disambiguates multiple concurrent editors — each `MarkdownEditor`
// owns one extension instance and passes its instanceId here.

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

export type InlineDocHeader = DocHeaderShellProps;

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

  if (!entry) return null;
  return createPortal(
    <DocHeaderContext.Provider value={ctx}>
      <DocHeaderShell {...inlineHeader} />
    </DocHeaderContext.Provider>,
    entry.container,
  );
}
