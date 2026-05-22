/**
 * React panel for an `http` fenced block. Mounted via `createPortal` into
 * three container divs (toolbar, result, statusbar) registered by the
 * `cm-http-block.tsx` CM6 extension. Settings drawer uses a Chakra Portal
 * (not Dialog — would trap focus away from CM6). Results cached by
 * method+URL+headers+body+env-snapshot; mutations never served from cache.
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  setHttpBlockActions,
  type HttpPortalEntry,
} from "@/lib/codemirror/cm-http-block";
import {
  deriveBodyMode,
  isCompatibleSwitch,
  setContentTypeForMode,
  stringifyHttpFenceInfo,
  stringifyHttpMessageBody,
  type HttpBlockMetadata,
  type HttpBodyMode,
  type HttpMessageParsed,
} from "@/lib/blocks/http-fence";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useBlockSettings } from "./useBlockSettings";
import {
  buildExecutorParams,
  deriveHost,
  httpElapsedOf,
  parseBody,
} from "./http-request-builder";
import { useHttpRefsContext } from "./useHttpRefsContext";
import { useHttpCacheHydrate } from "./useHttpCacheHydrate";
import { useHttpCodegenSnippets } from "./useHttpCodegenSnippets";
import { useHttpDrawerData } from "./useHttpDrawerData";
import { useExecutableBlock } from "@/hooks/useExecutableBlock";
import { HttpBodyView } from "./HttpBodyView";
import { HttpInlineCM } from "./HttpInlineEditors";
import { HttpBodyByMode } from "./HttpFormTables";
import { toaster } from "@/components/ui/toaster";
import {
  cancelBlockExecution,
  executeHttpStreamed,
  type HttpResponseFull,
} from "@/lib/tauri/streamedExecution";
import { resolveAllReferences } from "@/lib/blocks/references";
import { computeHttpCacheHash } from "@/lib/blocks/hash";
import { EditorView } from "@codemirror/view";

import { saveBlockResult } from "@/lib/tauri/commands";
import type { BlockContext } from "@/lib/blocks/references";

// Static themes — extracted so Emotion skips per-render recomputation.
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
const CommitOnBlurInput = memo(function CommitOnBlurInput({
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
const HttpInlineCM = memo(function HttpInlineCM({
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

/**
 * Multi-line CodeMirror for the body in form mode. Adds JSON highlight
 * when the body looks like JSON, plus `{{ref}}` highlight + autocomplete.
 */
const HttpBodyCM = memo(function HttpBodyCM({
  value,
  onCommit,
  refsGetters,
}: {
  value: string;
  onCommit: (next: string) => void;
  refsGetters?: {
    getBlocks: () => BlockContext[];
    getEnvKeys: () => (string | EnvKeyInfo)[];
  };
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  const extensions = useMemo(() => {
    const exts = [cmBodyTheme, cmTransparentBg, ...referenceHighlight];
    const trimmed = value.trimStart();
    if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
      exts.push(json());
    }
    if (refsGetters) {
      exts.push(
        createReferenceAutocomplete(
          refsGetters.getBlocks,
          refsGetters.getEnvKeys,
        ),
      );
    }
    return exts;
  }, [refsGetters, value]);

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
import {
  deleteBlockExample,
  getBlockResult,
  insertBlockHistory,
  listBlockExamples,
  listBlockHistory,
  purgeBlockHistory,
  saveBlockExample,
  saveBlockResult,
  type BlockExample,
  type HistoryEntry,
} from "@/lib/tauri/commands";
import { useEnvironmentStore } from "@/stores/environment";

interface HttpFencedPanelProps {
  blockId: string;
  block: HttpPortalEntry["block"];
  entry: HttpPortalEntry;
  view: EditorView;
  filePath: string;
}

import {
  type ExecutionState,
  type SendAsFormat,
  MUTATION_METHODS,
} from "./shared";
import { HttpToolbar } from "./HttpToolbar";

function parseBody(body: string): HttpMessageParsed {
  const legacy = parseLegacyHttpBody(body);
  if (legacy) return legacyToHttpMessage(legacy);
  return parseHttpMessageBody(body);
}

function deriveHost(rawUrl: string): string | null {
  if (!rawUrl) return null;
  try {
    const u = new URL(rawUrl);
    return u.host;
  } catch {
    return null;
  }
}

import { formatBytes } from "./shared";
import { HttpStatusBar } from "./HttpStatusBar";
import { HttpResultTabs } from "./HttpResultTabs";
import { HttpFormMode } from "./HttpFormMode";
import { HttpSettingsDrawer } from "./HttpSettingsDrawer";

// RFC 7230 header-name token characters. Reqwest rejects anything outside
// this set (notably whitespace, control chars, `{`, `}`, `(`, `)`, `,`,
// `:`, `;`, `<`, `>`, `=`, `@`, `[`, `\`, `]`, `?`, `/`, `"`, etc).
const HTTP_TOKEN_RE = /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/;

function isValidHeaderName(name: string): boolean {
  return HTTP_TOKEN_RE.test(name);
}

/** Build the executor params from the parsed-and-resolved request.
 *
 * `{{ref}}` is resolved in BOTH the key and the value of every header /
 * query param — keys must be resolvable too, otherwise a header name like
 * `{{auth.header_name}}` would reach reqwest verbatim and fail with
 * `builder error` (reqwest rejects `{` in header names per RFC 7230).
 *
 * Rows whose key resolves to empty are dropped as a safety net so a stray
 * `headers:` label or an unresolved ref doesn't generate an invalid request.
 *
 * Returns the executor params plus a list of validation errors collected
 * along the way (e.g. a header name that resolves to a value containing
 * whitespace — invalid per RFC 7230). The caller surfaces these to the
 * user instead of letting reqwest emit a generic `builder error`.
 */
function buildExecutorParams(
  parsed: HttpMessageParsed,
  resolveText: (s: string) => string,
  timeoutMs: number | undefined,
  settings: HttpBlockSettings = {},
): { params: Record<string, unknown>; errors: string[] } {
  const errors: string[] = [];

  const resolveHeaders = (rows: HttpMessageParsed["headers"]) =>
    rows
      .filter((r) => r.enabled)
      .map((r) => ({
        rawKey: r.key,
        key: resolveText(r.key).trim(),
        value: resolveText(r.value),
      }))
      .filter((r) => {
        if (r.key.length === 0) return false;
        if (!isValidHeaderName(r.key)) {
          errors.push(
            `Invalid header name "${r.key}"` +
              (r.rawKey !== r.key ? ` (resolved from "${r.rawKey}")` : "") +
              " — header names cannot contain spaces or special characters.",
          );
          return false;
        }
        return true;
      })
      .map(({ key, value }) => ({ key, value }));

  const resolveQueryParams = (rows: HttpMessageParsed["params"]) =>
    rows
      .filter((r) => r.enabled)
      .map((r) => ({
        key: resolveText(r.key).trim(),
        value: resolveText(r.value),
      }))
      .filter((r) => r.key.length > 0);

  const params: Record<string, unknown> = {
    method: parsed.method,
    url: resolveText(parsed.url),
    params: resolveQueryParams(parsed.params),
    headers: resolveHeaders(parsed.headers),
    body: parsed.body ? resolveText(parsed.body) : "",
  };
  if (timeoutMs !== undefined) params.timeout_ms = timeoutMs;
  // Forward only explicit overrides; backend defaults apply when absent.
  if (settings.followRedirects === false) params.follow_redirects = false;
  if (settings.verifySsl === false) params.verify_ssl = false;
  if (settings.encodeUrl === false) params.encode_url = false;
  if (settings.trimWhitespace === false) params.trim_whitespace = false;
  return { params, errors };
}

/**
 * Body tab content for the form mode. Picks a UI based on the `Content-Type`
 * driven `bodyMode` pill — text-ish modes keep the existing CodeMirror
 * editor; structured modes get table editors and file pickers.
 *
 * `none` is the only mode that intentionally renders no editor — it's a
 * prompt to pick a body type from the toolbar. The others all serialize
 * back to the canonical raw body via `onCommit`.
 */
function HttpBodyByMode({
  bodyMode,
  parsed,
  onCommit,
  onPickFile,
  refsGetters,
}: {
  bodyMode: HttpBodyMode;
  parsed: HttpMessageParsed;
  onCommit: (next: string) => void;
  onPickFile: () => Promise<string | null>;
  refsGetters?: {
    getBlocks: () => BlockContext[];
    getEnvKeys: () => (string | EnvKeyInfo)[];
  };
}) {
  if (bodyMode === "none") {
    return (
      <Box px={2} py={3}>
        <Text fontSize="xs" color="fg.muted">
          No body. Pick a Content-Type from the toolbar pill to add one.
        </Text>
      </Box>
    );
  }

  if (bodyMode === "form-urlencoded") {
    return (
      <FormUrlEncodedTable
        body={parsed.body}
        onCommit={onCommit}
        refsGetters={refsGetters}
      />
    );
  }

  if (bodyMode === "multipart") {
    return (
      <MultipartTable
        body={parsed.body}
        onCommit={onCommit}
        onPickFile={onPickFile}
      />
    );
  }

  if (bodyMode === "binary") {
    return (
      <BinaryFilePicker
        body={parsed.body}
        onCommit={onCommit}
        onPickFile={onPickFile}
      />
    );
  }

  // json / xml / text fall through to the existing CodeMirror editor with
  // sublanguage detection (JSON highlighted, XML / text plain).
  return (
    <HttpBodyCM
      value={parsed.body}
      onCommit={onCommit}
      refsGetters={refsGetters}
    />
  );
}

interface UrlEncodedRow {
  key: string;
  value: string;
}

function parseUrlEncoded(body: string): UrlEncodedRow[] {
  if (body.trim().length === 0) return [];
  return body
    .split("&")
    .map((seg) => {
      const eq = seg.indexOf("=");
      if (eq === -1) return { key: seg, value: "" };
      return { key: seg.slice(0, eq), value: seg.slice(eq + 1) };
    })
    .filter((r) => r.key.length > 0);
}

function stringifyUrlEncoded(rows: UrlEncodedRow[]): string {
  return rows
    .filter((r) => r.key.length > 0)
    .map((r) => (r.value ? `${r.key}=${r.value}` : r.key))
    .join("&");
}

function FormUrlEncodedTable({
  body,
  onCommit,
  refsGetters,
}: {
  body: string;
  onCommit: (next: string) => void;
  refsGetters?: {
    getBlocks: () => BlockContext[];
    getEnvKeys: () => (string | EnvKeyInfo)[];
  };
}) {
  const rows = useMemo(() => parseUrlEncoded(body), [body]);
  // Same pending-row pattern as `HttpFormPanel`: rows with empty `key`
  // would not survive a `parseUrlEncoded → stringifyUrlEncoded` round-trip,
  // so we hold them locally until the user fills the key.
  const [pending, setPending] = useState<UrlEncodedRow[]>([]);

  const updateRow = useCallback(
    (displayIndex: number, patch: Partial<UrlEncodedRow>) => {
      if (displayIndex < rows.length) {
        const next = rows.slice();
        next[displayIndex] = { ...next[displayIndex], ...patch };
        onCommit(stringifyUrlEncoded(next));
        return;
      }
      // Read-and-decide outside setPending — calling onCommit from within
      // a setState updater double-fires under StrictMode.
      const pIdx = displayIndex - rows.length;
      const current = pending[pIdx];
      if (!current) return;
      const updated = { ...current, ...patch };
      if (updated.key.trim() !== "") {
        setPending((prev) => prev.filter((_, i) => i !== pIdx));
        onCommit(stringifyUrlEncoded([...rows, updated]));
      } else {
        setPending((prev) => {
          const list = prev.slice();
          list[pIdx] = updated;
          return list;
        });
      }
    },
    [rows, pending, onCommit],
  );
  const addRow = useCallback(() => {
    setPending((prev) => [...prev, { key: "", value: "" }]);
  }, []);
  const deleteRow = useCallback(
    (displayIndex: number) => {
      if (displayIndex < rows.length) {
        onCommit(
          stringifyUrlEncoded(rows.filter((_, idx) => idx !== displayIndex)),
        );
        return;
      }
      const pIdx = displayIndex - rows.length;
      setPending((prev) => prev.filter((_, i) => i !== pIdx));
    },
    [rows, onCommit],
  );

  const merged = [...rows, ...pending];

  return (
    <Box>
      {merged.length === 0 && (
        <Text fontSize="xs" color="fg.muted" px={3} py={2}>
          (no fields — application/x-www-form-urlencoded)
        </Text>
      )}
      {merged.map((row, i) => (
        <Flex
          key={`urlenc-${i}`}
          align="center"
          gap={1}
          px={2}
          py={1}
          borderBottomWidth="1px"
          borderColor="border.muted"
          _last={{ borderBottomWidth: 0 }}
        >
          <Box flex={1}>
            <HttpInlineCM
              placeholder="key"
              value={row.key}
              onCommit={(next) => updateRow(i, { key: next })}
              refsGetters={refsGetters}
            />
          </Box>
          <Box flex={2}>
            <HttpInlineCM
              placeholder="value"
              value={row.value}
              onCommit={(next) => updateRow(i, { value: next })}
              refsGetters={refsGetters}
            />
          </Box>
          <IconButton
            aria-label={`Delete field ${i}`}
            size="xs"
            variant="ghost"
            onClick={() => deleteRow(i)}
          >
            <LuX />
          </IconButton>
        </Flex>
      ))}
      <Box px={2} py={1}>
        <Button size="2xs" variant="ghost" onClick={addRow}>
          + add field
        </Button>
      </Box>
    </Box>
  );
}

function MultipartTable({
  body,
  onCommit,
  onPickFile,
}: {
  body: string;
  onCommit: (next: string) => void;
  onPickFile: () => Promise<string | null>;
}) {
  const parts = useMemo(() => parseMultipartBody(body), [body]);
  // Pending parts: parts with empty `name` would be dropped at re-parse
  // (Content-Disposition without a name is invalid). Held locally and
  // promoted to `parts` once the user types a name.
  const [pending, setPending] = useState<MultipartPart[]>([]);

  const commit = useCallback(
    (next: MultipartPart[]) => {
      onCommit(stringifyMultipartBody(next).body);
    },
    [onCommit],
  );

  const updatePart = useCallback(
    (displayIndex: number, patch: Partial<MultipartPart>) => {
      if (displayIndex < parts.length) {
        const next = parts.slice();
        next[displayIndex] = { ...next[displayIndex], ...patch };
        commit(next);
        return;
      }
      // Read-and-decide outside setPending — calling commit from within a
      // setState updater double-fires under StrictMode and would push the
      // part to `parts` twice.
      const pIdx = displayIndex - parts.length;
      const current = pending[pIdx];
      if (!current) return;
      const updated = { ...current, ...patch };
      if (updated.name.trim() !== "") {
        setPending((prev) => prev.filter((_, i) => i !== pIdx));
        commit([...parts, updated]);
      } else {
        setPending((prev) => {
          const list = prev.slice();
          list[pIdx] = updated;
          return list;
        });
      }
    },
    [parts, pending, commit],
  );

  const addPart = useCallback((kind: MultipartPartKind) => {
    setPending((prev) => [
      ...prev,
      { kind, name: "", value: "", enabled: true },
    ]);
  }, []);

  const deletePart = useCallback(
    (displayIndex: number) => {
      if (displayIndex < parts.length) {
        commit(parts.filter((_, idx) => idx !== displayIndex));
        return;
      }
      const pIdx = displayIndex - parts.length;
      setPending((prev) => prev.filter((_, i) => i !== pIdx));
    },
    [parts, commit],
  );

  const pickFileForPart = useCallback(
    async (displayIndex: number) => {
      const path = await onPickFile();
      if (!path) return;
      updatePart(displayIndex, {
        kind: "file",
        value: path,
        filename: undefined,
        contentType: undefined,
      });
    },
    [onPickFile, updatePart],
  );

  const merged = [...parts, ...pending];

  return (
    <Box>
      {merged.length === 0 && (
        <Text fontSize="xs" color="fg.muted" px={3} py={2}>
          (no parts — multipart/form-data)
        </Text>
      )}
      {merged.map((part, i) => (
        <Flex
          key={`multi-${i}`}
          align="center"
          gap={1}
          px={2}
          py={1}
          borderBottomWidth="1px"
          borderColor="border.muted"
          _last={{ borderBottomWidth: 0 }}
        >
          <input
            type="checkbox"
            aria-label={`Toggle part ${i}`}
            checked={part.enabled}
            onChange={(e) => updatePart(i, { enabled: e.target.checked })}
          />
          <Box flex={1}>
            <CommitOnBlurInput
              placeholder="name"
              value={part.name}
              onCommit={(next) => updatePart(i, { name: next })}
            />
          </Box>
          <Box minW="64px">
            <NativeSelectRoot size="sm">
              <NativeSelectField
                value={part.kind}
                onChange={(e) => {
                  const nextKind = e.target.value as MultipartPartKind;
                  if (nextKind === part.kind) return;
                  // Switching to file with no value yet → leave value empty;
                  // user clicks Choose…
                  updatePart(i, {
                    kind: nextKind,
                    // Clear file metadata when switching back to text.
                    ...(nextKind === "text" && {
                      filename: undefined,
                      contentType: undefined,
                    }),
                  });
                }}
              >
                <option value="text">text</option>
                <option value="file">file</option>
              </NativeSelectField>
            </NativeSelectRoot>
          </Box>
          <Box flex={2}>
            {part.kind === "file" ? (
              <Flex align="center" gap={1}>
                <Text
                  fontFamily="mono"
                  fontSize="xs"
                  color="fg.muted"
                  truncate
                  flex={1}
                  title={part.value}
                >
                  {part.value || "(no file selected)"}
                </Text>
                <Button
                  size="2xs"
                  variant="outline"
                  onClick={() => void pickFileForPart(i)}
                >
                  Choose…
                </Button>
              </Flex>
            ) : (
              <CommitOnBlurInput
                placeholder="value"
                value={part.value}
                onCommit={(next) => updatePart(i, { value: next })}
              />
            )}
          </Box>
          <IconButton
            aria-label={`Delete part ${i}`}
            size="xs"
            variant="ghost"
            onClick={() => deletePart(i)}
          >
            <LuX />
          </IconButton>
        </Flex>
      ))}
      <Flex px={2} py={1} gap={2}>
        <Button size="2xs" variant="ghost" onClick={() => addPart("text")}>
          + add text part
        </Button>
        <Button size="2xs" variant="ghost" onClick={() => addPart("file")}>
          + add file part
        </Button>
      </Flex>
    </Box>
  );
}

function BinaryFilePicker({
  body,
  onCommit,
  onPickFile,
}: {
  body: string;
  onCommit: (next: string) => void;
  onPickFile: () => Promise<string | null>;
}) {
  const current = isBinaryFileBody(body)?.path ?? null;

  const choose = useCallback(async () => {
    const path = await onPickFile();
    if (!path) return;
    onCommit(buildBinaryFileBody(path));
  }, [onPickFile, onCommit]);

  const clear = useCallback(() => {
    onCommit("");
  }, [onCommit]);

  return (
    <Box px={3} py={3}>
      <Flex align="center" gap={2}>
        <Text
          fontFamily="mono"
          fontSize="xs"
          color={current ? "fg" : "fg.muted"}
          truncate
          flex={1}
          title={current ?? undefined}
        >
          {current ?? "(no file selected — body is empty)"}
        </Text>
        <Button size="2xs" variant="outline" onClick={() => void choose()}>
          {current ? "Replace…" : "Choose…"}
        </Button>
        {current && (
          <Button size="2xs" variant="ghost" onClick={clear}>
            Clear
          </Button>
        )}
      </Flex>
      <Text fontSize="2xs" color="fg.muted" mt={2}>
        The file is read at request time and uploaded as the raw body.
      </Text>
    </Box>
  );
}

// "pretty" routes by content-type: image/pdf/html → visual preview;
// everything else → CM6 read-only viewer. "visualize" for JSON tree.
type BodyViewMode = "pretty" | "raw" | "visualize";

const cmReadOnlyBodyTheme = EditorView.theme({
  "&": { fontSize: "12px", maxHeight: "320px" },
  ".cm-content": {
    fontFamily: "var(--chakra-fonts-mono)",
    padding: "8px",
  },
  ".cm-gutters": { display: "none" },
  ".cm-scroller": { overflow: "auto", overscrollBehavior: "contain" },
  ".cm-activeLine": { backgroundColor: "transparent" },
});
const cmBodyReadOnly = EditorState.readOnly.of(true);

/** Pick a CM6 language extension based on the response Content-Type, with
 * a JSON/XML heuristic fallback when the header is missing or generic. */
function selectBodyLanguage(
  contentType: string | null,
  text: string,
): Extension | null {
  if (contentType) {
    const ct = contentType.split(";")[0].trim().toLowerCase();
    if (ct.includes("json")) return json();
    if (ct.includes("xml") || ct.includes("svg")) return xml();
    if (ct.includes("html")) return html();
  }
  const heuristic = detectLang(text, "pretty");
  if (heuristic === "json") return json();
  if (heuristic === "xml") return xml();
  return null;
}

function HttpBodyCM6Viewer({
  text,
  contentType,
}: {
  text: string;
  contentType: string | null;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const lang = selectBodyLanguage(contentType, text);
    const extensions: Extension[] = [
      cmBodyReadOnly,
      cmReadOnlyBodyTheme,
      syntaxHighlighting(oneDarkHighlightStyle),
      ...(lang ? [lang] : []),
    ];
    const view = new EditorView({
      state: EditorState.create({ doc: text, extensions }),
      parent: containerRef.current,
    });
    return () => {
      view.destroy();
    };
  }, [text, contentType]);

  return (
    <Box
      ref={containerRef}
      border="1px solid"
      borderColor="border"
      rounded="md"
      overflow="hidden"
    />
  );
}

function HttpBodyView({
  rawBody,
  prettyBody,
  response,
}: {
  rawBody: string;
  prettyBody: string;
  response: HttpResponseFull;
}) {
  const [view, setView] = useState<BodyViewMode>("pretty");

  const previewMeta = useMemo(() => detectPreview(response), [response]);
  const visualizeData = useMemo(
    () => parseJsonForVisualize(prettyBody),
    [prettyBody],
  );

  const text = view === "pretty" ? prettyBody : rawBody;

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      /* noop */
    }
  };

  return (
    <>
      <HStack gap={1} mb={1}>
        <Button
          size="2xs"
          variant={view === "pretty" ? "solid" : "ghost"}
          onClick={() => setView("pretty")}
        >
          pretty
        </Button>
        <Button
          size="2xs"
          variant={view === "raw" ? "solid" : "ghost"}
          onClick={() => setView("raw")}
        >
          raw
        </Button>
        {visualizeData !== null && (
          <Button
            size="2xs"
            variant={view === "visualize" ? "solid" : "ghost"}
            onClick={() => setView("visualize")}
          >
            ⊞ visualize
          </Button>
        )}
        <Box flex={1} />
        {(view === "pretty" || view === "raw") && (
          <IconButton
            aria-label="Copy body"
            size="2xs"
            variant="ghost"
            onClick={onCopy}
          >
            <LuClipboard />
          </IconButton>
        )}
      </HStack>
      {view === "pretty" && previewMeta.kind !== "none" && (
        <HttpBodyPreview meta={previewMeta} sizeBytes={response.size_bytes} />
      )}
      {view === "visualize" && visualizeData !== null && (
        <HttpJsonVisualizer data={visualizeData} />
      )}
      {((view === "pretty" && previewMeta.kind === "none") || view === "raw") &&
        (text ? (
          <HttpBodyCM6Viewer
            text={text}
            // `pretty` view picks lang from the response Content-Type;
            // `raw` view shows the bytes verbatim with no highlight (avoids
            // distorting non-pretty payloads like form-urlencoded).
            contentType={
              view === "pretty"
                ? (response.headers["content-type"] ??
                  response.headers["Content-Type"] ??
                  null)
                : null
            }
          />
        ) : (
          <Box as="pre" fontFamily="mono" fontSize="xs" color="fg.muted">
            (empty body)
          </Box>
        ))}
    </>
  );
}

type PreviewMeta =
  | { kind: "none" }
  | { kind: "image"; dataUrl: string; alt: string }
  | { kind: "pdf"; dataUrl: string }
  | { kind: "html"; html: string };

function detectPreview(response: HttpResponseFull): PreviewMeta {
  const ctRaw =
    response.headers["content-type"] ?? response.headers["Content-Type"] ?? "";
  const ct = ctRaw.split(";")[0].trim().toLowerCase();
  const body = response.body;

  // Binary base64 — image or PDF
  if (
    typeof body === "object" &&
    body !== null &&
    "encoding" in body &&
    (body as Record<string, unknown>).encoding === "base64"
  ) {
    const data = String((body as Record<string, unknown>).data ?? "");
    if (ct.startsWith("image/")) {
      return { kind: "image", dataUrl: `data:${ct};base64,${data}`, alt: ct };
    }
    if (ct === "application/pdf") {
      return { kind: "pdf", dataUrl: `data:application/pdf;base64,${data}` };
    }
    return { kind: "none" };
  }

  // HTML — rendered in a sandboxed iframe (no scripts).
  if (ct === "text/html" && typeof body === "string") {
    return { kind: "html", html: body };
  }

  return { kind: "none" };
}

function HttpBodyPreview({
  meta,
  sizeBytes,
}: {
  meta: PreviewMeta;
  sizeBytes: number;
}) {
  // Lifecycle: HTML preview uses a blob URL we must revoke on unmount.
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  useEffect(() => {
    if (meta.kind !== "html") {
      setBlobUrl(null);
      return;
    }
    const url = URL.createObjectURL(
      new Blob([meta.html], { type: "text/html" }),
    );
    setBlobUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [meta]);

  const [open, setOpen] = useState(false);

  if (meta.kind === "none") {
    return (
      <Text fontSize="xs" color="fg.muted">
        Preview not available for this response.
      </Text>
    );
  }

  const label =
    meta.kind === "image"
      ? "Image preview"
      : meta.kind === "pdf"
        ? "PDF preview"
        : "HTML preview";

  // Image renders inline — no internal scroll, so no leak. The expand
  // button still gives access to the fullscreen viewer for big images.
  if (meta.kind === "image") {
    return (
      <>
        <Box
          position="relative"
          bg="bg.subtle"
          borderWidth="1px"
          borderColor="border"
          borderRadius="sm"
          p={2}
          display="flex"
          justifyContent="center"
          alignItems="center"
          maxH="400px"
          overflow="hidden"
        >
          <img
            src={meta.dataUrl}
            alt={meta.alt}
            style={{
              maxWidth: "100%",
              maxHeight: "380px",
              objectFit: "contain",
              display: "block",
            }}
          />
          <IconButton
            aria-label="Open image fullscreen"
            size="xs"
            variant="solid"
            onClick={() => setOpen(true)}
            position="absolute"
            top={2}
            right={2}
            opacity={0.85}
            _hover={{ opacity: 1 }}
          >
            <LuExpand />
          </IconButton>
        </Box>
        {open && (
          <PreviewOverlay
            meta={meta}
            blobUrl={blobUrl}
            label={label}
            onClose={() => setOpen(false)}
          />
        )}
      </>
    );
  }

  // PDF / HTML — richer placeholder card with icon + type + size + CTA.
  const Icon = meta.kind === "pdf" ? LuFileText : LuGlobe;
  const typeLine = meta.kind === "pdf" ? "PDF document" : "HTML page";

  return (
    <>
      <Box
        bg="bg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        px={4}
        py={3}
        display="flex"
        alignItems="center"
        gap={3}
      >
        <Box
          display="flex"
          alignItems="center"
          justifyContent="center"
          w="40px"
          h="40px"
          borderRadius="sm"
          bg="bg.panel"
          color="fg.muted"
          flexShrink={0}
        >
          <Box fontSize="20px">
            <Icon />
          </Box>
        </Box>
        <Box flex={1} display="flex" flexDirection="column" gap={0.5} minW={0}>
          <Text fontSize="sm" fontWeight="medium">
            {typeLine}
          </Text>
          <Text fontSize="xs" color="fg.muted">
            {formatBytes(sizeBytes)} · click to open in a focused viewer
          </Text>
        </Box>
        <Button
          size="sm"
          variant="outline"
          onClick={() => setOpen(true)}
          disabled={meta.kind === "html" && !blobUrl}
          flexShrink={0}
        >
          <LuExpand /> Open
        </Button>
      </Box>
      {open && (
        <PreviewOverlay
          meta={meta}
          blobUrl={blobUrl}
          label={label}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}

/** Portal + Box (not Dialog — would steal CM6 focus). Locks body scroll to
 *  contain iframe wheel events. Dismissed by Esc, backdrop click, or close. */
function PreviewOverlay({
  meta,
  blobUrl,
  label,
  onClose,
}: {
  meta: PreviewMeta;
  blobUrl: string | null;
  label: string;
  onClose: () => void;
}) {
  // Lock body scroll: iframe wheel events leak out in the Tauri webview.
  useEffect(() => {
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prevOverflow;
    };
  }, []);

  // Window-level — iframe steals focus.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <Portal>
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.700"
        zIndex={3000}
        display="flex"
        alignItems="center"
        justifyContent="center"
        p={6}
        onClick={onClose}
        role="dialog"
        aria-modal="true"
        backdropFilter="blur(2px)"
      >
        <Box
          bg="bg.panel"
          borderWidth="1px"
          borderColor="border"
          borderRadius="lg"
          boxShadow="2xl"
          w="90vw"
          h="90vh"
          maxW="1400px"
          display="flex"
          flexDirection="column"
          overflow="hidden"
          onClick={(e) => e.stopPropagation()}
        >
          <Flex
            justify="space-between"
            align="center"
            px={4}
            py={2.5}
            bg="bg.subtle"
            borderBottomWidth="1px"
            borderColor="border"
          >
            <Text fontSize="sm" fontWeight="semibold">
              {label}
            </Text>
            <IconButton
              aria-label="Close preview"
              size="xs"
              variant="ghost"
              onClick={onClose}
            >
              <LuX />
            </IconButton>
          </Flex>
          <Box flex={1} overflow="hidden" bg="bg" p={4}>
            {meta.kind === "image" && (
              <Box
                w="100%"
                h="100%"
                display="flex"
                alignItems="center"
                justifyContent="center"
                bg="bg.subtle"
                borderRadius="md"
              >
                <img
                  src={meta.dataUrl}
                  alt={meta.alt}
                  style={{
                    maxWidth: "100%",
                    maxHeight: "100%",
                    objectFit: "contain",
                  }}
                />
              </Box>
            )}
            {meta.kind === "pdf" && (
              <iframe
                src={meta.dataUrl}
                title="PDF preview"
                style={{
                  width: "100%",
                  height: "100%",
                  border: "1px solid var(--chakra-colors-border)",
                  borderRadius: "var(--chakra-radii-md)",
                  display: "block",
                  background: "white",
                }}
              />
            )}
            {meta.kind === "html" && blobUrl && (
              <iframe
                src={blobUrl}
                // `sandbox=""` (empty value) is the strictest policy: no
                // scripts, no forms, no same-origin, no popups. Layout-
                // only rendering.
                sandbox=""
                title="HTML preview"
                style={{
                  width: "100%",
                  height: "100%",
                  border: "1px solid var(--chakra-colors-border)",
                  borderRadius: "var(--chakra-radii-md)",
                  display: "block",
                  background: "white",
                }}
              />
            )}
          </Box>
        </Box>
      </Box>
    </Portal>
  );
}

function parseJsonForVisualize(prettyBody: string): unknown {
  const trimmed = prettyBody.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

/**
 * Flat node in a JSON tree. The visualizer flattens the tree into a linear
 * list of these (with `container-open` / `container-close` markers) so the
 * virtualizer can render a fixed number of rows regardless of payload size.
 */
type JsonFlatNode =
  | {
      kind: "leaf";
      depth: number;
      path: string;
      label?: string;
      value: unknown;
    }
  | {
      kind: "container-open";
      depth: number;
      path: string;
      label?: string;
      containerKind: "array" | "object";
      length: number;
      value: unknown;
      expanded: boolean;
    }
  | {
      kind: "container-close";
      depth: number;
      path: string;
      containerKind: "array" | "object";
    };

/** Default expansion: root always open; depth 1 open if container has ≤ 20
 * children. Anything deeper or larger starts collapsed — keeps the initial
 * row count bounded so virtualization cost is predictable. */
function shouldDefaultExpand(value: unknown, depth: number): boolean {
  if (depth === 0) return true;
  if (depth === 1) {
    if (Array.isArray(value)) return value.length <= 20;
    if (value !== null && typeof value === "object") {
      return Object.keys(value as Record<string, unknown>).length <= 20;
    }
  }
  return false;
}

function initialCollapsedPaths(data: unknown): Set<string> {
  const collapsed = new Set<string>();
  const walk = (value: unknown, path: string, depth: number) => {
    if (value === null || typeof value !== "object") return;
    if (!shouldDefaultExpand(value, depth)) {
      collapsed.add(path);
      return;
    }
    if (Array.isArray(value)) {
      (value as unknown[]).forEach((v, i) =>
        walk(v, path ? `${path}.${i}` : String(i), depth + 1),
      );
    } else {
      Object.entries(value as Record<string, unknown>).forEach(([k, v]) =>
        walk(v, path ? `${path}.${k}` : k, depth + 1),
      );
    }
  };
  walk(data, "", 0);
  return collapsed;
}

function flattenJson(data: unknown, collapsed: Set<string>): JsonFlatNode[] {
  const out: JsonFlatNode[] = [];
  const walk = (
    value: unknown,
    path: string,
    depth: number,
    label?: string,
  ) => {
    if (value === null || typeof value !== "object") {
      out.push({ kind: "leaf", depth, path, label, value });
      return;
    }
    const isArray = Array.isArray(value);
    const length = isArray
      ? (value as unknown[]).length
      : Object.keys(value as Record<string, unknown>).length;
    const expanded = !collapsed.has(path);
    out.push({
      kind: "container-open",
      depth,
      path,
      label,
      containerKind: isArray ? "array" : "object",
      length,
      value,
      expanded,
    });
    if (expanded) {
      if (isArray) {
        (value as unknown[]).forEach((v, i) =>
          walk(v, path ? `${path}.${i}` : String(i), depth + 1, String(i)),
        );
      } else {
        Object.entries(value as Record<string, unknown>).forEach(([k, v]) =>
          walk(v, path ? `${path}.${k}` : k, depth + 1, k),
        );
      }
      out.push({
        kind: "container-close",
        depth,
        path: `${path}::close`,
        containerKind: isArray ? "array" : "object",
      });
    }
  };
  walk(data, "", 0);
  return out;
}

/**
 * JSON tree viewer with right-click context menu, virtualized via
 * `@tanstack/react-virtual`. Flattens the tree into a linear list of
 * visible rows (re-flattened only when collapse-state or `data` changes)
 * and lets the virtualizer paint just the rows in the viewport. Replaces
 * the prior recursive `JsonNode` that tried to mount one DOM element per
 * key+value and choked on responses with ≥ 5k objects.
 */
function HttpJsonVisualizer({ data }: { data: unknown }) {
  const [collapsed, setCollapsed] = useState<Set<string>>(() =>
    initialCollapsedPaths(data),
  );
  // Reset collapse state when the underlying data identity changes (new
  // execution) — otherwise the previous response's open/closed paths leak
  // into the new tree.
  useEffect(() => {
    setCollapsed(initialCollapsedPaths(data));
  }, [data]);

  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    path: string;
    value: unknown;
  } | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);

  const flat = useMemo(() => flattenJson(data, collapsed), [data, collapsed]);

  const parentRef = useRef<HTMLDivElement | null>(null);
  const virtualizer = useVirtualizer({
    count: flat.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 20,
    overscan: 12,
    getItemKey: (index) => flat[index]?.path ?? `idx-${index}`,
  });

  const toggle = useCallback((path: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const onCopy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      /* noop */
    }
  }, []);

  const onContextMenu = useCallback(
    (e: React.MouseEvent, path: string, value: unknown) => {
      e.preventDefault();
      setMenu({ x: e.clientX, y: e.clientY, path, value });
    },
    [],
  );

  return (
    <Box
      ref={parentRef}
      maxH="400px"
      overflow="auto"
      overscrollBehavior="contain"
      fontFamily="mono"
      fontSize="xs"
      onClick={closeMenu}
    >
      <Box position="relative" h={`${virtualizer.getTotalSize()}px`} w="100%">
        {virtualizer.getVirtualItems().map((vi) => {
          const node = flat[vi.index];
          if (!node) return null;
          return (
            <Box
              key={vi.key}
              position="absolute"
              top={`${vi.start}px`}
              left={0}
              right={0}
              h={`${vi.size}px`}
            >
              <JsonRow
                node={node}
                onToggle={toggle}
                onContextMenu={onContextMenu}
              />
            </Box>
          );
        })}
      </Box>
      {menu && (
        <Portal>
          <Box
            position="fixed"
            left={`${menu.x}px`}
            top={`${menu.y}px`}
            zIndex={2000}
            bg="bg.panel"
            borderWidth="1px"
            borderColor="border"
            borderRadius="sm"
            boxShadow="md"
            py={1}
            minW="160px"
            onClick={(e) => e.stopPropagation()}
          >
            <Box
              as="button"
              w="100%"
              textAlign="left"
              px={3}
              py={1.5}
              fontSize="xs"
              _hover={{ bg: "bg.muted" }}
              onClick={() => {
                void onCopy(`response.body.${menu.path}`.replace(/\.$/, ""));
                closeMenu();
              }}
            >
              Copy path
            </Box>
            <Box
              as="button"
              w="100%"
              textAlign="left"
              px={3}
              py={1.5}
              fontSize="xs"
              _hover={{ bg: "bg.muted" }}
              onClick={() => {
                const text =
                  typeof menu.value === "string"
                    ? menu.value
                    : JSON.stringify(menu.value);
                void onCopy(text);
                closeMenu();
              }}
            >
              Copy value
            </Box>
          </Box>
        </Portal>
      )}
    </Box>
  );
}

/** Single visible row in the virtualized JSON tree. Receives a flat node
 *  produced by `flattenJson` and renders one of: leaf, container-open
 *  (clickable to toggle), container-close (closing brace). */
function JsonRow({
  node,
  onToggle,
  onContextMenu,
}: {
  node: JsonFlatNode;
  onToggle: (path: string) => void;
  onContextMenu: (e: React.MouseEvent, path: string, value: unknown) => void;
}) {
  // 12px per depth level + 8px gutter. Inline padding so virtualizer's
  // absolute positioning composes cleanly with the indent.
  const indent = `${node.depth * 12 + 8}px`;

  if (node.kind === "container-close") {
    return (
      <Box pl={indent} fontFamily="mono" fontSize="xs" color="fg.muted">
        {node.containerKind === "array" ? "]" : "}"}
      </Box>
    );
  }

  if (node.kind === "leaf") {
    return (
      <Box
        pl={indent}
        onContextMenu={(e) => onContextMenu(e, node.path, node.value)}
        _hover={{ bg: "bg.subtle" }}
        whiteSpace="nowrap"
        overflow="hidden"
        textOverflow="ellipsis"
      >
        {node.label !== undefined && (
          <Text as="span" color="purple.fg">
            {node.label}
            {": "}
          </Text>
        )}
        <Text as="span" color={primitiveColor(node.value)}>
          {primitiveDisplay(node.value)}
        </Text>
      </Box>
    );
  }

  // container-open
  return (
    <Box
      as="button"
      pl={indent}
      textAlign="left"
      w="100%"
      onClick={() => onToggle(node.path)}
      onContextMenu={(e) => onContextMenu(e, node.path, node.value)}
      _hover={{ bg: "bg.subtle" }}
      display="flex"
      alignItems="center"
      gap={1}
    >
      <Text as="span" color="fg.muted" w="12px">
        {node.expanded ? "▾" : "▸"}
      </Text>
      {node.label !== undefined && (
        <Text as="span" color="purple.fg">
          {node.label}:
        </Text>
      )}
      <Text as="span" color="fg.muted" fontSize="2xs">
        {node.containerKind === "array"
          ? `Array(${node.length})`
          : `Object{${node.length}}`}
      </Text>
    </Box>
  );
}

function primitiveDisplay(v: unknown): string {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  if (typeof v === "string") return `"${v}"`;
  return String(v);
}

function primitiveColor(v: unknown): string {
  if (v === null || v === undefined) return "fg.muted";
  if (typeof v === "string") return "green.fg";
  if (typeof v === "number") return "blue.fg";
  if (typeof v === "boolean") return "orange.fg";
  return "fg";
}

function detectLang(text: string, view: "pretty" | "raw"): string | null {
  // Pretty mode: try JSON first (most common), fall back to xml/html on
  // angle-bracket starts. Raw mode: trust the bytes — same heuristic.
  void view;
  const trimmed = text.trimStart();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      JSON.parse(trimmed);
      return "json";
    } catch {
      // fall through
    }
  }
  if (trimmed.startsWith("<")) return "xml";
  return null;
}

export const HttpFencedPanel = memo(function HttpFencedPanel({
  blockId,
  block,
  entry,
  view,
  filePath,
}: HttpFencedPanelProps) {
  const parsed = useMemo(() => parseBody(block.body), [block.body]);
  const host = useMemo(() => deriveHost(parsed.url), [parsed.url]);

  // executionState / response / error / durationMs / cached + the
  // AbortController + run/cancel are owned by useExecutableBlock (A2);
  // wired below once recordHistory + the adapter callbacks exist.
  const [lastRunAt, setLastRunAt] = useState<Date | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [settings, setSettings] = useBlockSettings(
    filePath,
    block.metadata.alias,
  );
  // Cumulative bytes received during a streamed response. Only meaningful
  // while `executionState === "running"` and stays at 0 for fast responses
  // that finish before any BodyChunk fires. Reset on every new run.
  const [downloadingBytes, setDownloadingBytes] = useState(0);

  // Cached `{{ref}}` autocomplete context for the form-mode inputs —
  // refresh-on-doc-structure-change + stable getters that the
  // autocomplete extension reads without forcing a re-render.
  const refsGetters = useHttpRefsContext(view, block.from, filePath);

  // ── Drawer data (uses a ref for `applyCachedResult` since the FSM
  // hook hasn't run yet; populated by the `useEffect` after
  // `useExecutableBlock`). The ref is read at click-time, so by the
  // user actually invoking `restoreExample` the FSM is mounted. ──
  const applyCachedResultRef = useRef<
    ((r: HttpResponseFull, elapsed?: number) => void) | null
  >(null);
  const applyCachedResultStable = useCallback(
    (r: HttpResponseFull, elapsed?: number) =>
      applyCachedResultRef.current?.(r, elapsed),
    [],
  );
  const closeDrawer = useCallback(() => setDrawerOpen(false), []);
  const {
    historyEntries,
    examples,
    recordHistory,
    bumpHistoryTick,
    purgeHistory,
    saveExample,
    restoreExample,
    deleteExample,
  } = useHttpDrawerData({
    filePath,
    alias: block.metadata.alias,
    drawerOpen,
    settings,
    applyCachedResult: applyCachedResultStable,
    setLastRunAt,
    closeDrawer,
  });

  // Hydrate from cache on mount / body change. Mutations are skipped:
  // re-running a destructive POST without a fresh user click is unsafe.
  useEffect(() => {
    if (MUTATION_METHODS.has(parsed.method)) return;
    if (!parsed.url || !parsed.url.trim()) return;
    let cancelled = false;
    void (async () => {
      try {
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
        const hash = await computeHttpCacheHash(
          {
            method: parsed.method,
            url: parsed.url,
            params: parsed.params
              .filter((p) => p.enabled)
              .map((p) => ({ key: p.key, value: p.value })),
            headers: parsed.headers
              .filter((h) => h.enabled)
              .map((h) => ({ key: h.key, value: h.value })),
            body: parsed.body,
          },
          envVars,
        );
        const hit = await getBlockResult(filePath, hash);
        if (cancelled || !hit) return;
        try {
          const stored = JSON.parse(hit.response) as unknown;
          const norm = normalizeHttpResponse(stored);
          setResponse(norm);
          setExecutionState("success");
          setDurationMs(norm.elapsed_ms || hit.elapsed_ms);
          setLastRunAt(hit.executed_at ? new Date(hit.executed_at) : null);
          setCached(true);
        } catch {
          // Ignore corrupt cache entries.
        }
      } catch {
        // Cache lookup is best-effort.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [parsed, filePath]);

  /** Persist a row in `block_run_history`. Best-effort: a write failure
   * (e.g. SQLite locked momentarily) doesn't block the user from seeing
   * the response. The drawer history list is the only consumer; missing
   * a row is at most an aesthetic miss. */
  const recordHistory = useCallback(
    async (info: {
      method: string;
      url: string;
      status: number | null;
      requestSize: number | null;
      responseSize: number | null;
      elapsedMs: number;
      outcome: "success" | "error" | "cancelled";
    }) => {
      const alias = block.metadata.alias;
      if (!alias) return; // No alias → no stable key to bucket history under.
      if (settings.historyDisabled === true) return;
      try {
        await insertBlockHistory({
          file_path: filePath,
          block_alias: alias,
          method: info.method,
          url_canonical: info.url,
          status: info.status,
          request_size: info.requestSize,
          response_size: info.responseSize,
          elapsed_ms: info.elapsedMs,
          outcome: info.outcome,
        });
        setHistoryRefreshTick((t) => t + 1);
      } catch {
        /* Best-effort. */
      }
    },
    [block.metadata.alias, filePath, settings.historyDisabled],
  );

  const prepare = useCallback(
    async ({
      blocksAbove,
      envVars,
      blockFrom,
    }: {
      blocksAbove: BlockContext[];
      envVars: Record<string, string>;
      blockFrom: number;
    }) => {
      const resolveText = (text: string) =>
        resolveAllReferences(text, blocksAbove, blockFrom, envVars).resolved;
      const { params, errors } = buildExecutorParams(
        parsed,
        resolveText,
        block.metadata.timeoutMs,
        settings,
      );
      if (errors.length > 0) return { error: errors.join("\n") };
      return { params };
    },
    [parsed, block.metadata.timeoutMs, settings],
  );

  // Mutations (POST/PUT/PATCH/DELETE) are never cached — re-executed
  // every time so we never serve a stale destructive result.
  const persist = useCallback(
    async (
      resp: HttpResponseFull,
      elapsed: number,
      { envVars }: { envVars: Record<string, string> },
    ) => {
      if (MUTATION_METHODS.has(parsed.method)) return;
      const hash = await computeHttpCacheHash(
        {
          method: parsed.method,
          url: parsed.url,
          params: parsed.params
            .filter((p) => p.enabled)
            .map((p) => ({ key: p.key, value: p.value })),
          headers: parsed.headers
            .filter((h) => h.enabled)
            .map((h) => ({ key: h.key, value: h.value })),
          body: parsed.body,
        },
        envVars,
      );
      await saveBlockResult(
        filePath,
        hash,
        "success",
        JSON.stringify(resp),
        elapsed,
        null,
      );
    },
    [parsed, filePath],
  );

  const onOutcome = useCallback(
    (
      outcome:
        | { status: "success"; response: HttpResponseFull }
        | { status: "error"; message: string }
        | { status: "cancelled" },
      elapsed: number,
    ) => {
      if (outcome.status === "success") {
        setLastRunAt(new Date());
        void recordHistory({
          method: parsed.method,
          url: parsed.url,
          status: outcome.response.status_code,
          requestSize: parsed.body.length || null,
          responseSize: outcome.response.size_bytes,
          elapsedMs: outcome.response.elapsed_ms || elapsed,
          outcome: "success",
        });
        return;
      }
      void recordHistory({
        method: parsed.method,
        url: parsed.url,
        status: null,
        requestSize: parsed.body.length || null,
        responseSize: null,
        elapsedMs: elapsed,
        outcome: outcome.status === "cancelled" ? "cancelled" : "error",
      });
    },
    [parsed, recordHistory],
  );

  const onRunStart = useCallback(() => setDownloadingBytes(0), []);
  const onProgress = useCallback(
    (bytes: number) => setDownloadingBytes(bytes),
    [],
  );

  const {
    executionState,
    response,
    error,
    durationMs,
    cached,
    run: runBlock,
    cancel: cancelBlock,
    applyCachedResult,
  } = useExecutableBlock<HttpResponseFull>({
    idPrefix: "http",
    blockId,
    view,
    blockFrom: block.from,
    filePath,
    validate,
    prepare,
    execute: executeHttpStreamed,
    elapsedOf: httpElapsedOf,
    persist,
    onOutcome,
    onRunStart,
    onProgress,
  });

  // Wire the ref so `drawer.restoreExample` can call `applyCachedResult`.
  useEffect(() => {
    applyCachedResultRef.current = applyCachedResult;
  }, [applyCachedResult]);

  // Hydrate from cache on mount / body change.
  useHttpCacheHydrate({ parsed, filePath, applyCachedResult, setLastRunAt });

  const onOpenSettings = useCallback(() => {
    setDrawerOpen(true);
    bumpHistoryTick();
  }, [bumpHistoryTick]);

  // Load history rows when the drawer is open or a fresh row is inserted.
  useEffect(() => {
    if (!drawerOpen) return;
    const alias = block.metadata.alias;
    if (!alias) {
      setHistoryEntries([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows = await listBlockHistory(filePath, alias);
        if (!cancelled) setHistoryEntries(rows);
      } catch {
        if (!cancelled) setHistoryEntries([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [drawerOpen, filePath, block.metadata.alias, historyRefreshTick]);

  useEffect(() => {
    if (!drawerOpen) return;
    const alias = block.metadata.alias;
    if (!alias) {
      setExamples([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows = await listBlockExamples(filePath, alias);
        if (!cancelled) setExamples(rows);
      } catch {
        if (!cancelled) setExamples([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [drawerOpen, filePath, block.metadata.alias, examplesRefreshTick]);

  /**
   * Pre-computed snippets per format, refreshed whenever the parsed body
   * or environment context changes. We have to pre-compute because the
   * browser's clipboard API requires a *user gesture* — `await`-ing on
   * `collectBlocksAboveCM` / `getActiveVariables` inside the click handler
   * loses that gesture context and the call silently fails. Holding the
   * resolved snippets in state lets the click handler call `writeText`
   * synchronously inside the gesture window.
   */
  const [snippets, setSnippets] = useState<Record<SendAsFormat, string> | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const blocksAbove = await collectBlocksAboveCM(
          view.state.doc,
          block.from,
          filePath,
        );
        const envVars = await useEnvironmentStore
          .getState()
          .getActiveVariables();
        if (cancelled) return;
        const resolveText = (text: string) =>
          resolveAllReferences(text, blocksAbove, block.from, envVars).resolved;
        const resolved = {
          method: parsed.method,
          url: resolveText(parsed.url),
          params: parsed.params.map((p) => ({
            ...p,
            key: resolveText(p.key),
            value: resolveText(p.value),
          })),
          headers: parsed.headers.map((h) => ({
            ...h,
            key: resolveText(h.key),
            value: resolveText(h.value),
          })),
          body: parsed.body ? resolveText(parsed.body) : "",
        };
        if (cancelled) return;
        setSnippets({
          curl: toCurl(resolved),
          fetch: toFetch(resolved),
          python: toPython(resolved),
          httpie: toHTTPie(resolved),
          "http-file": toHttpFile(resolved),
        });
      } catch {
        if (!cancelled) setSnippets(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [block.from, filePath, parsed, view.state.doc]);

  const handleSendAs = useCallback(
    (format: SendAsFormat) => {
      const snippet = snippets?.[format];
      if (!snippet) return;

      if (format === "http-file") {
        // Save dialog flow can run async — no clipboard gesture to preserve.
        void (async () => {
          try {
            const defaultName = `${block.metadata.alias ?? "request"}.http`;
            const path = await saveDialog({
              defaultPath: defaultName,
              filters: [{ name: "HTTP request", extensions: ["http", "rest"] }],
            });
            if (!path) return;
            await writeFile(path, new TextEncoder().encode(snippet));
          } catch (e) {
            window.alert(
              `Failed to save: ${e instanceof Error ? e.message : String(e)}`,
            );
          }
        })();
        return;
      }

      // Synchronous call from inside the click handler — gesture context
      // is still active here.
      navigator.clipboard.writeText(snippet).catch(() => {
        /* clipboard denied — user can retry */
      });
    },
    [block.metadata.alias, snippets],
  );

  const copyAsCurl = useCallback(() => {
    handleSendAs("curl");
  }, [handleSendAs]);

  useEffect(() => {
    setHttpBlockActions(blockId, {
      onRun: () => void runBlock(),
      onCancel: cancelBlock,
      onOpenSettings,
      onCopyAsCurl: copyAsCurl,
    });
  }, [blockId, runBlock, cancelBlock, onOpenSettings, copyAsCurl]);

  // Cancel any in-flight run if the panel unmounts or the block id
  // changes after a doc-level edit. `cancelBlock` (from the hook)
  // aborts the in-flight controller; we also tell the backend.
  useEffect(() => {
    return () => {
      cancelBlock();
      void cancelBlockExecution(`http_${blockId}`);
    };
  }, [blockId, cancelBlock]);

  const updateMetadata = useCallback(
    (patch: Partial<HttpBlockMetadata>) => {
      const next: HttpBlockMetadata = { ...block.metadata, ...patch };
      const infoText = stringifyHttpFenceInfo(next);
      const openLine = view.state.doc.lineAt(block.openLineFrom);
      view.dispatch({
        changes: {
          from: openLine.from,
          to: openLine.to,
          insert: "```" + infoText,
        },
      });
    },
    [block.metadata, block.openLineFrom, view],
  );

  const deleteBlockFromDoc = useCallback(() => {
    const from = block.from;
    const to = Math.min(block.to + 1, view.state.doc.length);
    view.dispatch({ changes: { from, to, insert: "" } });
    setDrawerOpen(false);
  }, [block.from, block.to, view]);

  const replaceBody = useCallback(
    (nextRaw: string) => {
      // bodyFrom is the start of the first body line; bodyTo is end of the
      // last body line. Replace the whole range with the new canonical raw.
      view.dispatch({
        changes: {
          from: block.bodyFrom,
          to: block.bodyTo,
          insert: nextRaw,
        },
      });
    },
    [block.bodyFrom, block.bodyTo, view],
  );

  const onFormChange = useCallback(
    (next: HttpMessageParsed) => {
      replaceBody(stringifyHttpMessageBody(next));
    },
    [replaceBody],
  );

  const onToggleMode = useCallback(
    (next: "raw" | "form") => {
      if (next === (block.metadata.mode ?? "raw")) return;
      // Persist mode in the info string. Default raw is omitted; form is
      // explicit. Re-stringify the body too so toggling raw → form → raw
      // is a fixed point (canonical reformat).
      const reformatted = stringifyHttpMessageBody(parsed);
      // If the mode changed, also reformat the body to keep the contract
      // "form re-parses raw on each flip" idempotent.
      if (reformatted !== block.body) {
        replaceBody(reformatted);
      }
      const meta: HttpBlockMetadata = {
        ...block.metadata,
        mode: next === "raw" ? undefined : "form",
      };
      const infoText = stringifyHttpFenceInfo(meta);
      const openLine = view.state.doc.lineAt(block.openLineFrom);
      view.dispatch({
        changes: {
          from: openLine.from,
          to: openLine.to,
          insert: "```" + infoText,
        },
      });
    },
    [block.body, block.metadata, block.openLineFrom, parsed, replaceBody, view],
  );

  const onPickBodyMode = useCallback(
    (next: HttpBodyMode) => {
      const prev = deriveBodyMode(parsed.headers);
      if (prev === next) return;
      if (!isCompatibleSwitch(prev, next, parsed.body)) {
        toaster.create({
          type: "warning",
          title: "Body may be incompatible",
          description: `Switched to ${next}. Existing body still looks like ${prev}; clear or replace it if it doesn't match.`,
        });
      }
      const updated = setContentTypeForMode(parsed, next);
      replaceBody(stringifyHttpMessageBody(updated));
    },
    [parsed, replaceBody],
  );

  /** Open the OS-native file picker; returns the absolute path or `null` if
   *  the user cancelled. Lives at the panel root so the form-mode body tab
   *  doesn't have to import Tauri APIs directly. */
  const pickFile = useCallback(async (): Promise<string | null> => {
    try {
      const result = await openFileDialog({ multiple: false });
      return typeof result === "string" ? result : null;
    } catch {
      return null;
    }
  }, []);

  const toolbarNode = entry.toolbar;
  const formNode = entry.form;
  const resultNode = entry.result;
  const statusbarNode = entry.statusbar;
  const currentMode: "raw" | "form" =
    block.metadata.mode === "form" ? "form" : "raw";
  const currentBodyMode = deriveBodyMode(parsed.headers);

  return (
    <>
      {toolbarNode &&
        createPortal(
          <HttpToolbar
            alias={block.metadata.alias}
            method={parsed.method}
            host={host}
            mode={currentMode}
            bodyMode={currentBodyMode}
            executionState={executionState}
            onRun={() => void runBlock()}
            onCancel={cancelBlock}
            onOpenSettings={onOpenSettings}
            onToggleMode={onToggleMode}
            onPickBodyMode={onPickBodyMode}
          />,
          toolbarNode,
        )}

      {formNode &&
        createPortal(
          <HttpFormMode
            parsed={parsed}
            bodyMode={currentBodyMode}
            onChange={onFormChange}
            onPickFile={pickFile}
            refsGetters={refsGetters}
            InlineCM={HttpInlineCM}
            renderBodyTab={({
              parsed: p,
              onCommit,
              onPickFile: pick,
              refsGetters: r,
            }) => (
              <HttpBodyByMode
                bodyMode={currentBodyMode}
                parsed={p}
                onCommit={onCommit}
                onPickFile={pick}
                refsGetters={r}
              />
            )}
          />,
          formNode,
        )}

      {resultNode &&
        createPortal(
          <HttpResultTabs
            executionState={executionState}
            response={response}
            error={error}
            cached={cached}
            bodyView={(rawBody, prettyBody, resp) => (
              <HttpBodyView
                rawBody={rawBody}
                prettyBody={prettyBody}
                response={resp}
              />
            )}
          />,
          resultNode,
        )}

      {statusbarNode &&
        createPortal(
          <HttpStatusBar
            alias={block.metadata.alias}
            host={host}
            executionState={executionState}
            response={response}
            durationMs={durationMs}
            cached={cached}
            lastRunAt={lastRunAt}
            downloadingBytes={downloadingBytes}
            onSendAs={handleSendAs}
          />,
          statusbarNode,
        )}

      {drawerOpen && (
        <HttpSettingsDrawer
          metadata={block.metadata}
          history={historyEntries}
          examples={examples}
          settings={settings}
          canSaveExample={!!response && !!block.metadata.alias}
          onClose={closeDrawer}
          onUpdateMetadata={updateMetadata}
          onUpdateSettings={setSettings}
          onDelete={deleteBlockFromDoc}
          onPurgeHistory={purgeHistory}
          onSaveExample={async (name) => {
            if (!response) return;
            await saveExample(name, response);
          }}
          onRestoreExample={restoreExample}
          onDeleteExample={deleteExample}
        />
      )}
    </>
  );
});
