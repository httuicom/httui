// Detail panel schema preview: collapsible table tree with a "Hot tables" section
// (top-N by hit count from `block_run_history`).
// Pure presentational — loader/refresh wiring lives in the consumer.

import { useState } from "react";
import { Box, Flex, HStack, Stack, Text, chakra } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";
import type { ConnectionSchema, SchemaTable } from "@/stores/schemaCache";

const TableHeader = chakra("button");

export interface HotTableEntry {
  /** `${schema}.${name}` for PG/MySQL, just `name` for SQLite. */
  tableName: string;
  hits: number;
}

export interface ConnectionDetailSchemaPreviewProps {
  schema: ConnectionSchema | null;
  loading: boolean;
  error: string | null;
  hotTables: HotTableEntry[];
  /** Bypass the SQLite cache and force re-introspection. */
  onRefresh?: () => void;
}

export const HOT_TABLES_LIMIT = 5;

export function ConnectionDetailSchemaPreview({
  schema,
  loading,
  error,
  hotTables,
  onRefresh,
}: ConnectionDetailSchemaPreviewProps) {
  return (
    <Stack data-testid="connection-schema-preview" gap={3} align="stretch">
      <Flex justify="space-between" align="center">
        <Text
          fontFamily="mono"
          fontSize="11px"
          fontWeight="bold"
          letterSpacing="0.08em"
          textTransform="uppercase"
          color="fg.muted"
        >
          Schema
        </Text>
        {onRefresh && (
          <Btn
            variant="ghost"
            data-testid="schema-refresh"
            onClick={onRefresh}
            disabled={loading}
          >
            {loading ? "Loading…" : "Refresh"}
          </Btn>
        )}
      </Flex>

      {error !== null && (
        <Text data-testid="schema-error" fontSize="11px" color="red.fg">
          {error}
        </Text>
      )}

      {loading && schema === null && (
        <Text data-testid="schema-loading" fontSize="11px" color="fg.subtle">
          Loading schema…
        </Text>
      )}

      {!loading && error === null && schema === null && (
        <Text data-testid="schema-empty" fontSize="11px" color="fg.subtle">
          Pick "Refresh" to introspect this connection's schema.
        </Text>
      )}

      <HotTablesSection hotTables={hotTables} />

      {schema !== null && schema.tables.length > 0 && (
        <Stack gap={1} align="stretch" data-testid="schema-tables">
          <Text
            fontSize="11px"
            color="fg.subtle"
            fontFamily="mono"
            data-testid="schema-tables-count"
          >
            All tables ({schema.tables.length})
          </Text>
          {schema.tables.map((t) => (
            <TableNode key={`${t.schema ?? ""}\0${t.name}`} table={t} />
          ))}
        </Stack>
      )}
    </Stack>
  );
}

function HotTablesSection({ hotTables }: { hotTables: HotTableEntry[] }) {
  if (hotTables.length === 0) return null;
  const top = hotTables.slice(0, HOT_TABLES_LIMIT);
  return (
    <Stack data-testid="schema-hot-tables" gap={1} align="stretch">
      <Text fontSize="11px" color="fg.subtle" fontFamily="mono">
        Hot tables (most queried)
      </Text>
      {top.map((t) => (
        <Flex
          key={t.tableName}
          data-testid={`schema-hot-row-${t.tableName}`}
          align="baseline"
          justify="space-between"
          gap={2}
          px={2}
          py="4px"
          borderRadius="4px"
          bg="bg.muted"
        >
          <Text fontFamily="mono" fontSize="11px" color="fg" truncate>
            {t.tableName}
          </Text>
          <Text
            fontFamily="mono"
            fontSize="11px"
            color="fg.subtle"
            flexShrink={0}
          >
            {t.hits} hits
          </Text>
        </Flex>
      ))}
    </Stack>
  );
}

function TableNode({ table }: { table: SchemaTable }) {
  const [open, setOpen] = useState(false);
  const display = table.schema ? `${table.schema}.${table.name}` : table.name;
  return (
    <Box data-testid={`schema-table-${display}`} borderRadius="4px">
      <TableHeader
        type="button"
        data-testid={`schema-table-toggle-${display}`}
        onClick={() => setOpen((v) => !v)}
        display="flex"
        w="full"
        alignItems="center"
        gap={2}
        px={2}
        py="6px"
        bg="transparent"
        cursor="pointer"
        textAlign="left"
        border="none"
        _hover={{ bg: "bg.muted" }}
      >
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          minW="10px"
        >
          {open ? "▾" : "▸"}
        </Text>
        <Text
          as="span"
          flex={1}
          fontFamily="mono"
          fontSize="12px"
          color="fg"
          truncate
        >
          {display}
        </Text>
        <Text as="span" fontFamily="mono" fontSize="10px" color="fg.subtle">
          {table.columns.length} cols
        </Text>
      </TableHeader>
      {open && (
        <Stack
          data-testid={`schema-table-cols-${display}`}
          gap={0.5}
          align="stretch"
          pl="22px"
          pr={2}
          pb={1}
        >
          {table.columns.map((c) => (
            <HStack key={c.name} gap={2} fontFamily="mono" fontSize="11px">
              <Text color="fg" flex={1} truncate>
                {c.name}
              </Text>
              <Text color="fg.subtle" flexShrink={0}>
                {c.dataType ?? "—"}
              </Text>
            </HStack>
          ))}
        </Stack>
      )}
    </Box>
  );
}
