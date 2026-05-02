// coverage:exclude file
// Epic 30a Story 01 (BlockRegistry) splits this monolith and retires
// the exclusion. Until then, the component composes too many CM6
// extensions (markdown, vim, slash commands, db/http blocks, hybrid
// rendering, wikilinks) to instantiate cleanly inside vitest jsdom —
// the integration tests live in `*.browser.test.tsx` files. See
// `docs-llm/jaum-audit/022-markdown-editor-coverage-exclude.md`.
import { useRef, useEffect, useMemo, useCallback, useState } from "react";
import { Box } from "@chakra-ui/react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView, keymap } from "@codemirror/view";
import { Compartment, EditorSelection, Prec } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages as cmLanguages } from "@codemirror/language-data";
import { LanguageDescription } from "@codemirror/language";

// Register db / db-postgres / db-mysql / db-sqlite as SQL so markdown's
// nested-code syntax highlighter colorizes the body of db fenced blocks.
const dbSqlLanguages: LanguageDescription[] = [
  "db",
  "db-postgres",
  "db-mysql",
  "db-sqlite",
].map((alias) =>
  LanguageDescription.of({
    name: alias,
    alias: [alias],
    async load() {
      const { sql } = await import("@codemirror/lang-sql");
      return sql();
    },
  }),
);
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  syntaxHighlighting,
  HighlightStyle,
  bracketMatching,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";
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
import {
  vim,
  Vim,
  getCM,
  type CodeMirrorV,
  type MotionArgs,
  type Pos,
  type vimState,
} from "@replit/codemirror-vim";
import { hybridRendering } from "@/lib/codemirror/cm-hybrid-rendering";
import {
  slashCommands,
  slashCompletionSource,
  slashIconOption,
} from "@/lib/codemirror/cm-slash-commands";
import { createEditorBlockWidgets } from "@/lib/codemirror/cm-block-widgets";
import { editorTheme } from "@/components/editor/editor-theme";
import {
  createDbBlockExtension,
  createDbBlockCompletionSource,
  createDbSchemaCompletionSource,
} from "@/lib/codemirror/cm-db-block";
import {
  createHttpBlockExtension,
  createHttpBlockCompletionSource,
} from "@/lib/codemirror/cm-http-block";
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
import { useEnvironmentStore } from "@/stores/environment";
import { BlockContextProvider } from "@/components/blocks/BlockContext";
import { DbWidgetPortals } from "./DbWidgetPortals";
import { HttpWidgetPortals } from "./HttpWidgetPortals";
import {
  registerActiveEditor,
  unregisterActiveEditor,
} from "@/lib/codemirror/active-editor";
import { useWorkspaceStore } from "@/stores/workspace";
import type { FileEntry } from "@/lib/tauri/commands";
import { listen } from "@tauri-apps/api/event";

function flattenFiles(entries: FileEntry[]): { name: string; path: string }[] {
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

interface MarkdownEditorProps {
  content: string;
  onChange: (markdown: string) => void;
  filePath: string;
  vimEnabled?: boolean;
  onNavigateFile?: (filePath: string) => void;
}

// Compartment for toggling vim mode without recreating the editor
const vimCompartment = new Compartment();

// Vim-aware guard: bail out when vim is active in a non-insert mode so vim
// keeps ownership of h/j/k/l / arrow motion / visual selection. In insert
// mode and when vim is off, we take over ArrowUp/Down to navigate by doc
// line (see rationale below).
function vimOwnsMotion(view: EditorView): boolean {
  const cm = getCM(view);
  const vimState = cm?.state.vim;
  if (!vimState) return false;
  return !vimState.insertMode;
}

// Doc-line ArrowUp/Down. CM6's default cursorLineUp/Down is pixel-based —
// it teleports to "Ln 1, Col 1" when there's a tall block widget (like
// DbClosePanelWidget) in between because moveVertically can't find a text
// line at the target y. This keymap walks by document lines instead and
// only fires outside of vim normal/visual mode.
const docLineNavKeymap = Prec.high(
  keymap.of([
    {
      key: "ArrowUp",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const sel = view.state.selection.main;
        if (!sel.empty) return false;
        const doc = view.state.doc;
        const line = doc.lineAt(sel.head);
        if (line.number === 1) return false;
        const prev = doc.line(line.number - 1);
        const col = sel.head - line.from;
        const target = Math.min(prev.from + col, prev.to);
        view.dispatch({
          selection: EditorSelection.cursor(target),
          scrollIntoView: true,
        });
        return true;
      },
    },
    {
      key: "ArrowDown",
      run: (view) => {
        if (vimOwnsMotion(view)) return false;
        const sel = view.state.selection.main;
        if (!sel.empty) return false;
        const doc = view.state.doc;
        const line = doc.lineAt(sel.head);
        if (line.number === doc.lines) return false;
        const next = doc.line(line.number + 1);
        const col = sel.head - line.from;
        const target = Math.min(next.from + col, next.to);
        view.dispatch({
          selection: EditorSelection.cursor(target),
          scrollIntoView: true,
        });
        return true;
      },
    },
  ]),
);

// Replace vim's built-in `moveByLines` motion with a doc-line variant.
// The upstream implementation uses `cm.findPosV(..., 'line', ...)` which
// the CM5→CM6 bridge routes through `moveVertically` — pixel-based motion
// that teleports through tall block widgets (DbClosePanelWidget, result
// panels, etc.). Because the vim dispatcher looks motions up by name, a
// single defineMotion call here transparently fixes j, k, <Up>, <Down>,
// +, -, _ in normal and visual mode.
//
// Why: keeps normal/visual vim state intact (HPos stickiness, visual
// selection extension) while replacing only the vertical-motion compute.
let vimMotionsInstalled = false;
function installDocLineVimMotions() {
  if (vimMotionsInstalled) return;
  vimMotionsInstalled = true;
  const docMoveByLines = function (
    cm: CodeMirrorV,
    head: Pos,
    motionArgs: MotionArgs,
    vimState: vimState,
  ): Pos {
    let endCh = head.ch;
    // HPos stickiness: for j/k/j/k chains we can detect ourselves. Any
    // other motion (h/l, word, gj, etc.) resets the goal column — a
    // minor regression vs. vanilla vim that we accept in exchange for
    // not teleporting through widgets.
    if (vimState.lastMotion === docMoveByLines) {
      endCh = vimState.lastHPos ?? head.ch;
    } else {
      vimState.lastHPos = endCh;
    }
    const repeat = motionArgs.repeat + (motionArgs.repeatOffset || 0);
    const first = cm.firstLine();
    const last = cm.lastLine();
    let line = motionArgs.forward ? head.line + repeat : head.line - repeat;
    if (line < first) line = first;
    if (line > last) line = last;
    if (motionArgs.toFirstChar) {
      const text: string = cm.getLine(line) ?? "";
      const match = /^\s*/.exec(text);
      endCh = match ? match[0].length : 0;
      vimState.lastHPos = endCh;
    }
    const lineText: string = cm.getLine(line) ?? "";
    if (endCh > lineText.length) endCh = lineText.length;
    try {
      vimState.lastHSPos = cm.charCoords({ line, ch: endCh }, "div").left;
    } catch {
      // charCoords can throw before the view is laid out; HSPos is only
      // used by gj/gk, which we don't override. Safe to ignore.
    }
    return { line, ch: endCh };
  };
  Vim.defineMotion("moveByLines", docMoveByLines);
}
installDocLineVimMotions();

// Custom highlight style — Chakra-token driven so the editor follows the app theme.
const markdownHighlightStyle = HighlightStyle.define([
  // Markdown inline formatting
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  {
    tag: tags.link,
    color: "var(--chakra-colors-blue-400)",
    textDecoration: "none",
  },
  { tag: tags.url, color: "var(--chakra-colors-blue-400)" },
  {
    tag: tags.monospace,
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "0.85em",
  },
  { tag: tags.processingInstruction, color: "var(--chakra-colors-fg-subtle)" },
  { tag: tags.meta, color: "var(--chakra-colors-fg-subtle)" },

  // Code syntax highlighting (for nested languages via codeLanguages)
  { tag: tags.keyword, color: "var(--chakra-colors-purple-500)" },
  {
    tag: [tags.atom, tags.bool, tags.null],
    color: "var(--chakra-colors-orange-500)",
  },
  {
    tag: [tags.number, tags.integer, tags.float],
    color: "var(--chakra-colors-orange-500)",
  },
  {
    tag: [tags.string, tags.special(tags.string)],
    color: "var(--chakra-colors-green-500)",
  },
  { tag: [tags.regexp, tags.escape], color: "var(--chakra-colors-green-400)" },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "var(--chakra-colors-fg-muted)",
    fontStyle: "italic",
  },
  { tag: [tags.variableName, tags.name], color: "var(--chakra-colors-fg)" },
  {
    tag: [tags.propertyName, tags.attributeName],
    color: "var(--chakra-colors-cyan-400)",
  },
  {
    tag: [tags.typeName, tags.className, tags.namespace],
    color: "var(--chakra-colors-yellow-400)",
  },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
    color: "var(--chakra-colors-blue-400)",
  },
  {
    tag: [
      tags.definition(tags.variableName),
      tags.definition(tags.propertyName),
    ],
    color: "var(--chakra-colors-blue-300)",
  },
  { tag: tags.operator, color: "var(--chakra-colors-pink-400)" },
  {
    tag: [
      tags.punctuation,
      tags.bracket,
      tags.squareBracket,
      tags.paren,
      tags.brace,
    ],
    color: "var(--chakra-colors-fg-subtle)",
  },
  { tag: tags.tagName, color: "var(--chakra-colors-red-400)" },
  {
    tag: tags.self,
    color: "var(--chakra-colors-purple-400)",
    fontStyle: "italic",
  },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.invalid, color: "var(--chakra-colors-red-500)" },
]);


// Static CSS for the container — @uiw/react-codemirror wraps the editor
// in its own div, which needs explicit height for .cm-scroller to work
const containerCss = {
  "& > div": { height: "100%" },
  "& .cm-editor": { height: "100%" },
  "& .cm-editor.cm-focused": { outline: "none" },
};

export function MarkdownEditor({
  content,
  onChange,
  filePath,
  vimEnabled = false,
  onNavigateFile,
}: MarkdownEditorProps) {
  const cmRef = useRef<ReactCodeMirrorRef>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [editorReady, setEditorReady] = useState(false);

  // Read workspace state imperatively (non-reactive)
  const entriesRef = useRef<FileEntry[]>(useWorkspaceStore.getState().entries);
  useEffect(() => {
    return useWorkspaceStore.subscribe((state) => {
      entriesRef.current = state.entries;
    });
  }, []);
  const handleFileSelectRef = useRef(onNavigateFile ?? (() => {}));
  handleFileSelectRef.current = onNavigateFile ?? (() => {});

  // Stable extensions (vim toggled via compartment, not via extensions prop)
  const extensions = useMemo(
    () => [
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
      // Doc-line ArrowUp/Down. The handlers bail out when vim is in a
      // non-insert mode so vim keeps ownership of normal/visual motion; in
      // insert mode and when vim is off, they run and avoid the pixel-based
      // teleport through tall DB block widgets.
      docLineNavKeymap,
      keymap.of([
        // Explicit Ctrl-Space for autocomplete — avoids relying on the Mac
        // default (Alt-`) so the popup fires on every platform.
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
      createDbBlockExtension(),
      createHttpBlockExtension(),
      createEditorBlockWidgets(),
      tables(),
      slashCommands(),
      wikilinks({
        getFiles: () => flattenFiles(entriesRef.current),
        onNavigate: (target: string) => {
          const files = flattenFiles(entriesRef.current);
          const match = files.find(
            (f) =>
              f.path === target ||
              f.name === target ||
              f.name === `${target}.md`,
          );
          if (match) handleFileSelectRef.current(match.path);
        },
      }),
      autocompletion({
        override: [
          slashCompletionSource,
          createWikilinkCompletion(() => flattenFiles(entriesRef.current)),
          // DB block {{ref}} autocomplete — activates only when the cursor
          // sits inside a db-* fenced body.
          createDbBlockCompletionSource(() => filePath),
          // Schema-aware SQL autocomplete (tables / columns) — same gating;
          // reads from the shared SchemaCache store.
          createDbSchemaCompletionSource(),
          // HTTP block {{ref}} autocomplete — activates only inside an http
          // fenced body.
          createHttpBlockCompletionSource(() => filePath),
        ],
        icons: false,
        addToOptions: [slashIconOption],
      }),
      // `{{ref}}` visual highlight + hover tooltip. The tooltip resolves
      // the reference against blocks above the enclosing fence (DB or
      // http/e2e) and shows the cached value — or the resolution error.
      // CM6 tooltips default to `position: fixed`, so the outer Box's
      // `overflow: hidden` does NOT clip them; we don't need a custom
      // `tooltips({ parent })` here (and setting one breaks baseTheme
      // styling, which is scoped to `.cm-editor`).
      ...referenceHighlight,
      createMarkdownReferenceTooltip(
        () => filePath,
        () => useEnvironmentStore.getState().getActiveVariables(),
      ),
      editorTheme,
      EditorView.lineWrapping,
    ],
    [],
  );

  // Editor created callback
  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      viewRef.current = view;
      setEditorReady(true);
      if (vimEnabled) {
        view.dispatch({
          effects: vimCompartment.reconfigure(vim()),
        });
      }
      // Register as the active editor so out-of-editor components (schema
      // panel, etc.) can dispatch edits into the currently-focused pane.
      // Focus wins here: the last-focused editor is authoritative. The
      // focus/blur listeners on the DOM keep this accurate across panes.
      const onFocus = () => registerActiveEditor(view);
      const onBlur = () => unregisterActiveEditor(view);
      view.dom.addEventListener("focusin", onFocus);
      view.dom.addEventListener("focusout", onBlur);
      // Seed as active immediately — queueMicrotask below will focus it, but
      // the first `focusin` fires before we've attached the listener above
      // when there's only one pane, so we're-registering here avoids losing
      // the first registration to the race.
      registerActiveEditor(view);
      queueMicrotask(() => view.focus());
    },
    [vimEnabled],
  );

  // Vim toggle after initial creation. The doc-line ArrowUp/Down keymap
  // no longer moves with this toggle — its handlers inspect the live vim
  // state and bail when vim owns motion.
  useEffect(() => {
    viewRef.current?.dispatch({
      effects: vimCompartment.reconfigure(vimEnabled ? vim() : []),
    });
  }, [vimEnabled]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      const view = viewRef.current;
      if (view) unregisterActiveEditor(view);
      viewRef.current = null;
      setEditorReady(false);
    };
  }, [filePath]);

  // Listen for external file reloads
  useEffect(() => {
    const unlisten = listen<{ path: string; markdown: string }>(
      "file-reloaded",
      (event) => {
        if (event.payload.path !== filePath) return;
        const view = viewRef.current;
        if (!view) return;

        const currentContent = view.state.doc.toString();
        if (currentContent === event.payload.markdown) return;

        view.dispatch({
          changes: {
            from: 0,
            to: view.state.doc.length,
            insert: event.payload.markdown,
          },
        });
      },
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [filePath]);

  return (
    <BlockContextProvider value={{ filePath }}>
      <Box position="relative" h="100%" overflow="hidden" css={containerCss}>
        <CodeMirror
          key={filePath}
          ref={cmRef}
          value={content}
          onChange={onChange}
          extensions={extensions}
          basicSetup={false}
          theme="none"
          height="100%"
          onCreateEditor={handleCreateEditor}
        />
        {editorReady && viewRef.current && (
          <>
            <DbWidgetPortals view={viewRef.current} filePath={filePath} />
            <HttpWidgetPortals view={viewRef.current} filePath={filePath} />
          </>
        )}
      </Box>
    </BlockContextProvider>
  );
}
