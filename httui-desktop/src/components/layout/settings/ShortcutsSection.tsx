import { Box, Flex, Text, VStack, Kbd } from "@chakra-ui/react";

interface Shortcut {
  keys: string[];
  description: string;
  category: string;
}

const IS_MAC = navigator.platform.toUpperCase().includes("MAC");
const MOD = IS_MAC ? "Cmd" : "Ctrl";

const SHORTCUTS: Shortcut[] = [
  // Navigation
  {
    keys: [MOD, "P"],
    description: "Quick open — search files by name",
    category: "Navigation",
  },
  {
    keys: [MOD, "Shift", "F"],
    description: "Full-text search across all notes",
    category: "Navigation",
  },
  {
    keys: [MOD, "Tab"],
    description: "Switch to next tab in active pane",
    category: "Navigation",
  },
  // Layout
  {
    keys: [MOD, "B"],
    description: "Toggle sidebar visibility",
    category: "Layout",
  },
  {
    keys: [MOD, "\\"],
    description: "Split pane vertically",
    category: "Layout",
  },
  {
    keys: [MOD, "Shift", "\\"],
    description: "Split pane horizontally",
    category: "Layout",
  },
  { keys: [MOD, "W"], description: "Close active tab", category: "Layout" },
  { keys: [MOD, "L"], description: "Toggle chat panel", category: "Layout" },
  // Editing
  {
    keys: [MOD, "S"],
    description: "Force save current file",
    category: "Editing",
  },
];

const CATEGORIES = ["Navigation", "Layout", "Editing"];

export function ShortcutsSection() {
  return (
    <Flex direction="column" gap={4}>
      <Box>
        <Text fontWeight="semibold" fontSize="sm">
          Keyboard shortcuts
        </Text>
        <Text fontSize="xs" color="fg.muted" mt={1}>
          All shortcuts use {IS_MAC ? "Cmd (⌘)" : "Ctrl"} as the modifier key.
          Shortcuts are currently not customizable.
        </Text>
      </Box>

      {CATEGORIES.map((category) => {
        const items = SHORTCUTS.filter((s) => s.category === category);
        return (
          <Box key={category}>
            <Text fontSize="xs" fontWeight="medium" color="fg.muted" mb={2}>
              {category}
            </Text>
            <VStack gap={0} align="stretch">
              {items.map((shortcut) => (
                <Flex
                  key={shortcut.description}
                  align="center"
                  justify="space-between"
                  py={1.5}
                  px={2}
                  borderRadius="md"
                  _hover={{ bg: "bg.subtle" }}
                >
                  <Text fontSize="xs">{shortcut.description}</Text>
                  <Flex gap={1} flexShrink={0}>
                    {shortcut.keys.map((key, i) => (
                      <Kbd key={i} size="sm">
                        {key === "Cmd" ? "⌘" : key === "Shift" ? "⇧" : key}
                      </Kbd>
                    ))}
                  </Flex>
                </Flex>
              ))}
            </VStack>
          </Box>
        );
      })}
    </Flex>
  );
}
