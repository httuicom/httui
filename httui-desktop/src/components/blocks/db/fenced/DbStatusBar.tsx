import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Box,
  Flex,
  HStack,
  IconButton,
  Menu,
  Portal,
  Text,
} from "@chakra-ui/react";
import {
  LuBraces,
  LuClipboard,
  LuDatabase,
  LuDownload,
  LuFileText,
  LuHardDriveDownload,
  LuTable2,
} from "react-icons/lu";
import {
  firstSelectResult,
  type DbResponse,
} from "@/components/blocks/db/types";
import {
  hasExportableRows,
  inferTableName,
  toCsv,
  toInserts,
  toJson,
  toMarkdown,
} from "@/lib/blocks/db-export";
import {
  formatElapsed,
  formatRelativeTime,
  type ExecutionState,
} from "./shared";

interface DbStatusBarProps {
  connection: string | undefined;
  /** Connection's `is_readonly` flag — used to tint the connection label. */
  isReadonly: boolean;
  /** Whether we resolved an actual Connection object (as opposed to just a
   *  raw identifier typed in the fence that doesn't match any record). */
  hasActiveConnection: boolean;
  durationMs: number | null;
  executionState: ExecutionState;
  response: DbResponse | null;
  cached: boolean;
  /** Raw query body — fed through the export menu for INSERT generation. */
  query: string;
  /** Alias — fallback filename when saving an export. */
  alias: string | undefined;
}

export function DbStatusBar({
  connection,
  isReadonly,
  hasActiveConnection,
  durationMs,
  executionState,
  response,
  cached,
  query,
  alias,
}: DbStatusBarProps) {
  const first = response?.results[0];
  const rowCount =
    first?.kind === "select"
      ? `${first.rows.length.toLocaleString()} row${first.rows.length === 1 ? "" : "s"}`
      : first?.kind === "mutation"
        ? `${first.rows_affected} affected`
        : null;

  const duration =
    durationMs !== null && durationMs !== undefined
      ? formatElapsed(durationMs)
      : null;

  // Seed the relative timestamp only once a run completes, not on mount.
  const [lastRunAt, setLastRunAt] = useState<number | null>(null);
  useEffect(() => {
    if (executionState === "success" || executionState === "error") {
      setLastRunAt(Date.now());
    }
  }, [executionState, response]);

  // 30s tick — finer cadence is visual noise for "X minutes ago".
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    if (lastRunAt === null) return;
    const id = window.setInterval(() => setNowTick(Date.now()), 30_000);
    return () => window.clearInterval(id);
  }, [lastRunAt]);

  const relativeRan =
    lastRunAt !== null ? formatRelativeTime(lastRunAt, nowTick) : null;

  const hint: string | null =
    executionState === "running"
      ? "⌘. to cancel"
      : hasActiveConnection
        ? "⌘↵ to run"
        : null;

  const stateLabel: string | null =
    executionState === "running"
      ? "running"
      : executionState === "error"
        ? "error"
        : executionState === "cancelled"
          ? "cancelled"
          : executionState === "success"
            ? "connected"
            : connection
              ? "connected"
              : null;

  const dotColor: string =
    executionState === "running"
      ? "yellow.400"
      : executionState === "error"
        ? "red.400"
        : executionState === "cancelled"
          ? "orange.400"
          : "green.400";

  const connectionColor = isReadonly ? "orange.400" : "fg.muted";

  const pipe = (
    <Box
      as="span"
      width="1px"
      height="14px"
      bg="border"
      opacity={0.6}
      flexShrink={0}
      mx={1}
    />
  );

  return (
    <Flex
      className="cm-db-statusbar"
      align="center"
      gap={3}
      fontFamily="mono"
      fontSize="xs"
      color="fg.muted"
    >
      <HStack gap={2} align="center" flexShrink={0}>
        <Box boxSize="2" borderRadius="full" bg={dotColor} flexShrink={0} />
        {stateLabel && (
          <Text color="fg" fontWeight="500">
            {stateLabel}
          </Text>
        )}
        {connection && (
          <>
            <Box as="span" color="fg.muted" opacity={0.5} fontWeight="300">
              ·
            </Box>
            <Text color={connectionColor} fontWeight="500">
              {connection}
              {hasActiveConnection && (
                <Text
                  as="span"
                  ml={1}
                  fontSize="2xs"
                  opacity={0.7}
                  letterSpacing="0.04em"
                >
                  {isReadonly ? "(ro)" : "(rw)"}
                </Text>
              )}
            </Text>
          </>
        )}
      </HStack>

      {rowCount && (
        <>
          {pipe}
          <Text>{rowCount}</Text>
        </>
      )}

      <Flex flex={1} />

      {duration && <Text>{duration}</Text>}
      {cached && duration && pipe}
      {cached && (
        <Badge
          size="xs"
          colorPalette="gray"
          variant="subtle"
          fontFamily="mono"
          textTransform="lowercase"
          px={2}
          py={0.5}
          rounded="sm"
        >
          cached
        </Badge>
      )}
      {relativeRan && (
        <>
          {(duration || cached) && pipe}
          <Text opacity={0.8}>ran {relativeRan}</Text>
        </>
      )}
      {hint && (
        <>
          {(relativeRan || duration || cached) && pipe}
          <Text
            color="fg.muted"
            opacity={0.6}
            fontFamily="mono"
            fontSize="2xs"
            letterSpacing="0.04em"
          >
            {hint}
          </Text>
        </>
      )}
      <ExportMenu response={response} query={query} alias={alias} />
    </Flex>
  );
}

interface ExportMenuProps {
  response: DbResponse | null;
  query: string;
  alias: string | undefined;
}

type ExportFormat = "csv" | "json" | "markdown" | "insert";

function ExportMenu({ response, query, alias }: ExportMenuProps) {
  const select = response ? firstSelectResult(response) : null;
  const canExport = select !== null && hasExportableRows(select);

  const buildPayload = useCallback(
    (format: ExportFormat): { text: string; extension: string } | null => {
      if (!select) return null;
      const tableName = inferTableName(query) ?? alias ?? "";
      switch (format) {
        case "csv":
          return { text: toCsv(select), extension: "csv" };
        case "json":
          return { text: toJson(select), extension: "json" };
        case "markdown":
          return { text: toMarkdown(select), extension: "md" };
        case "insert":
          return { text: toInserts(select, tableName), extension: "sql" };
      }
    },
    [select, query, alias],
  );

  const copy = useCallback(
    async (format: ExportFormat) => {
      const payload = buildPayload(format);
      if (!payload) return;
      try {
        await navigator.clipboard.writeText(payload.text);
      } catch {
        /* Clipboard denied. */
      }
    },
    [buildPayload],
  );

  const save = useCallback(
    async (format: ExportFormat) => {
      const payload = buildPayload(format);
      if (!payload) return;
      try {
        const [{ save: saveDialog }, { writeTextFile }] = await Promise.all([
          import("@tauri-apps/plugin-dialog"),
          import("@tauri-apps/plugin-fs"),
        ]);
        const base = alias?.trim() || "query-result";
        const path = await saveDialog({
          defaultPath: `${base}.${payload.extension}`,
          filters: [
            {
              name: payload.extension.toUpperCase(),
              extensions: [payload.extension],
            },
          ],
        });
        if (!path) return;
        await writeTextFile(path, payload.text);
      } catch {
        /* User cancelled or Tauri plugin unavailable. */
      }
    },
    [buildPayload, alias],
  );

  const formatIcon = {
    csv: LuTable2,
    json: LuBraces,
    markdown: LuFileText,
    insert: LuDatabase,
  } as const;

  const formatLabel = {
    csv: "CSV",
    json: "JSON",
    markdown: "Markdown",
    insert: "INSERT",
  } as const;

  const formatExtension = {
    csv: ".csv",
    json: ".json",
    markdown: ".md",
    insert: ".sql",
  } as const;

  const row = (action: "copy" | "save", format: ExportFormat) => {
    const Icon = formatIcon[format];
    const handler = action === "copy" ? copy : save;
    return (
      <Menu.Item
        value={`${action}-${format}`}
        onSelect={() => void handler(format)}
      >
        <Flex align="center" gap={2.5} flex={1} minW={0}>
          <Icon size={13} />
          <Text fontSize="xs" flex={1}>
            {formatLabel[format]}
          </Text>
          {action === "save" && (
            <Text
              fontSize="2xs"
              color="fg.muted"
              opacity={0.7}
              fontFamily="mono"
            >
              {formatExtension[format]}
            </Text>
          )}
        </Flex>
      </Menu.Item>
    );
  };

  return (
    <Menu.Root positioning={{ placement: "bottom-end" }}>
      <Menu.Trigger asChild>
        <IconButton
          size="2xs"
          variant="ghost"
          colorPalette="gray"
          aria-label="Export result"
          title="Export result"
          disabled={!canExport}
        >
          <LuDownload size={13} />
        </IconButton>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            minW="200px"
            py={1}
            css={{
              "& [data-scope='menu'][data-part='item']": {
                paddingTop: "4px",
                paddingBottom: "4px",
                paddingLeft: "10px",
                paddingRight: "10px",
                gap: "8px",
              },
            }}
          >
            <Menu.ItemGroup>
              <Menu.ItemGroupLabel
                fontSize="2xs"
                fontWeight="600"
                color="fg.muted"
                textTransform="uppercase"
                letterSpacing="wider"
                px={2.5}
                py={1.5}
                display="flex"
                alignItems="center"
                gap={1.5}
              >
                <LuClipboard size={10} />
                Copy
              </Menu.ItemGroupLabel>
              {row("copy", "csv")}
              {row("copy", "json")}
              {row("copy", "markdown")}
              {row("copy", "insert")}
            </Menu.ItemGroup>
            <Menu.Separator my={1} />
            <Menu.ItemGroup>
              <Menu.ItemGroupLabel
                fontSize="2xs"
                fontWeight="600"
                color="fg.muted"
                textTransform="uppercase"
                letterSpacing="wider"
                px={2.5}
                py={1.5}
                display="flex"
                alignItems="center"
                gap={1.5}
              >
                <LuHardDriveDownload size={10} />
                Save as file
              </Menu.ItemGroupLabel>
              {row("save", "csv")}
              {row("save", "json")}
              {row("save", "markdown")}
              {row("save", "insert")}
            </Menu.ItemGroup>
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
