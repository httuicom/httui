import { Box, Flex, HStack, IconButton, Table } from "@chakra-ui/react";
import { Fragment, useCallback, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { LuCopy, LuX } from "react-icons/lu";
import { formatElapsed } from "@/lib/format/time";
import type { CellValue } from "./types";

interface ResultTableProps {
  columns: { name: string; type: string }[];
  rows: Record<string, CellValue>[];
  durationMs?: number | null;
  hasMore: boolean;
  /** Called when the user clicks Load more. May return a Promise, which
   *  the table awaits to drive its own local loading state (so the parent
   *  does not need to re-render just to show the spinner). */
  onLoadMore?: () => Promise<void> | void;
}

function formatCellValue(value: CellValue): {
  text: string;
  isNull: boolean;
} {
  if (value === null) return { text: "NULL", isNull: true };
  if (typeof value === "boolean")
    return { text: value.toString(), isNull: false };
  if (typeof value === "object")
    return { text: JSON.stringify(value), isNull: false };
  return { text: String(value), isNull: false };
}

function tryParseJson(value: CellValue): { parsed: unknown; isJson: boolean } {
  if (typeof value !== "string") return { parsed: value, isJson: false };
  const trimmed = value.trim();
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return { parsed: JSON.parse(trimmed), isJson: true };
    } catch {
      return { parsed: value, isJson: false };
    }
  }
  return { parsed: value, isJson: false };
}

function isNumericType(type: string): boolean {
  const t = type.toUpperCase();
  return (
    t.includes("INT") ||
    t.includes("FLOAT") ||
    t.includes("DECIMAL") ||
    t.includes("NUMERIC") ||
    t.includes("REAL") ||
    t.includes("DOUBLE")
  );
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <Box
      as="pre"
      m={0}
      p={2}
      bg="bg"
      border="1px solid"
      borderColor="border"
      rounded="sm"
      fontSize="xs"
      fontFamily="mono"
      whiteSpace="pre-wrap"
      wordBreak="break-all"
      maxH="200px"
      overflowY="auto"
      overscrollBehavior="contain"
    >
      {JSON.stringify(value, null, 2)}
    </Box>
  );
}

function DetailValue({ value }: { value: CellValue }) {
  if (value === null) {
    return (
      <Box as="span" fontStyle="italic" color="fg.muted" opacity={0.6}>
        NULL
      </Box>
    );
  }
  if (typeof value === "object") {
    return <JsonBlock value={value} />;
  }
  const { parsed, isJson } = tryParseJson(value);
  if (isJson) {
    return <JsonBlock value={parsed} />;
  }
  return (
    <Box as="span" wordBreak="break-all">
      {String(value)}
    </Box>
  );
}

const ROW_HEIGHT = 32;
const EXPANDED_OVERHEAD = 180;

// CSS as a static constant — Emotion will not re-compute on every render.
// Uses semantic Chakra tokens via `var(--chakra-colors-*)` so the table
// adapts to light/dark themes without branching here.
//
// Design intent: match the mockup — generous row height (~42px), comfortable
// horizontal padding (24px), subtle header tint, no zebra (uniform rows so
// timestamps line up visually like a proper data grid).
const tableCss = {
  userSelect: "text",
  cursor: "text",
  borderCollapse: "separate",
  borderSpacing: 0,
  "& th, & td": {
    whiteSpace: "nowrap",
    maxWidth: "360px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    padding: "6px 16px",
    lineHeight: "18px",
    borderBottom:
      "1px solid color-mix(in srgb, var(--chakra-colors-border) 35%, transparent)",
    borderRight: "none",
  },
  "& thead th": {
    background: "color-mix(in srgb, var(--chakra-colors-fg) 2%, transparent)",
    color: "var(--chakra-colors-fg)",
    fontSize: "var(--chakra-font-sizes-xs)",
    textTransform: "none",
    letterSpacing: "0",
    fontWeight: 600,
    paddingTop: "8px",
    paddingBottom: "8px",
    borderBottom:
      "1px solid color-mix(in srgb, var(--chakra-colors-border) 70%, transparent)",
  },
  "& tbody tr:hover:not([data-selected='true']) td": {
    background: "color-mix(in srgb, var(--chakra-colors-fg) 4%, transparent)",
  },
  "& tbody tr[data-selected='true'] td": {
    background:
      "color-mix(in srgb, var(--chakra-colors-brand-500) 10%, transparent) !important",
  },
  "& tbody tr[data-selected='true'] td:first-of-type": {
    boxShadow: "inset 2px 0 0 var(--chakra-colors-brand-500)",
  },
  "& .cell-numeric": {
    textAlign: "right",
    fontVariantNumeric: "tabular-nums",
  },
  "& .cell-null": {
    fontStyle: "italic",
    color: "var(--chakra-colors-fg-muted)",
    opacity: 0.5,
  },
  "& .type-label": {
    fontSize: "var(--chakra-font-sizes-xs)",
    textTransform: "lowercase",
    letterSpacing: "0",
    color: "var(--chakra-colors-fg-muted)",
    opacity: 0.55,
    fontWeight: 400,
    paddingLeft: "8px",
  },
} as const;

export function ResultTable({
  columns,
  rows,
  durationMs,
  hasMore,
  onLoadMore,
}: ResultTableProps) {
  const [expandedRow, setExpandedRow] = useState<number | null>(null);
  const [copiedRow, setCopiedRow] = useState<number | null>(null);
  // Local loading state — flips only inside this component; the parent
  // (DbFencedPanel) keeps its dedup guard in a ref to avoid cascade renders.
  const [loadingMore, setLoadingMore] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  const triggerLoadMore = useCallback(async () => {
    if (!onLoadMore || loadingMore) return;
    setLoadingMore(true);
    try {
      await onLoadMore();
    } finally {
      setLoadingMore(false);
    }
  }, [onLoadMore, loadingMore]);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el || loadingMore || !hasMore || !onLoadMore) return;
    const { scrollTop, scrollHeight, clientHeight } = el;
    if (scrollHeight - scrollTop - clientHeight < 100) {
      void triggerLoadMore();
    }
  }, [loadingMore, hasMore, onLoadMore, triggerLoadMore]);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (index) =>
      expandedRow === index ? ROW_HEIGHT + EXPANDED_OVERHEAD : ROW_HEIGHT,
    overscan: 10,
  });

  const copyRowAsJson = useCallback(
    (rowIdx: number) => {
      const row = rows[rowIdx];
      if (!row) return;
      const payload: Record<string, CellValue> = {};
      for (const col of columns) {
        payload[col.name] = row[col.name] ?? null;
      }
      const text = JSON.stringify(payload, null, 2);
      void navigator.clipboard
        .writeText(text)
        .then(() => {
          setCopiedRow(rowIdx);
          window.setTimeout(
            () => setCopiedRow((v) => (v === rowIdx ? null : v)),
            1200,
          );
        })
        .catch(() => {});
    },
    [columns, rows],
  );

  if (columns.length === 0) {
    return (
      <Box p={3} color="fg.muted" fontSize="sm">
        No columns returned
      </Box>
    );
  }

  return (
    <Box>
      <Box
        ref={scrollRef}
        maxH="340px"
        overflowY="auto"
        overflowX="auto"
        // Contain scroll chaining: once the table hits its top/bottom edge,
        // the wheel event does NOT propagate to the outer CodeMirror editor.
        overscrollBehavior="contain"
        onScroll={handleScroll}
        onMouseDown={(e: React.MouseEvent) => e.stopPropagation()}
        onKeyDown={(e: React.KeyboardEvent) => e.stopPropagation()}
        onCopy={(e: React.ClipboardEvent) => e.stopPropagation()}
        tabIndex={0}
      >
        <Table.Root
          size="sm"
          stickyHeader
          fontSize="xs"
          fontFamily="mono"
          css={tableCss}
        >
          <Table.Header>
            <Table.Row>
              {columns.map((col, i) => {
                const numeric = isNumericType(col.type);
                return (
                  <Table.ColumnHeader
                    key={i}
                    title={`${col.name} (${col.type})`}
                    className={numeric ? "cell-numeric" : undefined}
                  >
                    <HStack
                      gap={0}
                      align="baseline"
                      justify={numeric ? "flex-end" : undefined}
                    >
                      <Box as="span">{col.name}</Box>
                      <Box as="span" className="type-label">
                        {col.type}
                      </Box>
                    </HStack>
                  </Table.ColumnHeader>
                );
              })}
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {rows.length === 0 ? (
              <Table.Row>
                <Table.Cell
                  colSpan={columns.length}
                  textAlign="center"
                  color="fg.muted"
                  py={6}
                >
                  <Box
                    fontSize="10px"
                    textTransform="uppercase"
                    letterSpacing="0.08em"
                    opacity={0.55}
                  >
                    No rows returned
                  </Box>
                </Table.Cell>
              </Table.Row>
            ) : (
              <>
                {virtualizer.getVirtualItems()[0]?.start > 0 && (
                  <Table.Row>
                    <Table.Cell
                      colSpan={columns.length}
                      h={`${virtualizer.getVirtualItems()[0].start}px`}
                      p={0}
                      borderBottom="none"
                    />
                  </Table.Row>
                )}
                {virtualizer.getVirtualItems().map((virtualRow) => {
                  const rowIdx = virtualRow.index;
                  const row = rows[rowIdx];
                  const isExpanded = expandedRow === rowIdx;
                  const wasCopied = copiedRow === rowIdx;
                  return (
                    <Fragment key={rowIdx}>
                      <Table.Row
                        data-index={virtualRow.index}
                        data-selected={isExpanded ? "true" : undefined}
                        onClick={() => {
                          const sel = window.getSelection();
                          if (sel && sel.toString().length > 0) return;
                          setExpandedRow(isExpanded ? null : rowIdx);
                        }}
                      >
                        {columns.map((col, colIdx) => {
                          const cell = row[col.name] ?? null;
                          const { text, isNull } = formatCellValue(cell);
                          const numeric = isNumericType(col.type);
                          const className = [
                            numeric ? "cell-numeric" : "",
                            isNull ? "cell-null" : "",
                          ]
                            .filter(Boolean)
                            .join(" ");
                          return (
                            <Table.Cell
                              key={colIdx}
                              title={text}
                              className={className || undefined}
                            >
                              {text}
                            </Table.Cell>
                          );
                        })}
                      </Table.Row>
                      {isExpanded && (
                        <Table.Row>
                          <Table.Cell
                            colSpan={columns.length}
                            p={0}
                            bg="bg.subtle"
                            borderBottom="1px solid"
                            borderColor="border"
                          >
                            <Box
                              px={3}
                              py={2.5}
                              borderLeft="2px solid"
                              borderColor="brand.500"
                            >
                              <Flex
                                align="center"
                                justify="space-between"
                                mb={2}
                                gap={2}
                              >
                                <Box
                                  fontSize="10px"
                                  textTransform="uppercase"
                                  letterSpacing="0.08em"
                                  fontWeight="600"
                                  color="fg.muted"
                                >
                                  Row {rowIdx + 1}
                                  <Box
                                    as="span"
                                    ml={2}
                                    opacity={0.5}
                                    fontWeight="400"
                                    textTransform="none"
                                    letterSpacing="normal"
                                  >
                                    {columns.length} field
                                    {columns.length === 1 ? "" : "s"}
                                  </Box>
                                </Box>
                                <HStack gap={0}>
                                  <IconButton
                                    size="2xs"
                                    variant="ghost"
                                    aria-label="Copy row as JSON"
                                    title={
                                      wasCopied ? "Copied" : "Copy as JSON"
                                    }
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      copyRowAsJson(rowIdx);
                                    }}
                                    colorPalette={wasCopied ? "green" : "gray"}
                                  >
                                    <LuCopy />
                                  </IconButton>
                                  <IconButton
                                    size="2xs"
                                    variant="ghost"
                                    aria-label="Close row"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setExpandedRow(null);
                                    }}
                                  >
                                    <LuX />
                                  </IconButton>
                                </HStack>
                              </Flex>
                              <Box
                                display="grid"
                                gridTemplateColumns="max-content 1fr"
                                columnGap={4}
                                rowGap={1}
                                fontSize="xs"
                                fontFamily="mono"
                              >
                                {columns.map((col) => {
                                  const value = row[col.name] ?? null;
                                  return (
                                    <Fragment key={col.name}>
                                      <Box
                                        py="3px"
                                        whiteSpace="nowrap"
                                        minW={0}
                                      >
                                        <Box
                                          as="span"
                                          color="fg"
                                          fontWeight="600"
                                        >
                                          {col.name}
                                        </Box>
                                        <Box as="span" className="type-label">
                                          {col.type}
                                        </Box>
                                      </Box>
                                      <Box py="3px" minW={0}>
                                        <DetailValue value={value} />
                                      </Box>
                                    </Fragment>
                                  );
                                })}
                              </Box>
                            </Box>
                          </Table.Cell>
                        </Table.Row>
                      )}
                    </Fragment>
                  );
                })}
                {(() => {
                  const items = virtualizer.getVirtualItems();
                  const lastItem = items[items.length - 1];
                  const remaining = lastItem
                    ? virtualizer.getTotalSize() - lastItem.end
                    : 0;
                  return remaining > 0 ? (
                    <Table.Row>
                      <Table.Cell
                        colSpan={columns.length}
                        h={`${remaining}px`}
                        p={0}
                        borderBottom="none"
                      />
                    </Table.Row>
                  ) : null;
                })()}
              </>
            )}
          </Table.Body>
        </Table.Root>
      </Box>

      {/* Footer — stats only (count + duration). Infinite scroll via
          handleScroll triggers load-more silently; no visible loading UI. */}
      {durationMs != null && (
        <Flex
          align="center"
          px={3}
          py={1.5}
          minHeight="32px"
          fontSize="xs"
          fontFamily="mono"
          color="fg.muted"
        >
          <HStack gap={2}>
            <Box as="span" color="fg" opacity={0.85}>
              {rows.length.toLocaleString()}
            </Box>
            <Box as="span" opacity={0.6}>
              row{rows.length === 1 ? "" : "s"}
            </Box>
            <Box as="span" opacity={0.3}>
              ·
            </Box>
            <Box as="span" opacity={0.6}>
              {formatElapsed(durationMs)}
            </Box>
          </HStack>
        </Flex>
      )}
    </Box>
  );
}
