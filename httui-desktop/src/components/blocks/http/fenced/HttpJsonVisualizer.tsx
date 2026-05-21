// JSON tree visualizer for the HTTP block result panel.
//
// Extracted verbatim from HttpFencedPanel.tsx (A1 / audit 03 §1 seam
// #1) — pure, highly testable, ~370 L. `HttpJsonVisualizer` +
// `parseJsonForVisualize` are consumed by `HttpBodyView`; the flatten/
// expand/primitive helpers are exported so they can be unit-tested in
// isolation (the panel itself had ~no coverage).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Box, Portal, Text } from "@chakra-ui/react";
import { useVirtualizer } from "@tanstack/react-virtual";

export function parseJsonForVisualize(prettyBody: string): unknown {
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
export type JsonFlatNode =
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
export function shouldDefaultExpand(value: unknown, depth: number): boolean {
  if (depth === 0) return true;
  if (depth === 1) {
    if (Array.isArray(value)) return value.length <= 20;
    if (value !== null && typeof value === "object") {
      return Object.keys(value as Record<string, unknown>).length <= 20;
    }
  }
  return false;
}

export function initialCollapsedPaths(data: unknown): Set<string> {
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

export function flattenJson(
  data: unknown,
  collapsed: Set<string>,
): JsonFlatNode[] {
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
export function HttpJsonVisualizer({ data }: { data: unknown }) {
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

export function primitiveDisplay(v: unknown): string {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  if (typeof v === "string") return `"${v}"`;
  return String(v);
}

export function primitiveColor(v: unknown): string {
  if (v === null || v === undefined) return "fg.muted";
  if (typeof v === "string") return "green.fg";
  if (typeof v === "number") return "blue.fg";
  if (typeof v === "boolean") return "orange.fg";
  return "fg";
}
