// HTTP block form-mode body editors: the body-tab dispatcher plus the
// form-urlencoded / multipart table editors and the binary file picker.
//
// Extracted verbatim from HttpFencedPanel.tsx (A1 / audit 03 §1 seam
// #3, done after seam #4 since these consume the inline CM editors).
// The orchestrator consumes only `HttpBodyByMode`; `parseUrlEncoded` /
// `stringifyUrlEncoded` (+ the `UrlEncodedRow` shape) are exported so
// the pure round-trip logic can be unit-tested. The table/picker
// components are module-internal.

import { useCallback, useMemo, useState } from "react";
import {
  Box,
  Button,
  Flex,
  IconButton,
  NativeSelectField,
  NativeSelectRoot,
  Text,
} from "@chakra-ui/react";
import { LuX } from "react-icons/lu";

import { type HttpMessageParsed } from "@/lib/blocks/http-message";
import {
  buildBinaryFileBody,
  isBinaryFileBody,
  parseMultipartBody,
  stringifyMultipartBody,
  type HttpBodyMode,
  type MultipartPart,
  type MultipartPartKind,
} from "@/lib/blocks/http-body-modes";
import type { BlockContext } from "@/lib/blocks/references";
import type { EnvKeyInfo } from "@/lib/blocks/cm-autocomplete";

import {
  CommitOnBlurInput,
  HttpInlineCM,
  HttpBodyCM,
} from "./HttpInlineEditors";

// ─────────────────────── Body tab dispatcher (Onda 2) ───────────────────────

/**
 * Body tab content for the form mode. Picks a UI based on the `Content-Type`
 * driven `bodyMode` pill — text-ish modes keep the existing CodeMirror
 * editor; structured modes get table editors and file pickers.
 *
 * `none` is the only mode that intentionally renders no editor — it's a
 * prompt to pick a body type from the toolbar. The others all serialize
 * back to the canonical raw body via `onCommit`.
 */
export function HttpBodyByMode({
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

// ─────────────────────── form-urlencoded ───────────────────────

export interface UrlEncodedRow {
  key: string;
  value: string;
}

export function parseUrlEncoded(body: string): UrlEncodedRow[] {
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

export function stringifyUrlEncoded(rows: UrlEncodedRow[]): string {
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

// ─────────────────────── multipart ───────────────────────

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

// ─────────────────────── binary ───────────────────────

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
