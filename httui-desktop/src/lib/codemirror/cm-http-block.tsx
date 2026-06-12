/**
 * CodeMirror extension for HTTP block rendering (stage 3 of the HTTP block
 * redesign — see `docs/http-block-redesign.md`).
 *
 * Post-A3: the generic skeleton (scanner, registry, decorations, keymap,
 * StateField, ref autocomplete) lives in `widget-portal-registry.ts` +
 * `createFencedBlockExtension.tsx`. This file owns only the HTTP-specific
 * spec: the open-fence regex, the method/header/form body decoration, the
 * `HttpFormPortalWidget` (no DB equivalent), and the keymap bindings
 * (HTTP includes Mod-Shift-c for copy-as-cURL).
 *
 * Public exports are preserved exactly (RULE 4): `findHttpBlocks`,
 * `createHttpBlockExtension`,
 * `subscribeToHttpPortals`, `getHttpPortalVersion`,
 * `getHttpWidgetContainers`, `setHttpBlockActions`, plus the `Http*`
 * types — all consumed by `HttpFencedPanel`, `block-portal-registry`,
 * `block-registry`, and the test suite.
 */

import type { EditorState } from "@codemirror/state";
import type { Text as CMText } from "@codemirror/state";

import {
  parseHttpFenceInfo,
  type HttpBlockMetadata,
} from "@/lib/blocks/http-message";
import {
  WidgetPortalRegistry,
  type FencedBlockBase,
  type PortalEntryOf,
} from "@/lib/codemirror/widget-portal-registry";
import {
  buildFenceDecorations,
  createFencedBlockExtension,
  makeFencedScanner,
  type PushItem,
} from "@/lib/codemirror/createFencedBlockExtension";
import { Decoration } from "@codemirror/view";

// ───── Types ─────

export interface HttpFencedBlock extends FencedBlockBase {
  metadata: HttpBlockMetadata;
}

export type HttpWidgetSlot = "toolbar" | "form" | "result" | "statusbar";

export interface HttpPortalActions {
  /** Run the block. Called by ⌘↵ or the toolbar ▶ button. */
  onRun?: () => void;
  /** Cancel an in-flight run. Called by ⌘. or the toolbar ⏹ button. */
  onCancel?: () => void;
  /** Open the settings drawer. Called by the ⚙ button. */
  onOpenSettings?: () => void;
  /** Copy the request as a cURL one-liner. Called by ⌘⇧C. */
  onCopyAsCurl?: () => void;
}

export type HttpPortalEntry = PortalEntryOf<
  HttpWidgetSlot,
  HttpPortalActions,
  HttpFencedBlock
>;

const HTTP_OPEN_RE = /^```http(.*)$/;

// ───── Scanner ─────

const scanner = makeFencedScanner<
  HttpFencedBlock,
  Pick<HttpFencedBlock, "info" | "metadata">
>({
  openRe: HTTP_OPEN_RE,
  parse: (match) => {
    const info = match[1].trim();
    const metadata = parseHttpFenceInfo(`http ${info}`.trim()) ?? {};
    return { info, metadata };
  },
});

export function findHttpBlocks(doc: CMText): HttpFencedBlock[] {
  return scanner.findBlocks(doc);
}

// ───── Registry (module-level singleton — preserved behavior) ─────

const registry = new WidgetPortalRegistry<
  HttpWidgetSlot,
  HttpPortalActions,
  HttpFencedBlock
>({
  idPrefix: "http_idx_",
  slots: ["toolbar", "form", "result", "statusbar"],
  metaChanged: (prev, next) =>
    prev.metadata.alias !== next.metadata.alias ||
    prev.metadata.timeoutMs !== next.metadata.timeoutMs ||
    prev.metadata.displayMode !== next.metadata.displayMode ||
    prev.metadata.mode !== next.metadata.mode,
  // Body changes used to be debounced via `scheduleBodyNotify` but the
  // 250ms gap created a visible hole in the form view: a pending row
  // promoted to committed disappeared until the debounce fired and the
  // panel's `parsed` re-derived. Immediate notify + React.memo on the
  // panel keeps the cascade cheap.
  bodyChangePolicy: "immediate",
  // Same-element re-registers skip notify so reading-mode re-renders
  // don't reanimate the CodeMirror inputs in the form view (visible flash).
  dedupeSameSlotElement: true,
});

export const subscribeToHttpPortals = registry.subscribe;
export const getHttpPortalVersion = registry.getVersion;
export const getHttpWidgetContainers = registry.getContainers;
export const setHttpBlockActions = registry.setBlockActions;

// ───── Widgets (generic factories instantiated with HTTP class names) ─────

const HttpToolbarPortalWidget = registry.slotWidget(
  "toolbar",
  "cm-http-toolbar-portal",
  44,
);

const HttpClosePanelWidget = registry.closePanelWidget({
  wrapClass: "cm-http-close-panel",
  spacerClass: "cm-http-fence-hidden",
  resultClass: "cm-http-result-portal",
  statusClass: "cm-http-statusbar-portal",
  resultSlot: "result",
  statusSlot: "statusbar",
  fallbackHeight: 60,
});

/**
 * Form-mode body widget. When `metadata.mode === "form"` and the cursor is
 * OUTSIDE the block, this widget replaces the body lines and lets the
 * React panel mount a tabular Params/Headers editor inside it.
 *
 * `eq` compares blockId only (inherited from the slot factory) — re-mounting
 * on body changes would lose React form state (focused input, scroll). The
 * body is rendered from `entry.block`, which `syncBlocks` keeps current.
 */
const HttpFormPortalWidget = registry.slotWidget(
  "form",
  "cm-http-form-portal",
  200,
);

// ───── HTTP-specific body decoration ─────

function decorateHttpBody(
  state: EditorState,
  block: HttpFencedBlock,
  blockId: string,
  editing: boolean,
  push: PushItem,
): void {
  // Form-mode body replacement (reading mode only — editing always shows
  // raw text for direct keyboard editing, regardless of persisted mode).
  const formMode = block.metadata.mode === "form";
  if (block.body.length > 0 && !editing && formMode) {
    push({
      from: block.bodyFrom,
      to: block.bodyTo,
      deco: Decoration.replace({
        widget: new HttpFormPortalWidget(blockId, block),
        block: true,
      }),
      order: 0,
    });
    return;
  }
  if (block.body.length === 0) return;

  // Layout line classes only (line numbering, padding, edit state).
  // Syntax coloring comes from the @httui/lezer-http language injected
  // through markdown `codeLanguages`.
  const firstBodyLine = state.doc.lineAt(block.bodyFrom).number;
  const lastBodyLine = state.doc.lineAt(block.bodyTo).number;
  for (let n = firstBodyLine; n <= lastBodyLine; n++) {
    const line = state.doc.line(n);
    const classes = ["cm-http-body-line"];
    if (editing) classes.push("cm-http-body-editing");
    if (n === firstBodyLine) classes.push("cm-http-body-line-first");
    if (n === lastBodyLine) classes.push("cm-http-body-line-last");

    push({
      from: line.from,
      to: line.from,
      deco: Decoration.line({ class: classes.join(" ") }),
      order: 0,
    });
  }
}

// ───── Public extension factory ─────

export function createHttpBlockExtension() {
  return createFencedBlockExtension({
    scanner,
    registry,
    buildDecorations: (state, blocks) =>
      buildFenceDecorations(state, blocks, {
        registry,
        classPrefix: "cm-http",
        ToolbarWidget: HttpToolbarPortalWidget,
        ClosePanelWidget: HttpClosePanelWidget,
        decorateBody: decorateHttpBody,
      }),
    keymapBindings: [
      { key: "Mod-Enter", action: "onRun", requireHandler: true },
      { key: "Mod-.", action: "onCancel", requireHandler: true },
      { key: "Mod-Shift-c", action: "onCopyAsCurl", requireHandler: true },
    ],
  });
}

// ───── Autocomplete source ─────

/**
 * `{{ref}}` completion inside an HTTP block body — block aliases above the
 * cursor + non-secret env-variable keys.
 */
