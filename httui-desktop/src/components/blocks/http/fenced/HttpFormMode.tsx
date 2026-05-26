import { Box, Button, Flex, IconButton, Tabs, Text } from "@chakra-ui/react";
import { LuX } from "react-icons/lu";
import {
  memo,
  useCallback,
  useRef,
  useState,
  type ComponentType,
  type ReactNode,
} from "react";
import type { EnvKeyInfo } from "@/lib/blocks/cm-autocomplete";
import type { BlockContext } from "@/lib/blocks/references";
import type {
  HttpBodyMode,
  HttpKVRow,
  HttpMessageParsed,
} from "@/lib/blocks/http-fence";

export type RefsGetters = {
  getBlocks: () => BlockContext[];
  getEnvKeys: () => (string | EnvKeyInfo)[];
};

export interface InlineCMProps {
  placeholder: string;
  value: string;
  onCommit: (next: string) => void;
  refsGetters?: RefsGetters;
}

interface HttpFormModeProps {
  parsed: HttpMessageParsed;
  bodyMode: HttpBodyMode;
  onChange: (next: HttpMessageParsed) => void;
  onPickFile: () => Promise<string | null>;
  refsGetters?: RefsGetters;
  /**
   * Single-line CodeMirror input component, injected by the parent panel
   * so this file does not depend on the CM6 runtime + reference-resolver
   * setup that lives in HttpFencedPanel.
   */
  InlineCM: ComponentType<InlineCMProps>;
  /**
   * Body tab content. The dispatcher (`HttpBodyByMode`) lives in the panel
   * because it pulls in form-urlencoded / multipart / binary editors with
   * their own deep dependencies.
   */
  renderBodyTab: (args: {
    parsed: HttpMessageParsed;
    onCommit: (next: string) => void;
    onPickFile: () => Promise<string | null>;
    refsGetters?: RefsGetters;
  }) => ReactNode;
}

/**
 * Tabular Params/Headers editor shown when `mode=form` and the cursor is
 * outside the block. Each input maintains a local draft and only re-emits
 * the canonical raw body on blur. Toggles, add, and delete commit
 * immediately — those are infrequent and not on the keystroke path.
 */
export function HttpFormMode({
  parsed,
  bodyMode: _bodyMode, // kept in the signature for parity; the body dispatch lives in renderBodyTab
  onChange,
  onPickFile,
  refsGetters,
  InlineCM,
  renderBodyTab,
}: HttpFormModeProps) {
  // ── Pending rows (local, not yet committed to the doc) ──
  // Rows with empty `key` would be dropped by the canonical stringifier
  // (a `?` query segment with no key, or a blank header line that breaks
  // header parsing). Keeping them in local state until the user types a
  // key avoids round-trip loss while still letting the doc remain the
  // source of truth for everything that's actually filled in.
  const [pending, setPending] = useState<{
    params: HttpKVRow[];
    headers: HttpKVRow[];
  }>({ params: [], headers: [] });

  // Refs that always carry the latest `parsed` / `pending` / `onChange` so
  // the row callbacks below can stay referentially stable. Without this,
  // every keystroke that committed a value would re-create every row's
  // `onCommit`, which forces the CodeMirror inline editor to re-render
  // and visibly flash.
  const parsedRef = useRef(parsed);
  const pendingRef = useRef(pending);
  const onChangeRef = useRef(onChange);
  parsedRef.current = parsed;
  pendingRef.current = pending;
  onChangeRef.current = onChange;

  const updateRow = useCallback(
    (
      kind: "params" | "headers",
      displayIndex: number,
      patch: Partial<HttpKVRow>,
    ) => {
      const parsed = parsedRef.current;
      const pending = pendingRef.current;
      const onChange = onChangeRef.current;

      const realLen = parsed[kind].length;
      if (displayIndex < realLen) {
        const rows = parsed[kind].slice();
        rows[displayIndex] = { ...rows[displayIndex], ...patch };
        onChange({ ...parsed, [kind]: rows });
        return;
      }
      const pIdx = displayIndex - realLen;
      const current = pending[kind][pIdx];
      if (!current) return;
      const updated = { ...current, ...patch };
      if (updated.key.trim() !== "") {
        // Promote pending → committed atomically (React 18 batches both).
        setPending((prev) => ({
          ...prev,
          [kind]: prev[kind].filter((_, i) => i !== pIdx),
        }));
        onChange({ ...parsed, [kind]: [...parsed[kind], updated] });
      } else {
        setPending((prev) => {
          const list = prev[kind].slice();
          list[pIdx] = updated;
          return { ...prev, [kind]: list };
        });
      }
    },
    [],
  );

  const addRow = useCallback((kind: "params" | "headers") => {
    setPending((prev) => ({
      ...prev,
      [kind]: [...prev[kind], { key: "", value: "", enabled: true }],
    }));
  }, []);

  const deleteRow = useCallback(
    (kind: "params" | "headers", displayIndex: number) => {
      const parsed = parsedRef.current;
      const onChange = onChangeRef.current;
      const realLen = parsed[kind].length;
      if (displayIndex < realLen) {
        const rows = parsed[kind].filter((_, i) => i !== displayIndex);
        onChange({ ...parsed, [kind]: rows });
        return;
      }
      const pIdx = displayIndex - realLen;
      setPending((prev) => ({
        ...prev,
        [kind]: prev[kind].filter((_, i) => i !== pIdx),
      }));
    },
    [],
  );

  const onBodyCommit = useCallback(
    (next: string) => onChange({ ...parsed, body: next }),
    [parsed, onChange],
  );

  const renderTable = (kind: "params" | "headers") => {
    const merged = [...parsed[kind], ...pending[kind]];
    return (
      <Box>
        {merged.length === 0 && (
          <Text fontSize="xs" color="fg.muted" px={3} py={2}>
            (no {kind})
          </Text>
        )}
        {merged.map((row, i) => (
          <KVRow
            key={`${kind}-${i}`}
            kind={kind}
            index={i}
            row={row}
            updateRow={updateRow}
            deleteRow={deleteRow}
            refsGetters={refsGetters}
            InlineCM={InlineCM}
          />
        ))}
        <Box px={2} py={1}>
          <Button size="2xs" variant="ghost" onClick={() => addRow(kind)}>
            + add {kind === "params" ? "param" : "header"}
          </Button>
        </Box>
      </Box>
    );
  };

  return (
    <Box px={2} py={2}>
      <Text
        fontSize="2xs"
        fontWeight="semibold"
        color="fg.muted"
        textTransform="uppercase"
        letterSpacing="wider"
        mb={1}
      >
        Request
      </Text>
      <Tabs.Root defaultValue="params" size="sm" variant="line">
        <Tabs.List>
          <Tabs.Trigger value="params">
            Params ({parsed.params.length + pending.params.length})
          </Tabs.Trigger>
          <Tabs.Trigger value="headers">
            Headers ({parsed.headers.length + pending.headers.length})
          </Tabs.Trigger>
          <Tabs.Trigger value="body">Body</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="params" px={0} pt={2}>
          {renderTable("params")}
        </Tabs.Content>
        <Tabs.Content value="headers" px={0} pt={2}>
          {renderTable("headers")}
        </Tabs.Content>
        <Tabs.Content value="body" px={0} pt={2}>
          {renderBodyTab({
            parsed,
            onCommit: onBodyCommit,
            onPickFile,
            refsGetters,
          })}
        </Tabs.Content>
      </Tabs.Root>
    </Box>
  );
}

// ─────────────────────── KV row ───────────────────────

interface KVRowProps {
  kind: "params" | "headers";
  index: number;
  row: HttpKVRow;
  updateRow: (
    kind: "params" | "headers",
    index: number,
    patch: Partial<HttpKVRow>,
  ) => void;
  deleteRow: (kind: "params" | "headers", index: number) => void;
  refsGetters?: RefsGetters;
  InlineCM: ComponentType<InlineCMProps>;
}

const KVRow = memo(function KVRow({
  kind,
  index,
  row,
  updateRow,
  deleteRow,
  refsGetters,
  InlineCM,
}: KVRowProps) {
  const onToggle = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) =>
      updateRow(kind, index, { enabled: e.target.checked }),
    [kind, index, updateRow],
  );
  const onKeyCommit = useCallback(
    (next: string) => updateRow(kind, index, { key: next }),
    [kind, index, updateRow],
  );
  const onValueCommit = useCallback(
    (next: string) => updateRow(kind, index, { value: next }),
    [kind, index, updateRow],
  );
  const onDescCommit = useCallback(
    (next: string) =>
      updateRow(kind, index, { description: next || undefined }),
    [kind, index, updateRow],
  );
  const onDelete = useCallback(
    () => deleteRow(kind, index),
    [kind, index, deleteRow],
  );

  return (
    <Flex
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
        aria-label={`Toggle ${kind} row ${index}`}
        checked={row.enabled}
        onChange={onToggle}
      />
      <Box flex={1}>
        <InlineCM
          placeholder="key"
          value={row.key}
          onCommit={onKeyCommit}
          refsGetters={refsGetters}
        />
      </Box>
      <Box flex={2}>
        <InlineCM
          placeholder="value"
          value={row.value}
          onCommit={onValueCommit}
          refsGetters={refsGetters}
        />
      </Box>
      <Box flex={1}>
        <InlineCM
          placeholder="description"
          value={row.description ?? ""}
          onCommit={onDescCommit}
          refsGetters={refsGetters}
        />
      </Box>
      <IconButton
        aria-label={`Delete ${kind} row ${index}`}
        size="xs"
        variant="ghost"
        onClick={onDelete}
      >
        <LuX />
      </IconButton>
    </Flex>
  );
});
