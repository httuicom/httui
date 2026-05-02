// Status-bar branch picker — replaces the standalone branch button
// in the TopBar. Trigger reads as a status-bar cell (branch label +
// optional ↑↓~ counts); clicking opens a placeholder dropdown until
// V10 wires the real branch switcher.
//
// Pure presentational over `useGitStatus`: the parent passes the
// label and counts in.

import { Box, HStack, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuGitBranch } from "react-icons/lu";

const Trigger = chakra("button");

export interface BranchMenuProps {
  /** Current branch label (e.g. `main`, `feat/login`). `null` shows
   * the dash placeholder. */
  branch: string | null;
  /** Commits ahead of upstream. */
  ahead?: number;
  /** Commits behind upstream. */
  behind?: number;
  /** Worktree changed-files count. */
  changeCount?: number;
}

export function BranchMenu({
  branch,
  ahead = 0,
  behind = 0,
  changeCount = 0,
}: BranchMenuProps) {
  const label = branch ?? "—";
  const hasCounts = ahead > 0 || behind > 0 || changeCount > 0;

  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Trigger
          type="button"
          data-testid="status-branch-trigger"
          data-atom="status-branch-trigger"
          aria-label={`Branch ${label}`}
          bg="transparent"
          color="fg.1"
          fontFamily="mono"
          fontSize="11px"
          cursor="pointer"
          display="inline-flex"
          alignItems="center"
          gap={2}
          px={1}
          flexShrink={0}
          _hover={{ color: "fg" }}
        >
          <LuGitBranch size={11} aria-hidden />
          <Box as="span" data-testid="status-branch">
            {label}
          </Box>
          {hasCounts && (
            <Box as="span" color="fg.3" data-testid="status-changes">
              {ahead > 0 && `↑${ahead} `}
              {behind > 0 && `↓${behind} `}
              {changeCount > 0 && `~${changeCount}`}
            </Box>
          )}
        </Trigger>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            data-testid="branch-menu"
            minW="220px"
            bg="bg"
            borderWidth="1px"
            borderColor="line"
            shadow="2xl"
          >
            <HStack px={3} py={2} gap={2}>
              <LuGitBranch size={12} aria-hidden />
              <Box fontSize="11px" fontFamily="mono">
                {label}
              </Box>
            </HStack>
            <Box
              borderTopWidth="1px"
              borderColor="line"
              px={3}
              py={2}
              fontSize="11px"
              color="fg.3"
              data-testid="branch-menu-placeholder"
            >
              Trocar de branch chega na V10. Por agora veja a branch
              ativa aqui — comandos vão pro Git panel.
            </Box>
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
