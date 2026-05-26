import {
  HStack,
  Text,
  Circle,
  IconButton,
  Menu,
  Portal,
} from "@chakra-ui/react";
import { LuX, LuGitCompareArrows } from "react-icons/lu";
import type { TabState } from "@/types/pane";
import { getTabId } from "@/types/pane";

interface TabBarProps {
  tabs: TabState[];
  activeTab: number;
  unsavedFiles: Set<string>;
  onSelectTab: (index: number) => void;
  onCloseTab: (index: number) => void;
  onCloseOthers: (index: number) => void;
  onCloseAll: () => void;
}

export function TabBar({
  tabs,
  activeTab,
  unsavedFiles,
  onSelectTab,
  onCloseTab,
  onCloseOthers,
  onCloseAll,
}: TabBarProps) {
  if (tabs.length === 0) return null;

  return (
    <HStack
      h="32px"
      bg="bg"
      borderBottomWidth="1px"
      borderColor="border"
      gap={0}
      overflowX="auto"
      flexShrink={0}
      css={{ "&::-webkit-scrollbar": { display: "none" } }}
    >
      {tabs.map((tab, index) => {
        const isActive = index === activeTab;
        const rawName =
          tab.filePath.split("/").pop()?.replace(".md", "") ?? tab.filePath;
        const isDiff = tab.kind === "diff";
        const fileName = isDiff ? `Diff: ${rawName}` : rawName;

        return (
          <Menu.Root key={getTabId(tab)}>
            <Menu.ContextTrigger asChild>
              <HStack
                px={3}
                h="100%"
                gap={1.5}
                cursor="pointer"
                bg={isActive ? "bg" : "bg.subtle"}
                borderRightWidth="1px"
                borderColor="border"
                borderBottomWidth={isActive ? "2px" : "0"}
                borderBottomColor={isActive ? "brand.500" : "transparent"}
                _hover={{ bg: isActive ? "bg" : "bg.muted" }}
                onClick={() => onSelectTab(index)}
                onMouseDown={(e) => {
                  // Middle click to close
                  if (e.button === 1) {
                    e.preventDefault();
                    onCloseTab(index);
                  }
                }}
              >
                {isDiff && (
                  <LuGitCompareArrows size={11} style={{ flexShrink: 0 }} />
                )}
                <Text
                  fontSize="xs"
                  color={isActive ? "fg" : "fg.subtle"}
                  whiteSpace="nowrap"
                >
                  {fileName}
                </Text>
                {!isDiff && unsavedFiles.has(tab.filePath) && (
                  <Circle size="6px" bg="orange.400" />
                )}
                <IconButton
                  aria-label="Close tab"
                  variant="ghost"
                  size="xs"
                  minW="16px"
                  h="16px"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(index);
                  }}
                  opacity={isActive ? 1 : 0}
                  _groupHover={{ opacity: 1 }}
                >
                  <LuX size={10} />
                </IconButton>
              </HStack>
            </Menu.ContextTrigger>
            <Portal>
              <Menu.Positioner>
                <Menu.Content>
                  <Menu.Item value="close" onSelect={() => onCloseTab(index)}>
                    Fechar
                  </Menu.Item>
                  <Menu.Item
                    value="close-others"
                    onSelect={() => onCloseOthers(index)}
                  >
                    Fechar outros
                  </Menu.Item>
                  <Menu.Item value="close-all" onSelect={onCloseAll}>
                    Fechar todos
                  </Menu.Item>
                </Menu.Content>
              </Menu.Positioner>
            </Portal>
          </Menu.Root>
        );
      })}
    </HStack>
  );
}
