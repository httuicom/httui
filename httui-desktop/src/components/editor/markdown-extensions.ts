// Builds the CM6 extensions array for `MarkdownEditor`.
//
// Extracted from the editor component so the long extension stack is
// testable in isolation and the React shell stays focused on lifecycle
// concerns. `buildExtensions` is intentionally pure: every input it
// needs is passed via params, and the returned value is a fresh array
// of CM6 extensions ready for `<CodeMirror extensions={...} />`.

import { EditorView, keymap } from "@codemirror/view";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages as cmLanguages } from "@codemirror/language-data";
import { syntaxHighlighting, bracketMatching } from "@codemirror/language";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
  startCompletion,
} from "@codemirror/autocomplete";
import {
  search,
  highlightSelectionMatches,
  searchKeymap,
} from "@codemirror/search";

import { hybridRendering } from "@/lib/codemirror/cm-hybrid-rendering";
import { mergeConflict } from "@/lib/codemirror/cm-merge-conflict";
import {
  slashCommands,
  slashCompletionSource,
  slashIconOption,
} from "@/lib/codemirror/cm-slash-commands";
import { editorTheme } from "@/components/editor/editor-theme";
import { blockRegistry } from "@/lib/blocks/block-registry";
import {
  wikilinks,
  createWikilinkCompletion,
} from "@/lib/codemirror/cm-wikilinks";
import { tables } from "@/lib/codemirror/cm-tables";
import { moveBlocksKeymap } from "@/lib/codemirror/cm-move-blocks";
import { referenceHighlight } from "@/lib/blocks/cm-references";
import { refClickExtension } from "@/lib/blocks/cm-ref-popover";
import { serverCompletionSource } from "@codemirror/lsp-client";

import type { FileEntry } from "@/lib/tauri/commands";
import { vimCompartment, docLineNavKeymap } from "./markdown-vim-motions";
import {
  dbSqlLanguages,
  markdownHighlightStyle,
} from "./markdown-highlight-style";

export interface DocHeaderHandleLike {
  extension: import("@codemirror/state").Extension;
  instanceId: string;
}

export interface BuildExtensionsParams {
  filePath: string;
  entriesRef: { readonly current: FileEntry[] };
  handleFileSelectRef: { readonly current: (path: string) => void };
  docHeaderHandle: DocHeaderHandleLike | null;
}

// Walk the workspace tree and yield every leaf .md file. Used both to
// resolve `[[wikilinks]]` and as completion source. Lives next to the
// extension factory because both consumers are in this file.
export function flattenFiles(
  entries: FileEntry[],
): { name: string; path: string }[] {
  const result: { name: string; path: string }[] = [];
  for (const entry of entries) {
    if (!entry.is_dir && entry.name.endsWith(".md")) {
      result.push({ name: entry.name, path: entry.path });
    }
    if (entry.children) {
      result.push(...flattenFiles(entry.children));
    }
  }
  return result;
}

export function buildExtensions(params: BuildExtensionsParams) {
  const { filePath, entriesRef, handleFileSelectRef, docHeaderHandle } = params;

  return [
    vimCompartment.of([]),
    markdown({
      base: markdownLanguage,
      codeLanguages: [...dbSqlLanguages, ...cmLanguages],
    }),
    syntaxHighlighting(markdownHighlightStyle),
    bracketMatching(),
    closeBrackets(),
    search({ top: false }),
    highlightSelectionMatches(),
    history(),
    // Doc-line ArrowUp/Down — bails out when vim owns motion (normal /
    // visual). In insert mode and when vim is off, replaces the
    // pixel-based teleport through tall DB block widgets.
    docLineNavKeymap,
    keymap.of([
      // Explicit Ctrl-Space for autocomplete — avoids relying on the
      // Mac default (Alt-`) so the popup fires on every platform.
      { key: "Ctrl-Space", run: startCompletion },
      ...completionKeymap,
      ...closeBracketsKeymap,
      ...defaultKeymap,
      ...historyKeymap,
      ...searchKeymap,
      indentWithTab,
    ]),
    moveBlocksKeymap(),
    hybridRendering(),
    // After hybridRendering so conflict line backgrounds + the inline
    // accept toolbar layer over the live-preview styling (V10 manual
    // -test follow-up — a conflicted .md must read as conflicted).
    mergeConflict(),
    ...(docHeaderHandle ? [docHeaderHandle.extension] : []),
    // Block-type CM6 extensions — iterates `block-registry.ts`. Order
    // is observable (extension priority); registry preserves the
    // pre-A5 DB-before-HTTP sequence.
    ...blockRegistry.map((m) => m.createExtension()),
    tables(),
    slashCommands(),
    wikilinks({
      getFiles: () => flattenFiles(entriesRef.current),
      onNavigate: (target: string) => {
        const files = flattenFiles(entriesRef.current);
        const match = files.find(
          (f) =>
            f.path === target || f.name === target || f.name === `${target}.md`,
        );
        if (match) handleFileSelectRef.current(match.path);
      },
    }),
    autocompletion({
      override: [
        slashCompletionSource,
        createWikilinkCompletion(() => flattenFiles(entriesRef.current)),
        // Block-type completion sources — each module contributes 1+
        // sources (DB returns 2: {{ref}} + schema-aware SQL; HTTP
        // returns 1: {{ref}}). Activates only inside that type's
        // fenced body.
        ...blockRegistry.flatMap((m) => m.completionSources(() => filePath)),
        // Language server refs/env completion. `override` replaces ALL
        // sources, so the server source must be listed here explicitly;
        // it returns null in editors without the LSP plugin.
        serverCompletionSource,
      ],
      icons: false,
      addToOptions: [slashIconOption],
    }),
    // `{{ref}}` visual highlight; hover and completion are served by
    // the language server (cm-lsp).
    ...referenceHighlight,
    refClickExtension,
    editorTheme,
    EditorView.lineWrapping,
  ];
}
