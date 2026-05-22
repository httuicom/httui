// coverage:exclude file
// Single-line CM6 editor for the pre-flight check popover value input.
// A transactionFilter strips \n/\r to enforce single-line; Enter commits.
// Coverage excluded: CM6 contenteditable needs a real DOM (browser tests cover it).

import { useEffect, useMemo, useRef, useState } from "react";
import { Box } from "@chakra-ui/react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import {
  autocompletion,
  acceptCompletion,
  completionStatus,
  startCompletion,
  type CompletionContext,
  type CompletionResult,
  type Completion,
} from "@codemirror/autocomplete";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";

import type { PreflightCheckKind } from "@/lib/blocks/preflight-checks";

const PLACEHOLDER_BY_KIND: Record<PreflightCheckKind, string> = {
  connection: "connection name (e.g., payments-db)",
  env_var: "ENV VAR NAME",
  branch: "branch name (e.g., main)",
  file_exists: "./path/to/file",
  command: "binary name (e.g., psql)",
};

export type PreflightCompletionProvider = (
  kind: PreflightCheckKind,
) => Promise<string[]>;

export interface PreflightValueEditorProps {
  kind: PreflightCheckKind;
  value: string;
  onChange: (next: string) => void;
  /** Fired on Enter when the autocomplete menu is closed. */
  onCommit: () => void;
  /** Fired on Esc. */
  onCancel: () => void;
  /** Async data fetcher for autocomplete. Called once on mount + on
   *  kind change; results cached in state. */
  getSuggestions?: PreflightCompletionProvider;
}

const editorTheme = EditorView.theme({
  "&": {
    fontSize: "13px",
    fontFamily: "var(--chakra-fonts-mono)",
  },
  ".cm-content": {
    padding: "6px 8px",
    color: "var(--chakra-colors-fg)",
  },
  ".cm-line": {
    padding: 0,
  },
  "&.cm-editor.cm-focused": {
    outline: "none",
  },
});

const containerSx = {
  border: "1px solid",
  borderColor: "border",
  borderRadius: "md",
  bg: "bg",
  "&:focus-within": {
    borderColor: "brand.500",
  },
};

export function PreflightValueEditor({
  kind,
  value,
  onChange,
  onCommit,
  onCancel,
  getSuggestions,
}: PreflightValueEditorProps) {
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const cmRef = useRef<ReactCodeMirrorRef>(null);

  // Focus via ref rather than `autoFocus` — the autoFocus prop swallowed
  // @uiw/react-codemirror's onChange listener in testing.
  useEffect(() => {
    cmRef.current?.view?.focus();
  }, []);

  useEffect(() => {
    if (!getSuggestions) {
      setSuggestions([]);
      return;
    }
    let cancelled = false;
    getSuggestions(kind)
      .then((list) => {
        if (!cancelled) setSuggestions(list);
      })
      .catch(() => {
        if (!cancelled) setSuggestions([]);
      });
    return () => {
      cancelled = true;
    };
  }, [kind, getSuggestions]);

  // Build extensions once; read suggestions + callbacks from refs so
  // closures stay fresh without re-mounting the editor on each render.
  const suggestionsRef = useRef<string[]>([]);
  suggestionsRef.current = suggestions;
  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;
  const onCancelRef = useRef(onCancel);
  onCancelRef.current = onCancel;

  const extensions = useMemo(() => {
    const completionSource = (
      ctx: CompletionContext,
    ): CompletionResult | null => {
      const list = suggestionsRef.current;
      if (list.length === 0) return null;
      const word = ctx.matchBefore(/[\w./-]*/);
      const from = word ? word.from : ctx.pos;
      // Don't open on every cursor move — only on explicit Ctrl-Space or typed input.
      if (!word && !ctx.explicit) return null;
      const options: Completion[] = list.map((label) => ({
        label,
        type: "variable",
      }));
      return { from, to: ctx.pos, options, filter: true };
    };

    return [
      EditorView.lineWrapping,
      EditorState.transactionFilter.of((tr) => {
        if (!tr.docChanged) return tr;
        const next = tr.newDoc.toString();
        if (/[\r\n]/.test(next)) return [];
        return tr;
      }),
      autocompletion({
        override: [completionSource],
        activateOnTyping: true,
      }),
      keymap.of([
        { key: "Ctrl-Space", run: startCompletion },
        { key: "Tab", run: acceptCompletion },
        {
          key: "Enter",
          run: (view) => {
            // If the autocomplete popup is open, accept; else commit.
            if (completionStatus(view.state) === "active") {
              return acceptCompletion(view);
            }
            onCommitRef.current();
            return true;
          },
        },
        {
          key: "Escape",
          run: () => {
            onCancelRef.current();
            return true;
          },
        },
      ]),
      editorTheme,
    ];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Box
      data-testid="preflight-check-popover-value-editor"
      data-kind={kind}
      css={containerSx}
    >
      <CodeMirror
        ref={cmRef}
        value={value}
        onChange={onChange}
        extensions={extensions}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          autocompletion: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          indentOnInput: false,
          bracketMatching: false,
          closeBrackets: false,
          history: true,
        }}
        height="auto"
        placeholder={PLACEHOLDER_BY_KIND[kind] ?? ""}
        style={{ fontFamily: "var(--chakra-fonts-mono)" }}
      />
    </Box>
  );
}
