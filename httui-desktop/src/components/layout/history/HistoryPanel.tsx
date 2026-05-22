import { useCallback, useEffect, useState } from "react";
import { Box, HStack, IconButton, Spinner, Text } from "@chakra-ui/react";
import { LuClock, LuRefreshCw, LuX } from "react-icons/lu";

import { HistoryList } from "@/components/layout/history/HistoryList";
import {
  listBlockHistoryForFile,
  type HistoryEntry,
} from "@/lib/tauri/commands";
import { selectActiveTabPath, usePaneStore } from "@/stores/pane";

interface HistoryPanelProps {
  width: number;
  onClose: () => void;
}

const DEFAULT_LIMIT = 50;

export function HistoryPanel({ width, onClose }: HistoryPanelProps) {
  const filePath = usePaneStore(selectActiveTabPath);
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchOnce = useCallback(async () => {
    if (!filePath) {
      setEntries([]);
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const next = await listBlockHistoryForFile(filePath, DEFAULT_LIMIT);
      setEntries(next);
    } catch (e) {
      setEntries([]);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [filePath]);

  useEffect(() => {
    void fetchOnce();
  }, [fetchOnce]);

  return (
    <Box
      data-testid="history-panel"
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
          <LuClock size={14} />
          <Text
            fontSize="xs"
            fontWeight="semibold"
            color="fg.subtle"
            textTransform="uppercase"
            letterSpacing="wider"
          >
            History
          </Text>
        </HStack>
        <HStack gap={1}>
          <IconButton
            aria-label="Refresh history"
            variant="ghost"
            size="xs"
            onClick={() => void fetchOnce()}
            disabled={loading}
          >
            {loading ? <Spinner size="xs" /> : <LuRefreshCw />}
          </IconButton>
          <IconButton
            aria-label="Close history panel"
            variant="ghost"
            size="xs"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </HStack>
      </HStack>
      <Box overflow="auto" flex={1}>
        {error !== null && (
          <Box
            data-testid="history-panel-error"
            px={3}
            py={2}
            fontSize="11px"
            color="error"
          >
            {error}
          </Box>
        )}
        <HistoryList entries={entries} />
      </Box>
    </Box>
  );
}
