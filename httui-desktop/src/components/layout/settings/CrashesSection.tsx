import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Flex,
  HStack,
  VStack,
  Text,
  Button,
  Badge,
} from "@chakra-ui/react";
import {
  listCrashLogs,
  readCrashLog,
  clearCrashLogs,
  type CrashLog,
} from "@/lib/tauri/crashes";

function formatTimestamp(epochMs: number): string {
  return new Date(epochMs).toLocaleString();
}

export function CrashesSection() {
  const [logs, setLogs] = useState<CrashLog[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [body, setBody] = useState<string>("");

  const refresh = useCallback(async () => {
    try {
      setLogs(await listCrashLogs());
    } catch (e) {
      console.error("Failed to list crash logs:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleSelect = useCallback(async (name: string) => {
    setSelected(name);
    try {
      setBody(await readCrashLog(name));
    } catch (e) {
      setBody(`Failed to read crash log: ${String(e)}`);
    }
  }, []);

  const handleClear = useCallback(async () => {
    try {
      await clearCrashLogs();
      setSelected(null);
      setBody("");
      await refresh();
    } catch (e) {
      console.error("Failed to clear crash logs:", e);
    }
  }, [refresh]);

  return (
    <Flex direction="column" gap={4}>
      <Box>
        <Flex align="center" justify="space-between" mb={1}>
          <Text fontWeight="semibold" fontSize="sm">
            Crash logs
          </Text>
          <Button
            size="xs"
            variant="outline"
            onClick={handleClear}
            disabled={logs.length === 0}
          >
            Clear all
          </Button>
        </Flex>
        <Text fontSize="xs" color="fg.muted">
          Panics captured locally from the app and the language server. Stored
          on this machine only — nothing is uploaded.
        </Text>
      </Box>

      {logs.length === 0 ? (
        <Text fontSize="sm" color="fg.muted" textAlign="center" py={4}>
          No crashes recorded
        </Text>
      ) : (
        <VStack gap={1} align="stretch">
          {logs.map((log) => (
            <Box
              key={log.name}
              as="button"
              textAlign="left"
              px={2.5}
              py={1.5}
              rounded="md"
              borderWidth="1px"
              borderColor={selected === log.name ? "blue.400" : "border"}
              bg={selected === log.name ? "bg.subtle" : "transparent"}
              _hover={{ bg: "bg.subtle" }}
              onClick={() => handleSelect(log.name)}
            >
              <HStack gap={2} mb={0.5}>
                <Badge size="xs" colorPalette="red" variant="subtle">
                  {log.source}
                </Badge>
                <Text fontSize="2xs" color="fg.muted">
                  {formatTimestamp(log.epoch_ms)}
                </Text>
              </HStack>
              <Text fontSize="xs" truncate fontFamily="mono">
                {log.summary || "(empty)"}
              </Text>
            </Box>
          ))}
        </VStack>
      )}

      {selected && (
        <Box>
          <Text fontSize="xs" fontWeight="semibold" color="fg.muted" mb={1}>
            {selected}
          </Text>
          <Box
            as="pre"
            fontSize="2xs"
            fontFamily="mono"
            whiteSpace="pre-wrap"
            wordBreak="break-word"
            bg="bg.subtle"
            borderWidth="1px"
            borderColor="border"
            rounded="md"
            p={2}
            maxH="320px"
            overflow="auto"
          >
            {body}
          </Box>
        </Box>
      )}
    </Flex>
  );
}
