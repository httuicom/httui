import { useState, useCallback, useEffect, useMemo } from "react";
import {
  Box,
  Flex,
  Text,
  Button,
  Badge,
  Spinner,
  HStack,
  VStack,
  Collapsible,
  Separator,
} from "@chakra-ui/react";
import {
  LuPlay,
  LuChevronDown,
  LuChevronRight,
  LuDatabase,
  LuMessageSquare,
  LuHardDrive,
  LuBookOpen,
  LuTerminal,
  LuInfo,
} from "react-icons/lu";
import { useColorMode } from "@/components/ui/color-mode";
import CodeMirror from "@uiw/react-codemirror";
import {
  sql,
  SQLite as SQLiteDialect,
  keywordCompletionSource,
} from "@codemirror/lang-sql";
import { autocompletion } from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";
import { ResultTable } from "@/components/blocks/db/ResultTable";
import { queryInternalDb, type AuditQueryResult } from "@/lib/tauri/audit";

const FETCH_SIZE = 80;

// ─── Data model ─────────────────────────────────────────────

interface AuditView {
  id: string;
  label: string;
  description: string;
  insight: string;
  query: string;
  columnDocs: Record<string, string>;
}

interface AuditCategory {
  id: string;
  label: string;
  icon: React.ReactNode;
  description: string;
  views: AuditView[];
}

interface TableDoc {
  name: string;
  description: string;
  columns: { name: string; type: string; description: string }[];
}

// ─── Category definitions ───────────────────────────────────

const CATEGORIES: AuditCategory[] = [
  {
    id: "queries",
    label: "Query Logs",
    icon: <LuDatabase size={13} />,
    description:
      "Every SQL query executed against your databases is logged here. Use this to investigate slow queries, track error patterns, or audit what ran against each connection.",
    views: [
      {
        id: "recent",
        label: "Recent queries",
        description: "The most recent queries executed across all connections.",
        insight:
          "Good starting point to check what happened recently. Look at the status and duration columns to spot problems.",
        query:
          "SELECT connection_id, query, status, duration_ms, created_at FROM query_log ORDER BY created_at DESC",
        columnDocs: {
          connection_id: "ID of the database connection used",
          query: "SQL executed (truncated to 500 characters)",
          status: "success or error",
          duration_ms: "How long the query took in milliseconds",
          created_at: "When the query was executed",
        },
      },
      {
        id: "slow",
        label: "Slow queries (>1s)",
        description: "Queries that took more than 1 second to execute.",
        insight:
          "If you see repeated slow queries on the same connection, it could indicate missing indexes, large result sets, or network latency.",
        query:
          "SELECT connection_id, query, status, duration_ms, created_at FROM query_log WHERE duration_ms > 1000 ORDER BY duration_ms DESC",
        columnDocs: {
          connection_id: "ID of the database connection used",
          query: "SQL executed (truncated to 500 characters)",
          status: "success or error",
          duration_ms: "How long the query took in milliseconds",
          created_at: "When the query was executed",
        },
      },
      {
        id: "failed",
        label: "Failed queries",
        description: "Queries that returned an error from the database.",
        insight:
          "Common causes: syntax errors, missing tables, permission issues, or connection timeouts. Check the query text for clues.",
        query:
          "SELECT connection_id, query, status, duration_ms, created_at FROM query_log WHERE status = 'error' ORDER BY created_at DESC",
        columnDocs: {
          connection_id: "ID of the database connection used",
          query: "SQL that failed (truncated to 500 characters)",
          status: "Always 'error' in this view",
          duration_ms: "Time before the error occurred",
          created_at: "When the failure happened",
        },
      },
      {
        id: "volume",
        label: "Volume by connection",
        description: "Aggregate stats per database connection.",
        insight:
          "Helps identify which connections are most active and which have high error rates. A high error ratio may indicate configuration issues.",
        query:
          "SELECT connection_id, COUNT(*) as total_queries, SUM(CASE WHEN status='error' THEN 1 ELSE 0 END) as errors, ROUND(AVG(duration_ms)) as avg_duration_ms FROM query_log GROUP BY connection_id ORDER BY total_queries DESC",
        columnDocs: {
          connection_id: "Database connection identifier",
          total_queries: "Total number of queries executed",
          errors: "How many queries failed",
          avg_duration_ms: "Average execution time in milliseconds",
        },
      },
    ],
  },
  {
    id: "chat",
    label: "Chat & Usage",
    icon: <LuMessageSquare size={13} />,
    description:
      "Tracks your AI chat sessions, token consumption, and tool usage. Useful for understanding usage patterns and managing costs.",
    views: [
      {
        id: "sessions",
        label: "Chat sessions",
        description: "All active (non-archived) chat sessions.",
        insight:
          "Each session represents a conversation with Claude. The CWD column shows the working directory that was active during the session.",
        query:
          "SELECT id, title, cwd, datetime(created_at, 'unixepoch') as created, datetime(updated_at, 'unixepoch') as updated FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC",
        columnDocs: {
          id: "Unique session identifier",
          title: "Session title (auto-generated or custom)",
          cwd: "Working directory during the session",
          created: "When the session started",
          updated: "Last activity in this session",
        },
      },
      {
        id: "tokens",
        label: "Token usage (30 days)",
        description:
          "Daily token consumption aggregated over the last 30 days.",
        insight:
          "Input tokens are what you send, output tokens are Claude's responses. Cache tokens reduce costs by reusing prior context. High cache ratios are good.",
        query:
          "SELECT date, SUM(input_tokens) as input_tokens, SUM(output_tokens) as output_tokens, SUM(cache_read_tokens) as cache_tokens FROM usage_stats GROUP BY date ORDER BY date DESC LIMIT 30",
        columnDocs: {
          date: "Day (YYYY-MM-DD)",
          input_tokens: "Tokens sent to Claude",
          output_tokens: "Tokens received from Claude",
          cache_tokens: "Tokens served from prompt cache (saves cost)",
        },
      },
      {
        id: "tools",
        label: "Tool call frequency",
        description: "How often each tool was used across all sessions.",
        insight:
          "Shows which tools Claude uses most. High error counts on a tool may indicate permissions issues or tool configuration problems.",
        query:
          "SELECT tool_name, COUNT(*) as total_calls, SUM(is_error) as errors FROM tool_calls GROUP BY tool_name ORDER BY total_calls DESC",
        columnDocs: {
          tool_name: "Name of the tool (e.g., read_note, update_note)",
          total_calls: "Number of times this tool was invoked",
          errors: "Number of invocations that returned an error",
        },
      },
    ],
  },
  {
    id: "storage",
    label: "Storage",
    icon: <LuHardDrive size={13} />,
    description:
      "Overview of cached data, permissions rules, and internal database size. Helps you understand what the app stores locally.",
    views: [
      {
        id: "cache",
        label: "Cached block results",
        description: "Results cached from previous block executions.",
        insight:
          "The cache avoids re-executing identical queries. Results are invalidated when the block content, environment, or connection changes.",
        query:
          "SELECT file_path, status, total_rows, executed_at, elapsed_ms FROM block_results ORDER BY executed_at DESC",
        columnDocs: {
          file_path: "Note file that contains the block",
          status: "Whether the last execution succeeded or failed",
          total_rows: "Number of rows returned (for SELECT queries)",
          executed_at: "When the block was last executed",
          elapsed_ms: "Execution time in milliseconds",
        },
      },
      {
        id: "permissions",
        label: "Active permissions",
        description: "Tool permission rules saved from chat interactions.",
        insight:
          "These rules determine which tools Claude can use without asking. 'always' rules persist across sessions; 'session' rules expire when the session ends.",
        query:
          "SELECT tool_name, path_pattern, scope, behavior, datetime(created_at, 'unixepoch') as created FROM tool_permissions ORDER BY created_at DESC",
        columnDocs: {
          tool_name: "Tool this rule applies to",
          path_pattern: "File path pattern (if applicable)",
          scope: "'always' persists forever, 'session' expires",
          behavior: "'allow' or 'deny'",
          created: "When this rule was created",
        },
      },
      {
        id: "dbsize",
        label: "Database size",
        description: "Row counts for each internal table.",
        insight:
          "Shows how much data the app stores locally. The query_log table is automatically pruned (max 50k rows, 30 day retention).",
        query:
          "SELECT 'query_log' as table_name, COUNT(*) as rows FROM query_log UNION ALL SELECT 'block_results', COUNT(*) FROM block_results UNION ALL SELECT 'messages', COUNT(*) FROM messages UNION ALL SELECT 'sessions', COUNT(*) FROM sessions UNION ALL SELECT 'tool_calls', COUNT(*) FROM tool_calls UNION ALL SELECT 'usage_stats', COUNT(*) FROM usage_stats UNION ALL SELECT 'connections', COUNT(*) FROM connections UNION ALL SELECT 'environments', COUNT(*) FROM environments",
        columnDocs: {
          table_name: "Internal table name",
          rows: "Number of rows stored",
        },
      },
    ],
  },
];

// ─── Schema documentation ───────────────────────────────────

const SCHEMA_DOCS: TableDoc[] = [
  {
    name: "query_log",
    description:
      "Records every SQL query executed via DB blocks. Automatically pruned: max 50,000 entries, 30-day retention.",
    columns: [
      {
        name: "id",
        type: "INTEGER",
        description: "Auto-incrementing primary key",
      },
      {
        name: "connection_id",
        type: "TEXT",
        description: "Which database connection was used",
      },
      {
        name: "query",
        type: "TEXT",
        description: "The SQL query (truncated to 500 characters for storage)",
      },
      { name: "status", type: "TEXT", description: "'success' or 'error'" },
      {
        name: "duration_ms",
        type: "INTEGER",
        description: "Execution time in milliseconds",
      },
      {
        name: "created_at",
        type: "TEXT",
        description: "ISO timestamp of when the query ran",
      },
    ],
  },
  {
    name: "connections",
    description:
      "Database connection configurations. Passwords are stored in the OS keychain, not here.",
    columns: [
      {
        name: "id",
        type: "TEXT",
        description: "Unique connection identifier (UUID)",
      },
      {
        name: "name",
        type: "TEXT",
        description: "User-assigned name (unique)",
      },
      {
        name: "driver",
        type: "TEXT",
        description: "Database type: postgres, mysql, or sqlite",
      },
      { name: "host", type: "TEXT", description: "Server hostname or IP" },
      { name: "port", type: "INTEGER", description: "Server port number" },
      {
        name: "database_name",
        type: "TEXT",
        description: "Database or file path (SQLite)",
      },
      {
        name: "username",
        type: "TEXT",
        description: "Authentication username",
      },
      {
        name: "password",
        type: "TEXT",
        description:
          "Always '__KEYCHAIN__' sentinel — real value in OS keychain",
      },
      {
        name: "ssl_mode",
        type: "TEXT",
        description: "SSL mode: prefer, require, or disable",
      },
      {
        name: "timeout_ms",
        type: "INTEGER",
        description: "Connection timeout in milliseconds",
      },
      {
        name: "query_timeout_ms",
        type: "INTEGER",
        description: "Default query timeout in milliseconds",
      },
      {
        name: "max_pool_size",
        type: "INTEGER",
        description: "Maximum concurrent connections in the pool",
      },
    ],
  },
  {
    name: "environments",
    description:
      "Environment groupings for organizing variables (e.g., Development, Staging, Production).",
    columns: [
      {
        name: "id",
        type: "TEXT",
        description: "Unique environment identifier",
      },
      { name: "name", type: "TEXT", description: "Environment name (unique)" },
      {
        name: "is_active",
        type: "INTEGER",
        description: "1 if this is the currently active environment",
      },
    ],
  },
  {
    name: "env_variables",
    description:
      "Key-value pairs within environments. Secret values are stored in the OS keychain.",
    columns: [
      { name: "id", type: "TEXT", description: "Unique variable identifier" },
      {
        name: "environment_id",
        type: "TEXT",
        description: "Which environment this belongs to",
      },
      {
        name: "key",
        type: "TEXT",
        description: "Variable name (used as {{KEY}} in blocks)",
      },
      {
        name: "value",
        type: "TEXT",
        description: "Variable value (or '__KEYCHAIN__' for secrets)",
      },
      {
        name: "is_secret",
        type: "INTEGER",
        description: "1 if encrypted via OS keychain",
      },
    ],
  },
  {
    name: "sessions",
    description:
      "AI chat sessions. Each session is a conversation with Claude.",
    columns: [
      { name: "id", type: "TEXT", description: "Unique session identifier" },
      {
        name: "claude_session_id",
        type: "TEXT",
        description: "Claude API session ID (for resume)",
      },
      { name: "title", type: "TEXT", description: "Session title" },
      {
        name: "cwd",
        type: "TEXT",
        description: "Working directory during the session",
      },
      {
        name: "created_at",
        type: "INTEGER",
        description: "Unix timestamp of creation",
      },
      {
        name: "updated_at",
        type: "INTEGER",
        description: "Unix timestamp of last activity",
      },
      {
        name: "archived_at",
        type: "INTEGER",
        description: "Unix timestamp when archived (NULL if active)",
      },
    ],
  },
  {
    name: "messages",
    description: "Individual messages within chat sessions.",
    columns: [
      { name: "id", type: "TEXT", description: "Unique message identifier" },
      {
        name: "session_id",
        type: "TEXT",
        description: "Which session this belongs to",
      },
      { name: "role", type: "TEXT", description: "'user' or 'assistant'" },
      {
        name: "turn_index",
        type: "INTEGER",
        description: "Order within the session",
      },
      {
        name: "content_json",
        type: "TEXT",
        description: "Message content as JSON",
      },
      {
        name: "tokens_in",
        type: "INTEGER",
        description: "Input tokens for this message",
      },
      {
        name: "tokens_out",
        type: "INTEGER",
        description: "Output tokens for this message",
      },
    ],
  },
  {
    name: "tool_calls",
    description: "Records of every tool invocation during chat sessions.",
    columns: [
      { name: "id", type: "TEXT", description: "Unique call identifier" },
      {
        name: "message_id",
        type: "TEXT",
        description: "Which message triggered this tool call",
      },
      {
        name: "tool_name",
        type: "TEXT",
        description: "Name of the tool (e.g., read_note, update_note)",
      },
      {
        name: "input_json",
        type: "TEXT",
        description: "Arguments passed to the tool",
      },
      {
        name: "result_json",
        type: "TEXT",
        description: "Tool execution result",
      },
      {
        name: "is_error",
        type: "INTEGER",
        description: "1 if the tool returned an error",
      },
    ],
  },
  {
    name: "usage_stats",
    description:
      "Aggregated token usage by day and session. Used for the usage panel chart.",
    columns: [
      { name: "date", type: "TEXT", description: "Day (YYYY-MM-DD)" },
      {
        name: "session_id",
        type: "TEXT",
        description: "Session (NULL for daily aggregate)",
      },
      {
        name: "input_tokens",
        type: "INTEGER",
        description: "Total input tokens consumed",
      },
      {
        name: "output_tokens",
        type: "INTEGER",
        description: "Total output tokens generated",
      },
      {
        name: "cache_read_tokens",
        type: "INTEGER",
        description: "Tokens served from prompt cache",
      },
    ],
  },
  {
    name: "tool_permissions",
    description:
      "Persisted permission rules from chat interactions. Controls which tools Claude can use without prompting.",
    columns: [
      { name: "id", type: "TEXT", description: "Unique rule identifier" },
      {
        name: "tool_name",
        type: "TEXT",
        description: "Tool this rule applies to",
      },
      {
        name: "path_pattern",
        type: "TEXT",
        description: "File path pattern (glob)",
      },
      {
        name: "scope",
        type: "TEXT",
        description: "'always' persists forever, 'session' expires",
      },
      { name: "behavior", type: "TEXT", description: "'allow' or 'deny'" },
    ],
  },
  {
    name: "block_results",
    description:
      "Cached execution results for blocks. Keyed by file path + content hash.",
    columns: [
      {
        name: "file_path",
        type: "TEXT",
        description: "Note file containing the block",
      },
      {
        name: "block_hash",
        type: "TEXT",
        description: "SHA-256 of content + env + connection",
      },
      { name: "status", type: "TEXT", description: "'success' or 'error'" },
      {
        name: "response",
        type: "TEXT",
        description: "Serialized block result (JSON)",
      },
      {
        name: "total_rows",
        type: "INTEGER",
        description: "Row count for SELECT results",
      },
      {
        name: "executed_at",
        type: "TEXT",
        description: "When this result was cached",
      },
      {
        name: "elapsed_ms",
        type: "INTEGER",
        description: "Execution time in milliseconds",
      },
    ],
  },
];

// ─── CodeMirror themes ──────────────────────────────────────

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

const cmBorderTheme = EditorView.theme({
  "&": {
    borderRadius: "6px",
    border: "1px solid var(--chakra-colors-border)",
  },
  "&.cm-focused": {
    outline: "none",
    borderColor: "var(--chakra-colors-border)",
  },
});

// ─── Helpers ────────────────────────────────────────────────

function rowsToRecords(
  columns: { name: string; type: string }[],
  rows: unknown[][],
): Record<string, string | number | boolean | null>[] {
  return rows.map((row) => {
    const record: Record<string, string | number | boolean | null> = {};
    columns.forEach((col, i) => {
      record[col.name] = row[i] as string | number | boolean | null;
    });
    return record;
  });
}

// ─── Sub-components ─────────────────────────────────────────

function ViewRunner({ view }: { view: AuditView }) {
  const { colorMode } = useColorMode();
  const cmTheme = colorMode === "dark" ? "dark" : "light";

  const [result, setResult] = useState<AuditQueryResult | null>(null);
  const [accumulatedRows, setAccumulatedRows] = useState<
    Record<string, string | number | boolean | null>[]
  >([]);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const [editorValue, setEditorValue] = useState(view.query);

  const sqlExtensions = useMemo(
    () => [
      sql({ dialect: SQLiteDialect }),
      autocompletion({ override: [keywordCompletionSource(SQLiteDialect)] }),
      EditorView.lineWrapping,
      cmTransparentBg,
      cmBorderTheme,
    ],
    [],
  );

  // Auto-load on mount
  useEffect(() => {
    runQuery(view.query);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.id]);

  // Reset when view changes
  useEffect(() => {
    setEditorValue(view.query);
    setCustomOpen(false);
  }, [view.id, view.query]);

  const runQuery = useCallback(async (queryStr: string) => {
    const trimmed = queryStr.trim();
    if (!trimmed) return;

    setLoading(true);
    setError(null);
    setResult(null);
    setAccumulatedRows([]);
    setHasMore(false);

    const start = performance.now();
    try {
      const res = await queryInternalDb(trimmed, 0, FETCH_SIZE);
      const elapsed = Math.round(performance.now() - start);
      setDurationMs(elapsed);
      setResult(res);
      setAccumulatedRows(rowsToRecords(res.columns, res.rows));
      setHasMore(res.has_more);
    } catch (err) {
      setError(String(err));
      setDurationMs(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadMore = useCallback(async () => {
    if (!result || loadingMore || !hasMore) return;
    const queryStr = customOpen ? editorValue.trim() : view.query;

    setLoadingMore(true);
    try {
      const res = await queryInternalDb(
        queryStr,
        accumulatedRows.length,
        FETCH_SIZE,
      );
      setAccumulatedRows((prev) => [
        ...prev,
        ...rowsToRecords(res.columns, res.rows),
      ]);
      setHasMore(res.has_more);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoadingMore(false);
    }
  }, [
    result,
    loadingMore,
    hasMore,
    customOpen,
    editorValue,
    view.query,
    accumulatedRows.length,
  ]);

  return (
    <Flex direction="column" gap={3}>
      {/* View description */}
      <Box>
        <Text fontSize="sm" fontWeight="medium">
          {view.label}
        </Text>
        <Text fontSize="xs" color="fg.muted" mt={0.5}>
          {view.description}
        </Text>
      </Box>

      {/* Insight box */}
      <Flex
        gap={2}
        px={3}
        py={2}
        borderRadius="md"
        bg="blue.subtle"
        borderWidth="1px"
        borderColor="blue.muted"
        align="flex-start"
      >
        <Box mt={0.5} flexShrink={0}>
          <LuInfo size={12} />
        </Box>
        <Text fontSize="xs" color="fg">
          {view.insight}
        </Text>
      </Flex>

      {/* Status bar */}
      <HStack gap={2} fontSize="xs">
        {loading && <Spinner size="xs" />}
        {durationMs !== null && !loading && (
          <Badge variant="subtle" size="sm" colorPalette="gray">
            {durationMs}ms
          </Badge>
        )}
        {result && !loading && (
          <Text color="fg.muted">
            {accumulatedRows.length} row
            {accumulatedRows.length !== 1 ? "s" : ""}
            {hasMore ? "+" : ""}
          </Text>
        )}
      </HStack>

      {/* Error */}
      {error && (
        <Box
          px={3}
          py={2}
          borderRadius="md"
          bg="red.subtle"
          borderWidth="1px"
          borderColor="red.muted"
        >
          <Text fontSize="xs" color="red.fg">
            {error}
          </Text>
        </Box>
      )}

      {/* Results table */}
      {result && result.columns.length > 0 && !loading && (
        <Box overflow="hidden">
          <ResultTable
            columns={result.columns}
            rows={accumulatedRows}
            durationMs={durationMs}
            hasMore={hasMore}
            onLoadMore={loadMore}
          />
        </Box>
      )}

      {/* Column legend */}
      {result && result.columns.length > 0 && !loading && (
        <Box
          borderWidth="1px"
          borderColor="border"
          borderRadius="md"
          px={3}
          py={2}
        >
          <Text fontSize="xs" fontWeight="medium" mb={1.5}>
            Column reference
          </Text>
          <VStack gap={1} align="stretch">
            {result.columns.map((col) => (
              <Flex key={col.name} gap={2} fontSize="xs">
                <Text
                  fontFamily="mono"
                  fontWeight="medium"
                  color="purple.fg"
                  flexShrink={0}
                >
                  {col.name}
                </Text>
                <Text color="fg.muted">
                  {view.columnDocs[col.name] ?? col.type}
                </Text>
              </Flex>
            ))}
          </VStack>
        </Box>
      )}

      {/* Empty state */}
      {result && result.columns.length === 0 && !error && !loading && (
        <Flex
          justify="center"
          align="center"
          py={6}
          color="fg.muted"
          fontSize="xs"
        >
          No results
        </Flex>
      )}

      {/* Advanced: custom SQL */}
      <Separator />
      <Box>
        <Flex
          align="center"
          gap={1}
          cursor="pointer"
          onClick={() => setCustomOpen((v) => !v)}
          fontSize="xs"
          color="fg.muted"
          _hover={{ color: "fg" }}
        >
          {customOpen ? (
            <LuChevronDown size={12} />
          ) : (
            <LuChevronRight size={12} />
          )}
          <LuTerminal size={12} />
          <Text>Custom query</Text>
        </Flex>
        <Collapsible.Root open={customOpen}>
          <Collapsible.Content>
            <Box mt={2}>
              <CodeMirror
                value={editorValue}
                onChange={setEditorValue}
                extensions={sqlExtensions}
                basicSetup={{
                  lineNumbers: true,
                  foldGutter: false,
                  autocompletion: false,
                }}
                theme={cmTheme}
                height="auto"
                minHeight="60px"
                maxHeight="200px"
                style={{ fontSize: "12px" }}
              />
              <Button
                size="xs"
                colorPalette="purple"
                mt={2}
                onClick={() => runQuery(editorValue)}
                disabled={loading || !editorValue.trim()}
              >
                <LuPlay size={12} />
                <Text ml={1}>Run</Text>
              </Button>
            </Box>
          </Collapsible.Content>
        </Collapsible.Root>
      </Box>
    </Flex>
  );
}

function SchemaExplorer() {
  const [expandedTable, setExpandedTable] = useState<string | null>(null);

  return (
    <Flex direction="column" gap={3}>
      <Text fontSize="xs" color="fg.muted">
        Complete reference of all internal tables. Click a table to see its
        columns and what each one stores.
      </Text>

      <VStack gap={1} align="stretch">
        {SCHEMA_DOCS.map((table) => {
          const isOpen = expandedTable === table.name;
          return (
            <Box
              key={table.name}
              borderWidth="1px"
              borderColor="border"
              borderRadius="md"
              overflow="hidden"
            >
              <Flex
                align="center"
                gap={2}
                px={3}
                py={2}
                cursor="pointer"
                _hover={{ bg: "bg.subtle" }}
                onClick={() => setExpandedTable(isOpen ? null : table.name)}
              >
                {isOpen ? (
                  <LuChevronDown size={12} />
                ) : (
                  <LuChevronRight size={12} />
                )}
                <Text fontSize="xs" fontFamily="mono" fontWeight="medium">
                  {table.name}
                </Text>
                <Text fontSize="xs" color="fg.muted" flex={1}>
                  {table.description.split(".")[0]}
                </Text>
                <Badge size="xs" variant="subtle">
                  {table.columns.length} cols
                </Badge>
              </Flex>

              {isOpen && (
                <Box px={3} pb={3}>
                  <Text fontSize="xs" color="fg.muted" mb={2}>
                    {table.description}
                  </Text>
                  <VStack gap={0} align="stretch">
                    {table.columns.map((col) => (
                      <Flex
                        key={col.name}
                        gap={2}
                        py={1}
                        fontSize="xs"
                        borderTopWidth="1px"
                        borderColor="border"
                      >
                        <Text
                          fontFamily="mono"
                          fontWeight="medium"
                          color="purple.fg"
                          w="140px"
                          flexShrink={0}
                        >
                          {col.name}
                        </Text>
                        <Badge size="xs" variant="outline" flexShrink={0}>
                          {col.type}
                        </Badge>
                        <Text color="fg.muted" flex={1}>
                          {col.description}
                        </Text>
                      </Flex>
                    ))}
                  </VStack>
                </Box>
              )}
            </Box>
          );
        })}
      </VStack>
    </Flex>
  );
}

// ─── Main component ─────────────────────────────────────────

export function AuditSection() {
  const [activeCategoryId, setActiveCategoryId] = useState(CATEGORIES[0].id);
  const [activeViewIndex, setActiveViewIndex] = useState(0);
  const [showSchema, setShowSchema] = useState(false);

  const activeCategory =
    CATEGORIES.find((c) => c.id === activeCategoryId) ?? CATEGORIES[0];
  const activeView = activeCategory.views[activeViewIndex];

  const handleCategoryChange = useCallback((catId: string) => {
    setActiveCategoryId(catId);
    setActiveViewIndex(0);
    setShowSchema(false);
  }, []);

  return (
    <Flex direction="column" gap={3} h="100%">
      {/* Category tabs */}
      <HStack gap={1} flexWrap="wrap">
        {CATEGORIES.map((cat) => (
          <Button
            key={cat.id}
            size="xs"
            variant={
              activeCategoryId === cat.id && !showSchema ? "subtle" : "ghost"
            }
            onClick={() => handleCategoryChange(cat.id)}
          >
            {cat.icon}
            <Text ml={1}>{cat.label}</Text>
          </Button>
        ))}
        <Button
          size="xs"
          variant={showSchema ? "subtle" : "ghost"}
          onClick={() => setShowSchema(true)}
        >
          <LuBookOpen size={13} />
          <Text ml={1}>Schema</Text>
        </Button>
      </HStack>

      {/* Schema explorer */}
      {showSchema && <SchemaExplorer />}

      {/* Category content */}
      {!showSchema && (
        <>
          {/* Category description */}
          <Text fontSize="xs" color="fg.muted">
            {activeCategory.description}
          </Text>

          {/* View selector */}
          {activeCategory.views.length > 1 && (
            <HStack gap={1}>
              {activeCategory.views.map((view, i) => (
                <Button
                  key={view.id}
                  size="xs"
                  variant={activeViewIndex === i ? "outline" : "ghost"}
                  onClick={() => setActiveViewIndex(i)}
                  fontSize="xs"
                >
                  {view.label}
                </Button>
              ))}
            </HStack>
          )}

          {/* Active view */}
          {activeView && <ViewRunner key={activeView.id} view={activeView} />}
        </>
      )}
    </Flex>
  );
}
