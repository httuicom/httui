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
import {
  referenceHighlight,
  createMarkdownReferenceTooltip,
} from "@/lib/blocks/cm-references";
import { refClickExtension } from "@/lib/blocks/cm-ref-popover";

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
  getActiveVariables: () =>
    | Promise<Record<string, string>>
    | Record<string, string>;
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
  const {
    filePath,
    entriesRef,
    handleFileSelectRef,
    docHeaderHandle,
    getActiveVariables,
  } = params;

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
    // accept toolbar layer over the live-preview styling.
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
        createDbBlockCompletionSource(() => filePath),
        createDbSchemaCompletionSource(),
        createHttpBlockCompletionSource(() => filePath),
      ],
      icons: false,
      addToOptions: [slashIconOption],
    }),
    // `{{ref}}` visual highlight + hover tooltip. CM6 tooltips default to
    // `position: fixed`, so the outer Box's `overflow: hidden` does NOT clip
    // them — no custom `tooltips({ parent })` needed (setting one breaks
    // baseTheme styling scoped to `.cm-editor`).
    ...referenceHighlight,
    refClickExtension,
    createMarkdownReferenceTooltip(() => filePath, getActiveVariables),
    editorTheme,
    EditorView.lineWrapping,
  ];
}
