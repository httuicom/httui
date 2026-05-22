import { Box, Text, chakra } from "@chakra-ui/react";
import { LuLink, LuTriangleAlert } from "react-icons/lu";

import { Dot, StatusBarShell } from "@/components/atoms";
import { BranchMenu } from "@/components/layout/BranchMenu";
import { EnvSwitcher } from "@/components/layout/EnvSwitcher";
import { ShareMenu } from "@/components/layout/ShareMenu";
import { useGitBranchActions } from "@/hooks/useGitBranchActions";
import { useGitStatus } from "@/hooks/useGitStatus";
import { usePendingSecretsStore } from "@/stores/pendingSecrets";
import { useWorkspaceStore } from "@/stores/workspace";

const PendingButton = chakra("button");

// `__APP_VERSION__` injected by `vite.config.ts` `define`. Tests run
// outside Vite — fall back to "dev" if not defined.
const APP_VERSION =
  typeof __APP_VERSION__ === "string" ? __APP_VERSION__ : "dev";

interface StatusBarProps {
  /** Optional cursor position override; tests use this. Defaults to
   * a placeholder until CM6 selection event wiring lands. */
  cursorLine?: number;
  cursorCol?: number;
  /** Whether the active block has a `{{ref}}` chained reference.
   * Default `false`; placeholder until block context is reachable. */
  chained?: boolean;
}

export function StatusBar({
  cursorLine = 1,
  cursorCol = 1,
  chained = false,
}: StatusBarProps = {}) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const { status: gitState } = useGitStatus(vaultPath);
  const branchActions = useGitBranchActions(vaultPath);
  const activeConnection = useWorkspaceStore((s) => s.activeConnection);
  const pendingSecretsCount = usePendingSecretsStore((s) => s.pending.length);
  const pendingModalOpen = usePendingSecretsStore((s) => s.modalOpen);
  const reopenPendingSecrets = usePendingSecretsStore((s) => s.reopen);

  const ahead = gitState?.ahead ?? 0;
  const behind = gitState?.behind ?? 0;
  // Status codes follow `git status --short`. Untracked counts as "added".
  let added = 0;
  let modified = 0;
  let deleted = 0;
  for (const c of gitState?.changed ?? []) {
    const code = c.status.trim().charAt(0);
    if (c.untracked || code === "A" || code === "?") added += 1;
    else if (code === "D") deleted += 1;
    else modified += 1;
  }

  return (
    <StatusBarShell data-testid="status-bar">
      <BranchMenu
        branch={gitState?.branch ?? null}
        ahead={ahead}
        behind={behind}
        added={added}
        modified={modified}
        deleted={deleted}
        branches={branchActions.branches}
        branchesBusy={branchActions.busy}
        onMenuOpen={branchActions.loadBranches}
        onSelectBranch={branchActions.selectBranch}
        onCreateBranch={branchActions.createBranch}
      />

      <ShareMenu vaultPath={vaultPath} />

      <Box w="1px" h="12px" bg="border" aria-hidden />

      <EnvSwitcher />

      {activeConnection && (
        <>
          <Box w="1px" h="12px" bg="border" aria-hidden />
          <Box
            display="inline-flex"
            gap={2}
            alignItems="center"
            data-testid="status-conn"
          >
            <Dot
              variant={activeConnection.status === "connected" ? "ok" : "err"}
            />
            <Text>{activeConnection.name}</Text>
          </Box>
        </>
      )}

      {pendingSecretsCount > 0 && !pendingModalOpen && (
        <>
          <Box w="1px" h="12px" bg="border" aria-hidden />
          <PendingButton
            type="button"
            data-testid="status-pending-secrets"
            onClick={reopenPendingSecrets}
            display="inline-flex"
            alignItems="center"
            gap={1.5}
            bg="transparent"
            color="orange.400"
            cursor="pointer"
            fontSize="11px"
            fontFamily="mono"
            _hover={{ color: "orange.300" }}
          >
            <LuTriangleAlert size={11} aria-hidden />
            <Text>
              {pendingSecretsCount} secret
              {pendingSecretsCount === 1 ? "" : "s"} pendente
              {pendingSecretsCount === 1 ? "" : "s"}
            </Text>
          </PendingButton>
        </>
      )}

      <Box flex={1} />

      {chained && (
        <Box
          color="brand.fg"
          data-testid="status-chained"
          title="Chained"
          display="inline-flex"
          alignItems="center"
          gap={1}
        >
          <LuLink size={11} aria-hidden />
          <Text>chained</Text>
        </Box>
      )}
      <Text data-testid="status-cursor">
        Ln {cursorLine}, Col {cursorCol}
      </Text>
      <Text data-testid="status-encoding">UTF-8</Text>
      <Box
        data-testid="status-version"
        px={2}
        h="16px"
        display="inline-flex"
        alignItems="center"
        borderRadius="3px"
        bg="bg.muted"
        color="fg.muted"
        fontSize="10px"
      >
        v{APP_VERSION}
      </Box>
    </StatusBarShell>
  );
}
