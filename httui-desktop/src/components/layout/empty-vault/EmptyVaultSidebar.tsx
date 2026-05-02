// Empty-vault sidebar — Epic 41 Story 02 (canvas §3).
//
// 260px sidebar shown on the welcome screen alongside the main
// EmptyVaultScreen content. Five sections:
//   • WORKSPACE label + workspace pill (square avatar + name)
//   • CTA "Novo runbook" (accent bg, plus icon)
//   • RECENTES — empty-state copy
//   • EXPLORAR — Templates / Connections / Variables / Members
//
// Pure presentational. The workspace concept is a placeholder until
// per-machine workspace identity ships (canvas teams.jsx implies a
// "workspace name" separate from per-vault).

import { Box, HStack, Stack, Text, chakra } from "@chakra-ui/react";
import { LuPlus, LuChevronDown } from "react-icons/lu";

import { Btn } from "@/components/atoms";

const WorkspacePill = chakra("button");

export interface EmptyVaultSidebarProps {
  /** Workspace label shown in the pill. Defaults to "default". */
  workspaceName?: string;
  /** Callback for the "Novo runbook" CTA — typically opens the
   * scaffold flow (EmptyVaultScreen's "Create" path). */
  onCreateRunbook: () => void;
  /** Callback for the workspace pill (workspace switcher; placeholder
   * for the future workspace-list popover). */
  onWorkspaceClick?: () => void;
}

interface ExplorerEntry {
  label: string;
  count: number | null;
}

const EXPLORE_ENTRIES: ReadonlyArray<ExplorerEntry> = [
  { label: "Connections", count: 0 },
  { label: "Variables", count: 0 },
  { label: "Members", count: 1 },
];

function SectionLabel({ children }: { children: string }) {
  return (
    <Text
      fontFamily="mono"
      fontSize="11px"
      fontWeight={700}
      letterSpacing="0.08em"
      color="fg.muted"
      textTransform="uppercase"
      data-testid={`section-${children.toLowerCase()}`}
    >
      {children}
    </Text>
  );
}

export function EmptyVaultSidebar({
  workspaceName = "default",
  onCreateRunbook,
  onWorkspaceClick,
}: EmptyVaultSidebarProps) {
  const initial = workspaceName.charAt(0).toUpperCase() || "?";

  return (
    <Stack
      data-atom="empty-vault-sidebar"
      w="260px"
      flexShrink={0}
      px={4}
      py={6}
      gap={6}
      bg="bg.subtle"
      borderRightWidth="1px"
      borderRightColor="border"
    >
      {/* Workspace section */}
      <Stack gap={2}>
        <SectionLabel>WORKSPACE</SectionLabel>
        <WorkspacePill
          type="button"
          data-testid="workspace-pill"
          onClick={onWorkspaceClick}
          h="32px"
          px={2}
          gap={2}
          display="inline-flex"
          alignItems="center"
          bg="bg.muted"
          borderRadius="6px"
          cursor={onWorkspaceClick ? "pointer" : "default"}
          _hover={onWorkspaceClick ? { bg: "bg.emphasized" } : undefined}
        >
          <Box
            aria-hidden
            w="18px"
            h="18px"
            borderRadius="4px"
            bg="oklch(0.74 0.14 50)"
            color="white"
            fontFamily="mono"
            fontSize="10px"
            fontWeight={700}
            display="inline-flex"
            alignItems="center"
            justifyContent="center"
          >
            {initial}
          </Box>
          <Text fontSize="13px" color="fg" flex={1} textAlign="left">
            {workspaceName}
          </Text>
          <LuChevronDown size={10} color="var(--chakra-colors-fg-3)" />
        </WorkspacePill>
        <Btn
          variant="primary"
          data-testid="create-runbook-btn"
          onClick={onCreateRunbook}
          h="32px"
          gap={2}
          fontWeight={600}
        >
          <LuPlus size={11} />
          Novo runbook
        </Btn>
      </Stack>

      {/* Recentes */}
      <Stack gap={2}>
        <SectionLabel>RECENTES</SectionLabel>
        <Text
          fontSize="12px"
          color="fg.subtle"
          lineHeight={1.4}
          data-testid="recentes-empty"
        >
          Vazio. Quando você criar runbooks, eles aparecerão aqui.
        </Text>
      </Stack>

      {/* Explorar */}
      <Stack gap={2}>
        <SectionLabel>EXPLORAR</SectionLabel>
        <Stack gap={1.5}>
          {EXPLORE_ENTRIES.map((entry) => (
            <HStack
              key={entry.label}
              gap={2}
              data-testid={`explore-${entry.label.toLowerCase()}`}
            >
              <Box
                aria-hidden
                w="4px"
                h="4px"
                borderRadius="full"
                bg="fg.subtle"
                flexShrink={0}
              />
              <Text fontSize="12px" color="fg.muted" flex={1}>
                {entry.label}
              </Text>
              {entry.count !== null && (
                <Text fontSize="11px" color="fg.subtle">
                  ({entry.count})
                </Text>
              )}
            </HStack>
          ))}
        </Stack>
      </Stack>
    </Stack>
  );
}
