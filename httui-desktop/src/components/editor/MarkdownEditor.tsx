// coverage:exclude file
// Composition shell for the markdown editor. Lifecycle (mount, vim
// toggle, file-reloaded listener) lives here; the heavy CM6 stack is
// built by `markdown-extensions.ts` and the doc-line vim motions live
// in `markdown-vim-motions.ts`. Each sub-module has unit tests.
//
// The exclusion stays because the React shell wires together CM6,
// portals, Tauri events and Zustand subscriptions — it can only be
// exercised through the integration tests in `*.browser.test.tsx`.
import { useRef, useEffect, useMemo, useCallback, useState } from "react";
import { Box } from "@chakra-ui/react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { vim } from "@replit/codemirror-vim";

import { createDocHeaderExtension } from "@/lib/codemirror/cm-doc-header";
import { useEnvironmentStore } from "@/stores/environment";
import { BlockContextProvider } from "@/components/blocks/BlockContext";
import {
  registerActiveEditor,
  unregisterActiveEditor,
} from "@/lib/codemirror/active-editor";
import { useWorkspaceStore } from "@/stores/workspace";
import type { FileEntry } from "@/lib/tauri/commands";
import { listen } from "@tauri-apps/api/event";

import { DbWidgetPortals } from "./DbWidgetPortals";
import { HttpWidgetPortals } from "./HttpWidgetPortals";
import {
  DocHeaderWidgetPortal,
  type InlineDocHeader,
} from "./DocHeaderWidgetPortal";
import {
  vimCompartment,
  installDocLineVimMotions,
} from "./markdown-vim-motions";
import { containerCss } from "./markdown-highlight-style";
import { buildExtensions } from "./markdown-extensions";

installDocLineVimMotions();

interface MarkdownEditorProps {
  content: string;
  onChange: (markdown: string) => void;
  filePath: string;
  vimEnabled?: boolean;
  onNavigateFile?: (filePath: string) => void;
  /** When provided, the DocHeader CM6 widget mounts at the top of the
   *  document and the React shell is rendered inside it via portal. The
   *  consumer (`DocHeaderedEditor`) feeds in the same data the
   *  standalone shell used to receive. */
  inlineHeader?: InlineDocHeader;
}

export function MarkdownEditor({
  content,
  onChange,
  filePath,
  vimEnabled = false,
  onNavigateFile,
  inlineHeader,
}: MarkdownEditorProps) {
  const cmRef = useRef<ReactCodeMirrorRef>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [editorReady, setEditorReady] = useState(false);

  // The DocHeader extension is created lazily once per editor mount. It
  // returns a stable instanceId so the React portal can find its slot
  // in the global registry. Stays null when `inlineHeader` is not
  // provided — the editor renders without a header (preserves callers
  // that don't need the DocHeader, e.g. standalone test mounts).
  const hasInlineHeader = inlineHeader != null;
  const docHeaderHandle = useMemo(
    () => (hasInlineHeader ? createDocHeaderExtension() : null),
    [hasInlineHeader],
  );

  // Read workspace state imperatively (non-reactive)
  const entriesRef = useRef<FileEntry[]>(useWorkspaceStore.getState().entries);
  useEffect(() => {
    return useWorkspaceStore.subscribe((state) => {
      entriesRef.current = state.entries;
    });
  }, []);
  const handleFileSelectRef = useRef(onNavigateFile ?? (() => {}));
  handleFileSelectRef.current = onNavigateFile ?? (() => {});

  // Stable extensions (vim toggled via compartment, not via extensions
  // prop). `filePath` and `docHeaderHandle` are captured from the first
  // render; the file path also keys the outer <CodeMirror key={filePath}>
  // so a new file mount produces a fresh closure naturally.
  const extensions = useMemo(
    () =>
      buildExtensions({
        filePath,
        entriesRef,
        handleFileSelectRef,
        docHeaderHandle,
        getActiveVariables: () =>
          useEnvironmentStore.getState().getActiveVariables(),
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      viewRef.current = view;
      setEditorReady(true);
      if (vimEnabled) {
        view.dispatch({
          effects: vimCompartment.reconfigure(vim()),
        });
      }
      // Register as the active editor so out-of-editor components
      // (schema panel, etc.) can dispatch edits into the currently-
      // focused pane. Focus wins here: the last-focused editor is
      // authoritative.
      const onFocus = () => registerActiveEditor(view);
      const onBlur = () => unregisterActiveEditor(view);
      view.dom.addEventListener("focusin", onFocus);
      view.dom.addEventListener("focusout", onBlur);
      // Seed as active immediately — queueMicrotask below will focus
      // it, but the first `focusin` fires before we've attached the
      // listener above when there's only one pane, so re-registering
      // here avoids losing the first registration to the race.
      registerActiveEditor(view);
      queueMicrotask(() => view.focus());
    },
    [vimEnabled],
  );

  // Vim toggle after initial creation. The doc-line ArrowUp/Down
  // keymap inspects the live vim state and bails when vim owns motion.
  useEffect(() => {
    viewRef.current?.dispatch({
      effects: vimCompartment.reconfigure(vimEnabled ? vim() : []),
    });
  }, [vimEnabled]);

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
            {docHeaderHandle && inlineHeader && (
              <DocHeaderWidgetPortal
                instanceId={docHeaderHandle.instanceId}
                inlineHeader={inlineHeader}
              />
            )}
          </>
        )}
      </Box>
    </BlockContextProvider>
  );
}
