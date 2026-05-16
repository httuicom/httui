// V10.1 cenário 3 — Sync button + per-step progress + the no-
// upstream confirm. Pure presentational over `useGitSync` so the
// orchestration stays testable on its own. Never a Dialog (CM6
// focus). Icons only, no emoji glyphs.

import { Box, Button, Flex, HStack, Text } from "@chakra-ui/react";
import { LuCircleAlert, LuRefreshCw } from "react-icons/lu";

import type { SyncStep, UseGitSyncResult } from "@/hooks/useGitSync";

const STEP_LABEL: Record<SyncStep, string> = {
  idle: "Sync",
  staging: "Staging…",
  committing: "Committing…",
  pulling: "Pulling…",
  pushing: "Pushing…",
  done: "Synced",
};

export type GitSyncBarProps = UseGitSyncResult;

export function GitSyncBar({
  step,
  error,
  failedStep,
  upstreamPrompt,
  busy,
  sync,
  confirmSetUpstream,
  cancelSetUpstream,
}: GitSyncBarProps) {
  if (upstreamPrompt) {
    return (
      <Box
        data-testid="git-sync-upstream-prompt"
        px={3}
        py={2}
        borderTopWidth="1px"
        borderTopColor="border"
        bg="bg.subtle"
      >
        <Text fontSize="11px" color="fg.muted" mb={2}>
          Branch{" "}
          <Text as="span" fontFamily="mono" color="fg">
            {upstreamPrompt.branch}
          </Text>{" "}
          has no upstream on{" "}
          <Text as="span" fontFamily="mono" color="fg">
            {upstreamPrompt.remote}
          </Text>
          .
        </Text>
        <HStack gap={2}>
          <Button
            data-testid="git-sync-upstream-confirm"
            size="xs"
            variant="solid"
            onClick={confirmSetUpstream}
          >
            Set upstream &amp; push
          </Button>
          <Button
            data-testid="git-sync-upstream-cancel"
            size="xs"
            variant="ghost"
            onClick={cancelSetUpstream}
          >
            Cancel
          </Button>
        </HStack>
      </Box>
    );
  }

  return (
    <Box
      data-testid="git-sync-bar"
      data-step={step}
      px={3}
      py={2}
      borderTopWidth="1px"
      borderTopColor="border"
      bg="bg.subtle"
    >
      <Flex align="center" gap={2}>
        <Button
          data-testid="git-sync-button"
          size="xs"
          variant="solid"
          disabled={busy}
          onClick={sync}
        >
          <LuRefreshCw />
          {STEP_LABEL[step]}
        </Button>
        {error && (
          <HStack gap={1} data-testid="git-sync-error" color="error">
            <LuCircleAlert size={12} />
            <Text fontSize="10px">
              {failedStep} failed: {error}
            </Text>
          </HStack>
        )}
        {!error && step === "done" && (
          <Text data-testid="git-sync-done" fontSize="10px" color="fg.subtle">
            up to date
          </Text>
        )}
      </Flex>
    </Box>
  );
}
