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
    // Clamp against current doc length — entry.offset may be stale if
    // the doc was edited between parse and click.
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
