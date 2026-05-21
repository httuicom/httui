// Inline CodeMirror editors + their static themes for the HTTP block
// form mode.
//
// Extracted verbatim from HttpFencedPanel.tsx (A1 / audit 03 §1 seam
// #4). Seam 3 (form tables) and the orchestrator consume
// CommitOnBlurInput / HttpInlineCM / HttpBodyCM, so this is extracted
// before seam 3 to avoid a transient panel re-export. `looksLikeJson
// Body` + `BodyCMProps` were already public (HttpBodyCM.test.tsx). The
// three `EditorView.theme` objects stay module-internal.

import { memo, useEffect, useMemo, useRef, useState } from "react";
import { Box, Input } from "@chakra-ui/react";
import { Compartment, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";

import { referenceHighlight } from "@/lib/blocks/cm-references";
import {
  createReferenceAutocomplete,
  type EnvKeyInfo,
} from "@/lib/blocks/cm-autocomplete";
import type { BlockContext } from "@/lib/blocks/references";

// Static themes — extracted so Emotion doesn't recreate them per render.
const cmTransparentBg = EditorView.theme({
  "&": { backgroundColor: "transparent !important" },
  "& .cm-gutters": {
    backgroundColor: "transparent !important",
    border: "none",
  },
  "& .cm-activeLineGutter, & .cm-activeLine": {
    backgroundColor: "transparent !important",
  },
});

const cmInlineTheme = EditorView.theme({
  "&": { backgroundColor: "transparent !important", fontSize: "12px" },
  "&.cm-focused": { outline: "none" },
  "& .cm-gutters": { display: "none" },
  "& .cm-activeLineGutter, & .cm-activeLine": {
    backgroundColor: "transparent !important",
  },
  "& .cm-scroller": {
    overflow: "auto hidden",
    scrollbarWidth: "none",
    lineHeight: "26px",
  },
  "& .cm-scroller::-webkit-scrollbar": { display: "none" },
  "& .cm-content": { padding: "0 8px", minHeight: "auto" },
  "& .cm-line": { padding: 0 },
  "& .cm-placeholder": {
    color: "var(--chakra-colors-fg-muted)",
    opacity: 0.5,
  },
  "& .cm-cursor": { borderLeftColor: "var(--chakra-colors-fg)" },
});

const cmBodyTheme = EditorView.theme({
  "&": { backgroundColor: "transparent !important", fontSize: "12px" },
  "&.cm-focused": { outline: "none" },
  "& .cm-gutters": { display: "none" },
  "& .cm-content": {
    fontFamily: "var(--chakra-fonts-mono)",
    padding: "8px",
    minHeight: "120px",
  },
  "& .cm-activeLineGutter, & .cm-activeLine": {
    backgroundColor: "transparent !important",
  },
});

/**
 * Light-weight HTML input that mirrors the commit-on-blur contract of
 * `HttpInlineCM` but without the CodeMirror runtime — used in tabs that
 * don't need `{{ref}}` highlighting (multipart `name` / file value), where
 * a CM re-render on every committed keystroke caused visible flashing.
 */
export const CommitOnBlurInput = memo(function CommitOnBlurInput({
  value,
  placeholder,
  onCommit,
  readOnly,
}: {
  value: string;
  placeholder?: string;
  onCommit: (next: string) => void;
  readOnly?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <Input
      size="xs"
      value={draft}
      placeholder={placeholder}
      readOnly={readOnly}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) onCommit(draft);
      }}
      fontFamily="mono"
      fontSize="xs"
    />
  );
});

/**
 * Single-line CodeMirror replacing `<Input>` for the form-mode KV rows.
 * Supports `{{ref}}` highlight + autocomplete. Commits on blur (matches
 * the existing form pattern — see `CommitOnBlurInput`).
 */
export const HttpInlineCM = memo(function HttpInlineCM({
  value,
  placeholder,
  onCommit,
  refsGetters,
}: {
  value: string;
  placeholder?: string;
  onCommit: (next: string) => void;
  refsGetters?: {
    getBlocks: () => BlockContext[];
    getEnvKeys: () => (string | EnvKeyInfo)[];
  };
}) {
  // Controlled-direct pattern (matches the legacy `HttpBlockView.InlineCM`):
  // value flows in, every keystroke flows out via `onCommit`. Without the
  // local-draft + commit-on-blur indirection, react-codemirror's internal
  // diff sees `value === currentDoc` after each commit and does NOT
  // reanimate the editor — that was the source of the visible flash when
  // we used `useState(draft)` + `useEffect`.
  const extensions = useMemo(() => {
    const exts = [cmInlineTheme, cmTransparentBg, ...referenceHighlight];
    if (refsGetters) {
      exts.push(
        createReferenceAutocomplete(
          refsGetters.getBlocks,
          refsGetters.getEnvKeys,
        ),
      );
    }
    return exts;
  }, [refsGetters]);

  return (
    <Box
      flex={1}
      borderWidth="1px"
      borderColor="border.muted"
      borderRadius="sm"
      bg="bg.canvas"
      _focusWithin={{ borderColor: "border.emphasized" }}
      overflow="hidden"
    >
      <CodeMirror
        value={value}
        onChange={onCommit}
        extensions={extensions}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          autocompletion: !!refsGetters,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          indentOnInput: false,
          bracketMatching: false,
          closeBrackets: false,
          history: true,
        }}
        height="auto"
        placeholder={placeholder}
        style={{ fontFamily: "var(--chakra-fonts-mono)" }}
      />
    </Box>
  );
});

/** A body is JSON-highlightable when its first non-space char is { or [. */
export function looksLikeJsonBody(body: string): boolean {
  const t = body.trimStart();
  return t.startsWith("{") || t.startsWith("[");
}

export interface BodyCMProps {
  value: string;
  onCommit: (next: string) => void;
  refsGetters?: {
    getBlocks: () => BlockContext[];
    getEnvKeys: () => (string | EnvKeyInfo)[];
  };
}

/**
 * Multi-line CodeMirror for the body in form mode. Adds JSON highlight
 * when the body looks like JSON, plus `{{ref}}` highlight + autocomplete.
 *
 * JSON highlight lives in a Compartment instead of being baked into the
 * memoized `extensions` array: keying the memo on `value` rebuilt the
 * whole extension set on every commit-on-blur (`onCommit` makes the
 * parent re-emit `value`), reconfiguring the editor and causing the
 * visible flash the draft/commit-on-blur indirection exists to avoid.
 * The extension array is now stable; only the JSON language
 * reconfigures, and only when the body's JSON-ness actually flips.
 */
export const HttpBodyCM = memo(function HttpBodyCM({
  value,
  onCommit,
  refsGetters,
}: BodyCMProps) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  const jsonCompartment = useMemo(() => new Compartment(), []);
  const viewRef = useRef<EditorView | null>(null);

  const extensions = useMemo(() => {
    const exts: Extension[] = [
      cmBodyTheme,
      cmTransparentBg,
      ...referenceHighlight,
      jsonCompartment.of([]),
    ];
    if (refsGetters) {
      exts.push(
        createReferenceAutocomplete(
          refsGetters.getBlocks,
          refsGetters.getEnvKeys,
        ),
      );
    }
    return exts;
  }, [refsGetters, jsonCompartment]);

  // Drive JSON highlight off the live draft, applied through the
  // compartment so the editor is not reconfigured and the dispatch only
  // fires when the JSON-ness flips.
  const isJson = looksLikeJsonBody(draft);
  useEffect(() => {
    viewRef.current?.dispatch({
      effects: jsonCompartment.reconfigure(isJson ? json() : []),
    });
  }, [isJson, jsonCompartment]);

  return (
    <Box
      borderWidth="1px"
      borderColor="border.muted"
      borderRadius="sm"
      bg="bg.canvas"
      overflow="hidden"
    >
      <CodeMirror
        value={draft}
        onChange={(v) => setDraft(v)}
        onBlur={() => {
          if (draft !== value) onCommit(draft);
        }}
        onCreateEditor={(view) => {
          viewRef.current = view;
        }}
        extensions={extensions}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          autocompletion: !!refsGetters,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          indentOnInput: false,
          bracketMatching: true,
          closeBrackets: true,
          history: true,
        }}
        placeholder="Request body (raw)"
        style={{ fontFamily: "var(--chakra-fonts-mono)" }}
      />
    </Box>
  );
});
