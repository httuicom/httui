// V6 / cenário 9 polish — single-line CodeMirror editor used as the
// value input inside the pre-flight check popover. Mirrors the inline
// forms in HTTP/DB blocks: the user gets the same autocomplete UX
// they're used to (CM6 native dropdown + Tab/Enter accept) instead of
// a custom <Input + suggestions list>.
//
// Per kind, the completion source pulls from a different data source:
//   - connection → vault connection names (listConnections Tauri)
//   - env_var    → active environment's variable keys (store)
//   - branch / keychain / file_exists / command → no completion
//
// Single-line guard: a transactionFilter strips any \n / \r so the
// editor stays at 1 visual line; Enter is repurposed as commit.

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
  keychain: "keychain key",
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

  // Fetch suggestions on kind change.
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

  // Build extensions once. Suggestions are read fresh from a ref via a
  // closure so the completion source always sees the latest list
  // without rebuilding the editor on every fetch.
  const suggestionsRef = useRef<string[]>([]);
  suggestionsRef.current = suggestions;

  const extensions = useMemo(() => {
    const completionSource = (
      ctx: CompletionContext,
    ): CompletionResult | null => {
      const list = suggestionsRef.current;
      if (list.length === 0) return null;
      const word = ctx.matchBefore(/[\w./-]*/);
      const from = word ? word.from : ctx.pos;
      // Open the dropdown even on empty input (explicit Ctrl-Space) but
      // not on every cursor move when there's nothing to match — `null`
      // returns let CM6 close the menu.
      if (!word && !ctx.explicit) return null;
      const options: Completion[] = list.map((label) => ({
        label,
        type: "variable",
      }));
      return { from, to: ctx.pos, options, filter: true };
    };

    return [
      EditorView.lineWrapping,
      // Single-line: discard any transaction that introduces a newline.
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
            onCommit();
            return true;
          },
        },
        {
          key: "Escape",
          run: () => {
            onCancel();
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
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          drawSelection: true,
          dropCursor: true,
          allowMultipleSelections: false,
          indentOnInput: false,
          syntaxHighlighting: false,
          bracketMatching: false,
          closeBrackets: false,
          autocompletion: false,
          rectangularSelection: false,
          crosshairCursor: false,
          highlightSelectionMatches: false,
          searchKeymap: false,
        }}
        theme="none"
        height="auto"
        placeholder={PLACEHOLDER_BY_KIND[kind] ?? ""}
        autoFocus
      />
    </Box>
  );
}
