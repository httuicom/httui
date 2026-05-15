// Status-bar branch picker — replaces the standalone branch button
// in the TopBar. Trigger reads as a status-bar cell (branch label +
// optional `↑a ↓b +n ~m -d` counts); clicking opens a placeholder
// dropdown until V10 wires the real branch switcher.
//
// Pure presentational over `useGitStatus`: the parent passes the
// label and parsed counts in.

import { Box, HStack, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuGitBranch } from "react-icons/lu";

import { GitBranchPicker } from "@/components/layout/git/GitBranchPicker";
import type { BranchInfo } from "@/lib/tauri/git";

const Trigger = chakra("button");

export interface BranchMenuProps {
  /** Current branch label (e.g. `main`, `feat/login`). `null` shows
   * the dash placeholder. */
  branch: string | null;
  /** Commits ahead of upstream. */
  ahead?: number;
  /** Commits behind upstream. */
  behind?: number;
  /** New / untracked files in the worktree (`+N`). */
  added?: number;
  /** Modified files in the worktree (`~M`). */
  modified?: number;
  /** Deleted files in the worktree (`-D`). */
  deleted?: number;
  // --- Branch switcher (V10 cenário 4) ---
  /** Branch list for the dropdown picker. When `onSelectBranch` is
   * absent the menu falls back to the read-only placeholder. */
  branches?: ReadonlyArray<BranchInfo>;
  branchesBusy?: boolean;
  /** Called when the menu opens — consumer lazy-loads branches. */
  onMenuOpen?: () => void;
  onSelectBranch?: (branch: BranchInfo) => void;
  onCreateBranch?: (name: string) => void;
}

export function BranchMenu({
  branch,
  ahead = 0,
  behind = 0,
  added = 0,
  modified = 0,
  deleted = 0,
  branches,
  branchesBusy,
  onMenuOpen,
  onSelectBranch,
  onCreateBranch,
}: BranchMenuProps) {
  const label = branch ?? "—";
  const hasCounts =
    ahead > 0 || behind > 0 || added > 0 || modified > 0 || deleted > 0;
  const switcher = !!onSelectBranch;

  return (
    <Menu.Root
      onOpenChange={(e) => {
        if (e.open) onMenuOpen?.();
      }}
    >
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
            <Box as="span" color="fg.subtle" data-testid="status-changes">
              {ahead > 0 && `↑${ahead} `}
              {behind > 0 && `↓${behind} `}
              {added > 0 && `+${added} `}
              {modified > 0 && `~${modified} `}
              {deleted > 0 && `-${deleted}`}
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
            borderColor="border"
            shadow="2xl"
          >
            <HStack px={3} py={2} gap={2}>
              <LuGitBranch size={12} aria-hidden />
              <Box fontSize="11px" fontFamily="mono">
                {label}
              </Box>
            </HStack>
            {switcher ? (
              <Box
                borderTopWidth="1px"
                borderColor="border"
                data-testid="branch-menu-switcher"
              >
                <GitBranchPicker
                  branches={branches ?? []}
                  busy={branchesBusy}
                  onSelectBranch={onSelectBranch}
                  onCreateBranch={onCreateBranch}
                />
              </Box>
            ) : (
              <Box
                borderTopWidth="1px"
                borderColor="border"
                px={3}
                py={2}
                fontSize="11px"
                color="fg.subtle"
                data-testid="branch-menu-placeholder"
              >
                Trocar de branch chega na V10. Por agora veja a branch
                ativa aqui — comandos vão pro Git panel.
              </Box>
            )}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
