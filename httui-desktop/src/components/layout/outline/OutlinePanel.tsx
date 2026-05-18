// Right-side outline panel — mount.
//
// Sibling toggle to `<SchemaPanel>`. Reads the active editor's
// content from the pane store, derives an outline via
// `extractOutline`, and renders `<OutlineList>` with click-to-
// navigate that dispatches a CM6 selection on the registered
// active editor (`getActiveEditor()` from
// `lib/codemirror/active-editor.ts` — same registry SchemaPanel
// uses).
//
// Listens to editorContents map mutations indirectly via the
// pane-store subscription, so the outline updates as the user
// types (the parser short-circuits when there are no changes
// — useMemo on content keeps re-renders cheap).

import { useCallback, useMemo } from "react";
import { Box, HStack, IconButton, Text } from "@chakra-ui/react";
import { LuListTree, LuX } from "react-icons/lu";

import { OutlineList } from "@/components/layout/outline/OutlineList";
import { extractOutline } from "@/lib/blocks/outline";
import { getActiveEditor } from "@/lib/codemirror/active-editor";
import { usePaneStore, selectActiveTabPath } from "@/stores/pane";
import type { OutlineEntry } from "@/lib/blocks/outline";

interface OutlinePanelProps {
  width: number;
  onClose: () => void;
}

export function OutlinePanel({ width, onClose }: OutlinePanelProps) {
  const filePath = usePaneStore(selectActiveTabPath);
  const content = usePaneStore((s) =>
    filePath ? (s.editorContents.get(filePath) ?? "") : "",
  );
  const entries = useMemo(() => extractOutline(content), [content]);

  const handleSelect = useCallback((entry: OutlineEntry) => {
    const view = getActiveEditor();
    if (!view) return;
    // Clamp the offset against current doc length — the active
    // editor may have been edited between parse and click, leaving
    // entry.offset stale. Past-end offsets land at the document
    // end, which is the safe sane behaviour.
    const safeAnchor = Math.min(entry.offset, view.state.doc.length);
    view.dispatch({
      selection: { anchor: safeAnchor },
      effects: [],
      scrollIntoView: true,
    });
    view.focus();
  }, []);

  return (
    <Box
      data-testid="outline-panel"
      w={`${width}px`}
      bg="bg"
      borderLeftWidth="1px"
      borderColor="border"
      display="flex"
      flexDirection="column"
      overflow="hidden"
      flexShrink={0}
    >
      <HStack
        px={3}
        py={2}
        borderBottomWidth="1px"
        borderColor="border"
        justify="space-between"
      >
        <HStack gap={2}>
          <LuListTree size={14} />
          <Text
            fontSize="xs"
            fontWeight="semibold"
            color="fg.subtle"
            textTransform="uppercase"
            letterSpacing="wider"
          >
            Outline
          </Text>
        </HStack>
        <IconButton
          aria-label="Close outline panel"
          variant="ghost"
          size="xs"
          onClick={onClose}
        >
          <LuX />
        </IconButton>
      </HStack>
      <Box overflow="auto" flex={1}>
        <OutlineList entries={entries} onSelect={handleSelect} />
      </Box>
    </Box>
  );
}
