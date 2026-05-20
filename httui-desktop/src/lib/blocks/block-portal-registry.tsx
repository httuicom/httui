/**
 * Block-type registry — React/Panel half (A5). Each entry binds a
 * `WidgetPortalRegistry` trio (subscribe/getVersion/getContainers) +
 * a `Panel` component and exposes `renderPortal(view, filePath)` so
 * `MarkdownEditor.tsx` can iterate without naming any block type.
 *
 * Kept separate from `block-registry.ts` (CM-level) so that
 * `markdown-extensions.test.ts` — which mocks the cm-*-block modules
 * but not the heavy Fenced Panels — can pull the CM registry without
 * also dragging HttpFencedPanel/DbFencedPanel + their Chakra/CM
 * dependency trees through the test bundler.
 *
 * Each entry's `id` MUST match a `BlockTypeSpec.id` in `block-
 * registry.ts`. New block types add a row to BOTH lists (the audit's
 * "edit ≥5 files" violation collapses to "edit 2 lists in 2 files").
 */

import type { ReactElement } from "react";
import type { EditorView } from "@codemirror/view";

import { BlockWidgetPortals } from "@/components/editor/BlockWidgetPortals";

import { DbFencedPanel } from "@/components/blocks/db/fenced/DbFencedPanel";
import {
  getDbPortalVersion,
  getDbWidgetContainers,
  subscribeToDbPortals,
} from "@/lib/codemirror/cm-db-block";

import { HttpFencedPanel } from "@/components/blocks/http/fenced/HttpFencedPanel";
import {
  getHttpPortalVersion,
  getHttpWidgetContainers,
  subscribeToHttpPortals,
} from "@/lib/codemirror/cm-http-block";

export interface BlockPortalEntry {
  /** Matches a `BlockTypeSpec.id` in `block-registry.ts`. */
  id: string;
  /** Pre-bound portal renderer — caller passes only view/filePath. */
  renderPortal: (view: EditorView, filePath: string) => ReactElement;
}

export const blockPortals: BlockPortalEntry[] = [
  {
    id: "db",
    renderPortal: (view, filePath) => (
      <BlockWidgetPortals
        view={view}
        filePath={filePath}
        subscribe={subscribeToDbPortals}
        getVersion={getDbPortalVersion}
        getContainers={getDbWidgetContainers}
        Panel={DbFencedPanel}
      />
    ),
  },
  {
    id: "http",
    renderPortal: (view, filePath) => (
      <BlockWidgetPortals
        view={view}
        filePath={filePath}
        subscribe={subscribeToHttpPortals}
        getVersion={getHttpPortalVersion}
        getContainers={getHttpWidgetContainers}
        Panel={HttpFencedPanel}
      />
    ),
  },
];
