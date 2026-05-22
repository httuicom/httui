import { Box, HStack, Text, IconButton, Menu, Portal } from "@chakra-ui/react";
import { FileTree } from "./file-tree";
import { ConnectionsList } from "./connections/ConnectionsList";
import { VariablesPanel } from "./VariablesPanel";
import { useWorkspace } from "@/contexts/WorkspaceContext";
import { LuPlus, LuFileText, LuFolder } from "react-icons/lu";

interface SidebarProps {
  width: number;
}

export function Sidebar({ width }: SidebarProps) {
  const { vaultPath, handleStartCreate } = useWorkspace();

  return (
    <Box
      w={`${width}px`}
      bg="bg"
      borderRightWidth="1px"
      borderColor="border"
      display="flex"
      flexDirection="column"
      overflow="hidden"
      flexShrink={0}
    >
      <Box flex={1} overflowY="auto">
        <HStack px={3} py={2} justify="space-between">
          <Text
            fontSize="xs"
            fontWeight="semibold"
            color="fg.subtle"
            textTransform="uppercase"
            letterSpacing="wider"
          >
            Files
          </Text>
          {vaultPath && (
            <Menu.Root positioning={{ placement: "bottom-end" }}>
              <Menu.Trigger asChild>
                <IconButton aria-label="New..." variant="ghost" size="xs">
                  <LuPlus />
                </IconButton>
              </Menu.Trigger>
              <Portal>
                <Menu.Positioner>
                  <Menu.Content>
                    <Menu.Item
                      value="note"
                      onSelect={() => handleStartCreate("note", "")}
                    >
                      <LuFileText />
                      Nova nota
                    </Menu.Item>
                    <Menu.Item
                      value="folder"
                      onSelect={() => handleStartCreate("folder", "")}
                    >
                      <LuFolder />
                      Nova pasta
                    </Menu.Item>
                  </Menu.Content>
                </Menu.Positioner>
              </Portal>
            </Menu.Root>
          )}
        </HStack>
        {vaultPath ? (
          <FileTree />
        ) : (
          <Box px={3} py={8} textAlign="center">
            <Text fontSize="sm" color="fg.muted">
              No vault selected
            </Text>
          </Box>
        )}
      </Box>

      <Box borderTopWidth="1px" borderColor="border">
        <ConnectionsList />
      </Box>

      <Box borderTopWidth="1px" borderColor="border">
        <VariablesPanel />
      </Box>
    </Box>
  );
}
