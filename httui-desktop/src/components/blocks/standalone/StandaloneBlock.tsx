import { useState, useCallback, useMemo, useEffect, useRef, memo } from "react";
import { Box, Text, Badge, HStack } from "@chakra-ui/react";
import {
  EditorState,
  RangeSetBuilder,
  StateField,
  type Extension,
} from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";
import { syntaxHighlighting } from "@codemirror/language";
import { oneDarkHighlightStyle } from "@codemirror/theme-one-dark";
import { sql } from "@codemirror/lang-sql";
import { json } from "@codemirror/lang-json";
import { StandaloneBlockShell } from "../StandaloneBlockShell";
import { ResultTable } from "../db/ResultTable";
import {
  firstSelectResult,
  normalizeDbResponse,
  type DbResponse,
} from "../db/types";
import { executeBlock } from "@/lib/tauri/commands";
import type { DisplayMode, ExecutionState } from "../ExecutableBlock";

interface StandaloneBlockProps {
  blockType: string;
  content: string;
  counterpartContent?: string;
  /** "a" = left/current side, "b" = right/proposed side */
  side?: "a" | "b";
  alias?: string;
}

interface ParsedBlock {
  displayContent: string;
  connectionId?: string;
  method?: string;
  url?: string;
}

function parseBlockContent(blockType: string, raw: string): ParsedBlock {
  try {
    const data = JSON.parse(raw);
    if (blockType === "db") {
      return {
        displayContent: data.query ?? raw,
        connectionId: data.connectionId,
      };
    }
    if (blockType === "http") {
      if (typeof data === "string") return { displayContent: data };
      return {
        displayContent: data.body ?? raw,
        method: data.method,
        url: data.url,
      };
    }
    return { displayContent: JSON.stringify(data, null, 2) };
  } catch {
    return { displayContent: raw };
  }
}

function langExtension(blockType: string): Extension[] {
  if (blockType === "db") return [sql()];
  if (blockType === "http") return [json()];
  return [];
}

/** Compute which lines differ between two texts. Returns set of 1-based line numbers. */
function computeChangedLines(thisText: string, otherText: string): Set<number> {
  const thisLines = thisText.split("\n");
  const otherLines = otherText.split("\n");
  const changed = new Set<number>();

  const maxLen = Math.max(thisLines.length, otherLines.length);
  for (let i = 0; i < maxLen; i++) {
    if (thisLines[i] !== otherLines[i]) {
      if (i < thisLines.length) changed.add(i + 1);
    }
  }
  return changed;
}

/** Create a StateField that applies line decorations for changed lines */
function createDiffHighlightField(changedLines: Set<number>, side: "a" | "b") {
  const lineClass =
    side === "a"
      ? Decoration.line({ class: "cm-diff-deleted" })
      : Decoration.line({ class: "cm-diff-added" });

  return StateField.define<DecorationSet>({
    create(state) {
      const builder = new RangeSetBuilder<Decoration>();
      for (let i = 1; i <= state.doc.lines; i++) {
        if (changedLines.has(i)) {
          builder.add(
            state.doc.line(i).from,
            state.doc.line(i).from,
            lineClass,
          );
        }
      }
      return builder.finish();
    },
    update(decos) {
      return decos; // read-only doc, decorations don't change
    },
    provide: (f) => EditorView.decorations.from(f),
  });
}

const readOnlyExt = EditorState.readOnly.of(true);
const cmTheme = EditorView.theme({
  "&": { fontSize: "12px", maxHeight: "250px" },
  ".cm-content": { fontFamily: "var(--chakra-fonts-mono)", padding: "8px" },
  ".cm-gutters": { display: "none" },
  ".cm-scroller": { overflow: "auto" },
  ".cm-activeLine": { backgroundColor: "transparent" },
  ".cm-diff-deleted": { backgroundColor: "rgba(248, 81, 73, 0.15)" },
  ".cm-diff-added": { backgroundColor: "rgba(63, 185, 80, 0.15)" },
});

/** Single CodeMirror editor with diff line highlights */
function BlockCodeEditor({
  content,
  counterpartContent,
  blockType,
  side,
}: {
  content: string;
  counterpartContent?: string;
  blockType: string;
  side: "a" | "b";
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const extensions: Extension[] = [
      readOnlyExt,
      cmTheme,
      syntaxHighlighting(oneDarkHighlightStyle),
      ...langExtension(blockType),
    ];

    if (counterpartContent !== undefined && counterpartContent !== content) {
      const changedLines = computeChangedLines(content, counterpartContent);
      if (changedLines.size > 0) {
        extensions.push(createDiffHighlightField(changedLines, side));
      }
    }

    const view = new EditorView({
      state: EditorState.create({ doc: content, extensions }),
      parent: containerRef.current,
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [content, counterpartContent, blockType, side]);

  return (
    <Box
      ref={containerRef}
      border="1px solid"
      borderColor="border"
      rounded="md"
      overflow="hidden"
      mx={3}
      my={2}
    />
  );
}

export const StandaloneBlock = memo(function StandaloneBlock({
  blockType,
  content,
  counterpartContent,
  side = "b",
  alias,
}: StandaloneBlockProps) {
  const [displayMode, setDisplayMode] = useState<DisplayMode>("input");
  const [executionState, setExecutionState] = useState<ExecutionState>("idle");
  const [dbResponse, setDbResponse] = useState<DbResponse | null>(null);
  const [rawResponse, setRawResponse] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const parsed = useMemo(
    () => parseBlockContent(blockType, content),
    [blockType, content],
  );

  const handleRun = useCallback(async () => {
    setExecutionState("running");
    setError(null);
    try {
      const params = buildParams(blockType, content);
      const result = await executeBlock(blockType, params);
      if (blockType === "db" && result.data) {
        setDbResponse(normalizeDbResponse(result.data));
      } else {
        setRawResponse(JSON.stringify(result.data, null, 2));
      }
      setExecutionState(result.status === "error" ? "error" : "success");
      setDisplayMode("split");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setExecutionState("error");
      setDisplayMode("split");
    }
  }, [blockType, content]);

  const handleCancel = useCallback(() => {
    setExecutionState("idle");
  }, []);

  return (
    <Box my={1} mx={1}>
      <StandaloneBlockShell
        blockType={blockType}
        alias={alias ?? ""}
        displayMode={displayMode}
        executionState={executionState}
        onAliasChange={() => {}}
        onDisplayModeChange={setDisplayMode}
        onRun={handleRun}
        onCancel={handleCancel}
        splitDirection="column"
        inputSlot={
          <Box>
            {parsed.method && (
              <HStack gap={2} px={3} pt={2}>
                <Badge size="sm" colorPalette="blue">
                  {parsed.method}
                </Badge>
                <Text fontSize="xs" fontFamily="mono" color="fg.muted" truncate>
                  {parsed.url}
                </Text>
              </HStack>
            )}
            <BlockCodeEditor
              content={parsed.displayContent}
              counterpartContent={counterpartContent}
              blockType={blockType}
              side={side}
            />
          </Box>
        }
        outputSlot={(() => {
          if (error) {
            return (
              <Box p={3} color="red.500" fontSize="sm" fontFamily="mono">
                {error}
              </Box>
            );
          }
          if (dbResponse) {
            const sel = firstSelectResult(dbResponse);
            if (sel) {
              return (
                <Box p={2} display="flex" flexDirection="column" gap={1}>
                  <HStack gap={2}>
                    <Badge
                      colorPalette="green"
                      variant="subtle"
                      fontFamily="mono"
                      size="sm"
                    >
                      {sel.rows.length} rows
                    </Badge>
                  </HStack>
                  <ResultTable
                    columns={sel.columns}
                    rows={sel.rows}
                    hasMore={false}
                  />
                </Box>
              );
            }
            const first = dbResponse.results[0];
            if (first && first.kind === "mutation") {
              return (
                <Box p={3}>
                  <Badge
                    colorPalette="blue"
                    variant="subtle"
                    fontFamily="mono"
                    size="sm"
                  >
                    {first.rows_affected} rows affected
                  </Badge>
                </Box>
              );
            }
            if (first && first.kind === "error") {
              return (
                <Box p={3} color="red.500" fontSize="sm" fontFamily="mono">
                  {first.message}
                </Box>
              );
            }
            return null;
          }
          return rawResponse ? (
            <Box px={3} py={2}>
              <Box
                bg="bg.subtle"
                border="1px solid"
                borderColor="border"
                rounded="md"
                px={3}
                py={2}
                fontFamily="mono"
                fontSize="xs"
                whiteSpace="pre-wrap"
                overflowX="auto"
                maxH="200px"
                overflowY="auto"
              >
                {rawResponse}
              </Box>
            </Box>
          ) : null;
        })()}
      />
    </Box>
  );
});

function buildParams(
  blockType: string,
  content: string,
): Record<string, unknown> {
  try {
    const data = JSON.parse(content);
    if (blockType === "db") {
      return {
        query: data.query ?? content,
        connection_id: data.connectionId ?? "",
        page: 1,
        page_size: 100,
      };
    }
    if (blockType === "http") return data;
    return data;
  } catch {
    if (blockType === "db") {
      return { query: content, connection_id: "", page: 1, page_size: 100 };
    }
    return { raw: content };
  }
}
