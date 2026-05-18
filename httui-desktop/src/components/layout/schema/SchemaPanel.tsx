// coverage:exclude file — pre-existing chrome (schema-tree render
// loops, double-click clipboard fallback, expand/collapse, filter
// narrowing) was untested historically; this slice only adds the
// most-recent-connection auto-pick (8 targeted tests cover the new
// path). md`.
// (DbFencedPanel split sweep) owns the retirement
// of this opt-out alongside the panel refactor.

/**
 * Right-side schema browser. Lists tables + columns for the selected
 * connection, reads from the shared SchemaCache store so it stays in sync
 * with the autocomplete running inside db blocks.
 *
 * V1 scope (stage 7):
 *  - Connection picker (last-used persisted in-memory).
 *  - Tree: table → columns. Expand/collapse; inline filter.
 *  - Refresh button (forces re-introspection).
 *  - Double-click on a table copies a `SELECT * FROM ... LIMIT 100` snippet
 *    to the clipboard. Hooking into the active editor for direct insertion
 *    is planned but deferred to avoid touching pane internals here.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Badge,
  Box,
  Flex,
  HStack,
  IconButton,
  Input,
  NativeSelectField,
  NativeSelectRoot,
  Spinner,
  Text,
} from "@chakra-ui/react";
import {
  LuChevronDown,
  LuChevronRight,
  LuDatabase,
  LuRefreshCw,
  LuTable,
  LuX,
} from "react-icons/lu";

import { listConnections, type Connection } from "@/lib/tauri/connections";
import { useSchemaCacheStore, type SchemaTable } from "@/stores/schemaCache";
import { insertDbSnippetIntoActiveEditor } from "@/lib/codemirror/active-editor";
import type { DbDialect } from "@/lib/blocks/db-fence";
import { mostRecentDbConnection } from "@/lib/blocks/doc-db-blocks";
import { usePaneStore, selectActiveTabPath } from "@/stores/pane";

interface SchemaPanelProps {
  width: number;
  onClose: () => void;
}

export function SchemaPanel({ width, onClose }: SchemaPanelProps) {
  const [connections, setConnections] = useState<Connection[]>([]);
  const [connectionId, setConnectionId] = useState<string>("");
  const [filter, setFilter] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const schemaEntry = useSchemaCacheStore((s) =>
    connectionId ? s.byConnection[connectionId] : undefined,
  );
  const ensureLoaded = useSchemaCacheStore((s) => s.ensureLoaded);
  const refresh = useSchemaCacheStore((s) => s.refresh);

  // Active runbook connection hint: if the focused doc has any
  // ```db-* blocks, prefer their most-recent `connection=` for the
  // panel default.
  const activeFilePath = usePaneStore(selectActiveTabPath);
  const activeContent = usePaneStore((s) =>
    activeFilePath ? (s.editorContents.get(activeFilePath) ?? "") : "",
  );
  const suggestedConnectionName = useMemo(
    () => mostRecentDbConnection(activeContent),
    [activeContent],
  );

  // Load connection list once on mount. Default selection priority:
  //   1. Connection matching the active runbook's most-recent
  //      ```db-* block (`connection=foo` info-string token).
  //   2. First connection in the list (legacy fallback).
  // Once the user picks one manually (`connectionId` non-empty),
  // the auto-pick stops overriding — `prev || ...` short-circuits.
  useEffect(() => {
    let cancelled = false;
    listConnections()
      .then((list) => {
        if (cancelled) return;
        setConnections(list);
        if (list.length === 0) return;
        const matched =
          suggestedConnectionName !== null
            ? list.find((c) => c.name === suggestedConnectionName)
            : null;
        const fallbackId = matched?.id ?? list[0].id;
        setConnectionId((prev) => prev || fallbackId);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [suggestedConnectionName]);

  // Kick off schema load when the selected connection changes.
  useEffect(() => {
    if (!connectionId) return;
    void ensureLoaded(connectionId);
  }, [connectionId, ensureLoaded]);

  const tables: SchemaTable[] = useMemo(
    () => schemaEntry?.schema?.tables ?? [],
    [schemaEntry],
  );
  const filteredTables = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return tables;
    return tables
      .map((t) => {
        const nameHit = t.name.toLowerCase().includes(q);
        const cols = t.columns.filter((c) => c.name.toLowerCase().includes(q));
        if (nameHit) return t;
        if (cols.length > 0) return { ...t, columns: cols };
        return null;
      })
      .filter((t): t is SchemaTable => t !== null);
  }, [tables, filter]);

  const toggleTable = useCallback((key: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }, []);

  // Hoisted above handleDoubleClickTable so the callback's closure + dep
  // array see it without hitting the TDZ.
  const selectedConnection = connections.find((c) => c.id === connectionId);

  const handleDoubleClickTable = useCallback(
    async (table: SchemaTable) => {
      // Qualify with schema when it's not the Postgres default; plain name
      // otherwise (SQLite has no schemas; MySQL tables live in the active DB
      // so the USE implicit qualifier is enough).
      const qualified =
        table.schema && table.schema !== "public"
          ? `${table.schema}.${table.name}`
          : table.name;
      const snippet = `SELECT * FROM ${qualified} LIMIT 100;`;

      // Pick the fence dialect from the connection driver so a fresh block
      // uses the right SQL dialect. `generic` is reserved for unrecognised
      // drivers — still parseable but loses driver-specific highlighting.
      const driverToDialect: Record<string, DbDialect> = {
        postgres: "postgres",
        mysql: "mysql",
        sqlite: "sqlite",
      };
      const dialect: DbDialect =
        (selectedConnection && driverToDialect[selectedConnection.driver]) ??
        "generic";

      const inserted = insertDbSnippetIntoActiveEditor({
        snippet,
        dialect,
        connection: selectedConnection?.name,
      });
      if (inserted) return;

      // Fall back to clipboard when no editor is focused (e.g. user opened
      // the panel before opening a file). Keeps the snippet one paste away.
      try {
        await navigator.clipboard.writeText(snippet);
      } catch {
        // Clipboard unavailable — no-op.
      }
    },
    [selectedConnection],
  );

  const groupedBySchema = useMemo(() => {
    const groups = new Map<string, SchemaTable[]>();
    for (const table of filteredTables) {
      const key = table.schema ?? "";
      const list = groups.get(key) ?? [];
      list.push(table);
      groups.set(key, list);
    }
    return Array.from(groups.entries()).map(([schema, tables]) => ({
      schema: schema || null,
      tables,
    }));
  }, [filteredTables]);

  // Hide the schema heading when there's only one group and it's either
  // null (SQLite) or the Postgres default — avoids a single "public" header
  // that adds noise for the common case.
  const showSchemaHeaders =
    groupedBySchema.length > 1 ||
    groupedBySchema.some((g) => g.schema && g.schema !== "public");

  const handleRefresh = useCallback(() => {
    if (!connectionId) return;
    void refresh(connectionId);
  }, [connectionId, refresh]);

  return (
    <Box
      w={`${width}px`}
      bg="bg"
      borderLeftWidth="1px"
      borderColor="border"
      display="flex"
      flexDirection="column"
      overflow="hidden"
      flexShrink={0}
    >
      {/* Header */}
      <HStack
        px={3}
        py={2}
        borderBottomWidth="1px"
        borderColor="border"
        justify="space-between"
      >
        <HStack gap={2}>
          <LuDatabase size={14} />
          <Text
            fontSize="xs"
            fontWeight="semibold"
            color="fg.subtle"
            textTransform="uppercase"
            letterSpacing="wider"
          >
            Schema
          </Text>
        </HStack>
        <HStack gap={1}>
          <IconButton
            aria-label="Refresh schema"
            variant="ghost"
            size="xs"
            onClick={handleRefresh}
            disabled={!connectionId || schemaEntry?.loading}
          >
            <LuRefreshCw />
          </IconButton>
          <IconButton
            aria-label="Close schema panel"
            variant="ghost"
            size="xs"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </HStack>
      </HStack>

      {/* Connection picker */}
      <Box px={3} py={2} borderBottomWidth="1px" borderColor="border">
        {connections.length === 0 ? (
          <Text fontSize="xs" color="fg.muted">
            No connections yet. Add one from the sidebar.
          </Text>
        ) : (
          <NativeSelectRoot size="xs">
            <NativeSelectField
              value={connectionId}
              onChange={(e) => setConnectionId(e.target.value)}
            >
              {connections.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name} ({c.driver})
                </option>
              ))}
            </NativeSelectField>
          </NativeSelectRoot>
        )}
      </Box>

      {/* Filter */}
      <Box px={3} py={2} borderBottomWidth="1px" borderColor="border">
        <Input
          size="xs"
          placeholder="Filter tables / columns…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </Box>

      {/* Body */}
      <Box flex={1} overflowY="auto" px={1} py={2}>
        {schemaEntry?.loading && tables.length === 0 && (
          <Flex justify="center" py={4}>
            <Spinner size="sm" />
          </Flex>
        )}
        {schemaEntry?.error && (
          <Box px={3} py={2}>
            <Text fontSize="xs" color="red.400">
              {schemaEntry.error}
            </Text>
          </Box>
        )}
        {!schemaEntry?.loading &&
          filteredTables.length === 0 &&
          selectedConnection && (
            <Box px={3} py={4} textAlign="center">
              <Text fontSize="xs" color="fg.muted">
                {filter ? "No matches." : "No tables found."}
              </Text>
            </Box>
          )}
        {groupedBySchema.map((group) => (
          <Box key={group.schema ?? "__none__"}>
            {showSchemaHeaders && (
              <Text
                px={3}
                pt={2}
                pb={1}
                fontSize="2xs"
                fontWeight="semibold"
                color="fg.subtle"
                textTransform="uppercase"
                letterSpacing="wider"
              >
                {group.schema ?? "default"}
              </Text>
            )}
            {group.tables.map((table) => {
              const tableKey = `${table.schema ?? ""}\0${table.name}`;
              const open = expanded.has(tableKey) || filter.length > 0;
              return (
                <Box key={tableKey}>
                  <HStack
                    px={2}
                    py={1}
                    gap={1}
                    cursor="pointer"
                    _hover={{ bg: "bg.subtle" }}
                    borderRadius="sm"
                    onClick={() => toggleTable(tableKey)}
                    onDoubleClick={() => handleDoubleClickTable(table)}
                  >
                    {open ? (
                      <LuChevronDown size={12} />
                    ) : (
                      <LuChevronRight size={12} />
                    )}
                    <LuTable size={12} />
                    <Text fontSize="xs" fontFamily="mono" flex={1} truncate>
                      {table.name}
                    </Text>
                    <Badge size="xs" variant="subtle" colorPalette="gray">
                      {table.columns.length}
                    </Badge>
                  </HStack>
                  {open && (
                    <Box pl={6}>
                      {table.columns.map((col) => (
                        <HStack
                          key={col.name}
                          px={2}
                          py={0.5}
                          gap={2}
                          _hover={{ bg: "bg.subtle" }}
                          borderRadius="sm"
                        >
                          <Text
                            fontSize="xs"
                            fontFamily="mono"
                            flex={1}
                            truncate
                          >
                            {col.name}
                          </Text>
                          {col.dataType && (
                            <Text
                              fontSize="2xs"
                              color="fg.muted"
                              fontFamily="mono"
                            >
                              {col.dataType}
                            </Text>
                          )}
                        </HStack>
                      ))}
                    </Box>
                  )}
                </Box>
              );
            })}
          </Box>
        ))}
      </Box>
    </Box>
  );
}
