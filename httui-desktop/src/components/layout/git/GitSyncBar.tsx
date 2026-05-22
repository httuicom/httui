import { Box, Button, Flex, HStack, Text } from "@chakra-ui/react";
import { LuCircleAlert, LuRefreshCw } from "react-icons/lu";

import { formatGitError } from "@/lib/blocks/git-error";
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
          {error ? "Retry sync" : STEP_LABEL[step]}
        </Button>
        {error && (
          <HStack gap={1} data-testid="git-sync-error" color="error">
            <LuCircleAlert size={12} />
            <Text fontSize="10px" fontFamily="mono">
              {failedStep} failed
            </Text>
          </HStack>
        )}
        {!error && step === "done" && (
          <Text data-testid="git-sync-done" fontSize="10px" color="fg.subtle">
            up to date
          </Text>
        )}
      </Flex>

      {error && <GitSyncError raw={error} />}
    </Box>
  );
}

function GitSyncError({ raw }: { raw: string }) {
  const { summary, detail } = formatGitError(raw);
  return (
    <Box mt={2} data-testid="git-sync-error-block">
      <Text
        data-testid="git-sync-error-summary"
        fontSize="11px"
        fontWeight="600"
        color="error"
        mb={1}
      >
        {summary}
      </Text>
      <Box
        as="pre"
        data-testid="git-sync-error-detail"
        m={0}
        px={2}
        py={2}
        maxH="140px"
        overflow="auto"
        bg="bg.muted"
        borderWidth="1px"
        borderColor="border"
        borderRadius="4px"
        fontFamily="mono"
        fontSize="10px"
        color="fg.muted"
        whiteSpace="pre-wrap"
        css={{ wordBreak: "break-word" }}
      >
        {detail}
      </Box>
    </Box>
  );
}
