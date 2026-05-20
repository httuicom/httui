/**
 * Generic widget-portals mount (A4 — collapses `HttpWidgetPortals.tsx` +
 * `DbWidgetPortals.tsx` into one component dirigido pelo registry exposto
 * por cada extensão CM6).
 *
 * Each block type's extension owns a `WidgetPortalRegistry` (A3); this
 * component subscribes to one and renders the type's React Panel into
 * every entry. The Panel is supplied by the caller, so any future block
 * type plugs in by passing its own (subscribe/getVersion/getContainers,
 * Panel) trio — that's the A5 `BlockTypeModule` shape in embryo.
 */

import { useMemo, useSyncExternalStore, type ComponentType } from "react";
import type { EditorView } from "@codemirror/view";

/**
 * Shape every block-type's portal entry shares: a `block` payload + the
 * stable `blockId` used as React key. The slot fields (`toolbar`,
 * `result`, …) are read by the Panel itself, not here.
 */
interface PortalEntryShape {
  block: unknown;
}

export interface BlockPanelProps<Entry extends PortalEntryShape> {
  blockId: string;
  block: Entry["block"];
  entry: Entry;
  view: EditorView;
  filePath: string;
}

interface BlockWidgetPortalsProps<Entry extends PortalEntryShape> {
  view: EditorView;
  filePath: string;
  /** Registry hooks (bound to one block-type's `WidgetPortalRegistry`). */
  subscribe: (cb: () => void) => () => void;
  getVersion: () => number;
  getContainers: () => ReadonlyMap<string, Entry>;
  /** Type's React Panel — `HttpFencedPanel` / `DbFencedPanel` / future. */
  Panel: ComponentType<BlockPanelProps<Entry>>;
}

export function BlockWidgetPortals<Entry extends PortalEntryShape>({
  view,
  filePath,
  subscribe,
  getVersion,
  getContainers,
  Panel,
}: BlockWidgetPortalsProps<Entry>) {
  const version = useSyncExternalStore(subscribe, getVersion);

  const entries = useMemo(
    () => Array.from(getContainers().entries()),
    // `version` is the explicit signal that the registry mutated; the
    // getter itself is stable (a bound class method). exhaustive-deps
    // would push us to add `getContainers`, which never changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [version, getContainers],
  );

  return (
    <>
      {entries.map(([blockId, entry]) => (
        <Panel
          key={blockId}
          blockId={blockId}
          // Pass block separately so React.memo detects when the scanner
          // updates it — `entry` is a stable ref, but `entry.block` swaps
          // on every doc edit via `syncBlocks`.
          block={entry.block}
          entry={entry}
          view={view}
          filePath={filePath}
        />
      ))}
    </>
  );
}
