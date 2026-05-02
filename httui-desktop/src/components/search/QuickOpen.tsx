import { useEffect, useRef } from "react";
import { Box, Flex, Input, Text } from "@chakra-ui/react";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { useFileSearch } from "@/hooks/useFileSearch";
import { LuFileText } from "react-icons/lu";

interface QuickOpenProps {
  open: boolean;
  onClose: () => void;
}

export function QuickOpen({ open, onClose }: QuickOpenProps) {
  if (!open) return null;
  return <QuickOpenInner onClose={onClose} />;
}

function QuickOpenInner({ onClose }: { onClose: () => void }) {
  const { vaultPath, handleFileSelect } = useWorkspace();
  const inputRef = useRef<HTMLInputElement>(null);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);

  const {
    query,
    results,
    safeIndex,
    setSelectedIndex,
    handleSearch,
    handleSelect,
    handleKeyDown,
  } = useFileSearch({
    vaultPath,
    onSelect: handleFileSelect,
    onClose,
  });

  useEffect(() => {
    setTimeout(() => inputRef.current?.focus(), 50);
  }, []);

  // Scroll selected item into view
  useEffect(() => {
    itemRefs.current[safeIndex]?.scrollIntoView({ block: "nearest" });
  }, [safeIndex]);

  return (
    <>
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.400"
        zIndex={9998}
        onClick={onClose}
      />
      <Box
        position="fixed"
        top="80px"
        left="50%"
        transform="translateX(-50%)"
        w="500px"
        maxW="90vw"
        bg="bg.1"
        borderWidth="1px"
        borderColor="line"
        rounded="lg"
        shadow="2xl"
        zIndex={9999}
        overflow="hidden"
      >
        <Box p={2}>
          <Input
            ref={inputRef}
            placeholder="Buscar arquivo..."
            value={query}
            onChange={(e) => handleSearch(e.target.value)}
            onKeyDown={handleKeyDown}
            size="md"
            variant="flushed"
            autoComplete="off"
          />
        </Box>
        <Box maxH="300px" overflowY="auto" pb={1}>
          {results.length === 0 && query && (
            <Flex px={3} py={4} justify="center">
              <Text fontSize="sm" color="fg.3">
                Nenhum resultado
              </Text>
            </Flex>
          )}
          {results.map((result, index) => (
            <Flex
              key={result.path}
              ref={(el: HTMLDivElement | null) => {
                itemRefs.current[index] = el;
              }}
              align="center"
              gap={2}
              px={3}
              py={1.5}
              mx={1}
              rounded="md"
              cursor="pointer"
              bg={index === safeIndex ? "bg.3" : "transparent"}
              _hover={{ bg: "bg.3" }}
              onClick={() => handleSelect(index)}
              onMouseEnter={() => setSelectedIndex(index)}
            >
              <LuFileText size={14} />
              <Box flex={1}>
                <Text fontSize="sm" color="fg">
                  {result.name}
                </Text>
                <Text fontSize="xs" color="fg.3">
                  {result.path}
                </Text>
              </Box>
            </Flex>
          ))}
        </Box>
      </Box>
    </>
  );
}
