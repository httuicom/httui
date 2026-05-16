// Epic 48 Story 05 — Fetch / Pull / Push button row.
//
// Pure presentational. Three small buttons that fire callbacks the
// consumer wires to the future `git_fetch` / `git_pull` / `git_push`
// Tauri commands (carries — Story 05's backend slice). Each button
// disables itself while its op is in flight via `inFlight` prop;
// success / error toasts are the consumer's responsibility (we
// just emit the click intent).
//
// Push gates on `hasRemote`: when no remote is configured, the
// Push button surfaces as a disabled button + an inline hint
// pointing to the Epic 49 share popover (consumer wires
// `onConfigureRemote`).

import { Box, Flex, Text } from "@chakra-ui/react";
import { LuArrowDown, LuArrowUp } from "react-icons/lu";

import { Btn } from "@/components/atoms";

export type SyncOp = "fetch" | "pull" | "push";

export interface GitSyncButtonsProps {
  /** Op currently in flight, or null when idle. Disables every
   *  button while non-null so we don't queue duplicate ops. */
  inFlight?: SyncOp | null;
  /** True when at least one remote is configured. Push is disabled
   *  + shows the configure-remote hint when false. */
  hasRemote?: boolean;
  onFetch?: () => void;
  onPull?: () => void;
  onPush?: () => void;
  /** Routes to Epic 49's share/configure flow. Hidden when
   *  hasRemote is true. */
  onConfigureRemote?: () => void;
}

export function GitSyncButtons({
  inFlight,
  hasRemote = true,
  onFetch,
  onPull,
  onPush,
  onConfigureRemote,
}: GitSyncButtonsProps) {
  const busy = inFlight !== null && inFlight !== undefined;

  return (
    <Box
      data-testid="git-sync-buttons"
      data-in-flight={inFlight || undefined}
      data-no-remote={!hasRemote || undefined}
    >
      <Flex gap={2} align="center" px={3} py={2}>
        {onFetch && (
          <Btn
            data-testid="git-sync-fetch"
            data-in-flight={inFlight === "fetch" || undefined}
            variant="ghost"
            disabled={busy || !hasRemote}
            onClick={onFetch}
          >
            <LuArrowDown size={11} aria-hidden />
            {inFlight === "fetch" ? "Fetching…" : "Fetch"}
          </Btn>
        )}
        {onPull && (
          <Btn
            data-testid="git-sync-pull"
            data-in-flight={inFlight === "pull" || undefined}
            variant="ghost"
            disabled={busy || !hasRemote}
            onClick={onPull}
          >
            <LuArrowDown size={11} aria-hidden />
            {inFlight === "pull" ? "Pulling…" : "Pull"}
          </Btn>
        )}
        {onPush && (
          <Btn
            data-testid="git-sync-push"
            data-in-flight={inFlight === "push" || undefined}
            variant="primary"
            disabled={busy || !hasRemote}
            onClick={onPush}
          >
            <LuArrowUp size={11} aria-hidden />
            {inFlight === "push" ? "Pushing…" : "Push"}
          </Btn>
        )}
      </Flex>
      {!hasRemote && onPush && (
        <Box px={3} pb={2}>
          <Text
            as="div"
            data-testid="git-sync-no-remote-hint"
            fontFamily="mono"
            fontSize="10px"
            color="warn"
          >
            No remote configured.
            {onConfigureRemote && (
              <>
                {" "}
                <Text
                  as="button"
                  data-testid="git-sync-configure-remote"
                  fontFamily="mono"
                  fontSize="10px"
                  color="brand.fg"
                  onClick={onConfigureRemote}
                  cursor="pointer"
                >
                  Configure
                </Text>
              </>
            )}
          </Text>
        </Box>
      )}
    </Box>
  );
}
