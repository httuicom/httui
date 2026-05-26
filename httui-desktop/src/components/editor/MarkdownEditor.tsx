// coverage:exclude file
// Composition shell for the markdown editor. Lifecycle (mount, vim
// toggle, file-reloaded listener) lives here; the heavy CM6 stack is
// built by `markdown-extensions.ts` and the doc-line vim motions live
// in `markdown-vim-motions.ts`. Each sub-module has unit tests.
//
// The exclusion stays because the React shell wires together CM6,
// portals, Tauri events and Zustand subscriptions — it can only be
// exercised through the integration tests in `*.browser.test.tsx`.
import {
  Fragment,
  useRef,
  useEffect,
  useMemo,
  useCallback,
  useState,
} from "react";
import { Box } from "@chakra-ui/react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { vim } from "@replit/codemirror-vim";

import { createDocHeaderExtension } from "@/lib/codemirror/cm-doc-header";
import { useEnvironmentStore } from "@/stores/environment";
import { BlockContextProvider } from "@/components/blocks/BlockContext";
import {
  activeEditorTracker,
  registerActiveEditor,
  unregisterActiveEditor,
} from "@/lib/codemirror/active-editor";
import { useWorkspaceStore } from "@/stores/workspace";
import type { FileEntry } from "@/lib/tauri/commands";
import { listen } from "@tauri-apps/api/event";

import { blockPortals } from "@/lib/blocks/block-portal-registry";
import { RefPopoverHost } from "./RefPopoverHost";
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
    () => [
      ...buildExtensions({
        filePath,
        entriesRef,
        handleFileSelectRef,
        docHeaderHandle,
        getActiveVariables: () =>
          useEnvironmentStore.getState().getActiveVariables(),
      }),
      // Focus/destroy-driven active-editor registry. CM owns the
      // listener lifecycle (auto-removed on view destroy), so there is
      // no manual addEventListener to leak.
      activeEditorTracker(),
    ],
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
      // Focus/blur are tracked by the `activeEditorTracker()` extension
      // (CM-owned listener lifecycle — no manual addEventListener to
      // leak). Seed as active immediately so this editor is
      // authoritative before the first focus event: queueMicrotask
      // focuses it, but with a single pane the first `focusin` can fire
      // before the extension's handler runs.
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
            {/* Block-type portal mounts — iterates block-portal-
              registry, so a new block type adds one entry there and
              this JSX never changes (audit 03 #2 OCP). The outer
              `editorReady && viewRef.current` guard already proves
              `viewRef.current` non-null; the `!` is just to keep TS
              happy across the .map callback boundary. */}
            {blockPortals.map((p) => (
              <Fragment key={p.id}>
                {p.renderPortal(viewRef.current!, filePath)}
              </Fragment>
            ))}
            <RefPopoverHost />
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
