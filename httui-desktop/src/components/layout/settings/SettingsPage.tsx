// SettingsPage — V3 replacement for the legacy SettingsDrawer. Two
// top tabs: User (per-machine) and Workspace (committed defaults).
//
// Mounting is the same Portal + Box pattern the drawer used (NOT
// Chakra Dialog — Dialog's focus trap breaks CM6 keyboard input on
// close, see CLAUDE.md "Key Conventions").

import { useState } from "react";
import {
  Box,
  Flex,
  IconButton,
  Portal,
  Tabs,
  Text,
} from "@chakra-ui/react";
import { LuX } from "react-icons/lu";

import { useSettingsStore } from "@/stores/settings";

import { SettingsUserTab } from "./SettingsUserTab";
import { SettingsWorkspaceTab } from "./SettingsWorkspaceTab";

type SettingsTab = "user" | "workspace";

export function SettingsPage() {
  const settingsOpen = useSettingsStore((s) => s.settingsOpen);
  const closeSettings = useSettingsStore((s) => s.closeSettings);
  const [active, setActive] = useState<SettingsTab>("user");

  if (!settingsOpen) return null;

  return (
    <Portal>
      <Box
        data-testid="settings-page-backdrop"
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1400}
        onClick={closeSettings}
      />
      <Box
        data-testid="settings-page"
        position="fixed"
        top={0}
        right={0}
        h="100vh"
        w="780px"
        maxW="90vw"
        bg="bg"
        borderLeftWidth="1px"
        borderColor="border"
        zIndex={1401}
        display="flex"
        flexDirection="column"
      >
        <Flex
          align="center"
          justify="space-between"
          px={4}
          py={3}
          borderBottomWidth="1px"
          borderColor="border"
        >
          <Text fontWeight="semibold" fontSize="sm">
            Settings
          </Text>
          <IconButton
            aria-label="Close settings"
            variant="ghost"
            size="sm"
            onClick={closeSettings}
          >
            <LuX />
          </IconButton>
        </Flex>

        <Tabs.Root
          value={active}
          onValueChange={(d) => setActive(d.value as SettingsTab)}
          size="sm"
          variant="line"
          flex={1}
          display="flex"
          flexDirection="column"
          overflow="hidden"
        >
          <Tabs.List px={4} pt={2}>
            <Tabs.Trigger value="user" data-testid="settings-tab-user">
              User
            </Tabs.Trigger>
            <Tabs.Trigger
              value="workspace"
              data-testid="settings-tab-workspace"
            >
              Workspace
            </Tabs.Trigger>
          </Tabs.List>

          <Tabs.Content
            value="user"
            flex={1}
            overflow="auto"
            px={4}
            py={4}
          >
            <SettingsUserTab />
          </Tabs.Content>

          <Tabs.Content
            value="workspace"
            flex={1}
            overflow="auto"
            px={4}
            py={4}
          >
            <SettingsWorkspaceTab />
          </Tabs.Content>
        </Tabs.Root>
      </Box>
    </Portal>
  );
}
