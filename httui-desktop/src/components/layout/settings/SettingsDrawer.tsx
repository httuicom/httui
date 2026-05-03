import { useState } from "react";
import {
  Box,
  Flex,
  Text,
  IconButton,
  VStack,
  Portal,
  Separator,
} from "@chakra-ui/react";
import {
  LuX,
  LuSettings,
  LuCode,
  LuShieldCheck,
  LuKeyboard,
  LuInfo,
  LuPalette,
} from "react-icons/lu";
import { useSettingsStore } from "@/stores/settings";
import { AuditSection } from "./AuditSection";
import { GeneralSection } from "./GeneralSection";
import { EditorSection } from "./EditorSection";
import { ShortcutsSection } from "./ShortcutsSection";
import { ThemeSection } from "./ThemeSection";
import { AboutSection } from "./AboutSection";

type SettingsTab =
  | "general"
  | "theme"
  | "editor"
  | "shortcuts"
  | "audit"
  | "about";

interface TabDef {
  id: SettingsTab;
  label: string;
  icon: React.ReactNode;
  group: "settings" | "advanced" | "info";
}

const TABS: TabDef[] = [
  {
    id: "general",
    label: "General",
    icon: <LuSettings size={14} />,
    group: "settings",
  },
  {
    id: "theme",
    label: "Theme",
    icon: <LuPalette size={14} />,
    group: "settings",
  },
  {
    id: "editor",
    label: "Editor",
    icon: <LuCode size={14} />,
    group: "settings",
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    icon: <LuKeyboard size={14} />,
    group: "settings",
  },
  {
    id: "audit",
    label: "Audit",
    icon: <LuShieldCheck size={14} />,
    group: "advanced",
  },
  { id: "about", label: "About", icon: <LuInfo size={14} />, group: "info" },
];

export function SettingsDrawer() {
  const settingsOpen = useSettingsStore((s) => s.settingsOpen);
  const closeSettings = useSettingsStore((s) => s.closeSettings);
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");

  if (!settingsOpen) return null;

  const settingsTabs = TABS.filter((t) => t.group === "settings");
  const advancedTabs = TABS.filter((t) => t.group === "advanced");
  const infoTabs = TABS.filter((t) => t.group === "info");

  const renderTabItem = (tab: TabDef) => (
    <Flex
      key={tab.id}
      align="center"
      gap={2}
      px={2}
      py={1.5}
      borderRadius="md"
      cursor="pointer"
      fontSize="xs"
      fontWeight={activeTab === tab.id ? "semibold" : "normal"}
      bg={activeTab === tab.id ? "bg.subtle" : "transparent"}
      _hover={{ bg: "bg.subtle" }}
      onClick={() => setActiveTab(tab.id)}
    >
      {tab.icon}
      <Text>{tab.label}</Text>
    </Flex>
  );

  return (
    <Portal>
      {/* Backdrop */}
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1400}
        onClick={closeSettings}
      />
      {/* Panel */}
      <Box
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
        {/* Header */}
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
            aria-label="Close"
            variant="ghost"
            size="sm"
            onClick={closeSettings}
          >
            <LuX />
          </IconButton>
        </Flex>

        <Flex flex={1} overflow="hidden">
          {/* Sidebar tabs */}
          <VStack
            w="140px"
            flexShrink={0}
            borderRightWidth="1px"
            borderColor="border"
            p={2}
            gap={1}
            align="stretch"
            justify="space-between"
          >
            <VStack gap={1} align="stretch">
              {settingsTabs.map(renderTabItem)}
              <Separator my={1} />
              {advancedTabs.map(renderTabItem)}
            </VStack>
            <VStack gap={1} align="stretch">
              <Separator my={1} />
              {infoTabs.map(renderTabItem)}
            </VStack>
          </VStack>

          {/* Content */}
          <Box flex={1} overflow="auto" p={4}>
            {activeTab === "general" && <GeneralSection />}
            {activeTab === "theme" && <ThemeSection />}
            {activeTab === "editor" && <EditorSection />}
            {activeTab === "shortcuts" && <ShortcutsSection />}
            {activeTab === "audit" && <AuditSection />}
            {activeTab === "about" && <AboutSection />}
          </Box>
        </Flex>
      </Box>
    </Portal>
  );
}
