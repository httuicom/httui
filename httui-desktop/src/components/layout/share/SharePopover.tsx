// Share popover.
//
// Pure presentational. Consumer (git panel header) fetches the
// configured remotes via `git remote -v` (Tauri command — already
// available through `git_branch_list_cmd`'s sibling pattern; the
// `list_remotes` cmd carries) and feeds them in as `remotes` prop.
// The popover handles: empty state, single-remote one-click copy,
// multi-remote dropdown picker.
//
// Forge-specific permalink/compare URLs are NOT this popover's job
// — those compose via `lib/share/share-url.ts` (already shipped in
// 3e34bcf) and surface either inside this popover (Stories 02/03
// add actions to the action list) or in the DocHeader action row's
// Share button.

import { useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

export interface RemoteOption {
  /** `origin`, `upstream`, … or V10's `HTTPS` / `SSH` / `Web`. */
  name: string;
  /** Raw URL (`git@host:owner/repo.git` or `https://...`). */
  url: string;
  /** When true the popover offers an "Open" action (browser). */
  openable?: boolean;
}

export interface SharePopoverProps {
  /** Configured remotes. Empty array → "No remote configured" path. */
  remotes: ReadonlyArray<RemoteOption>;
  /** True when a copy is in flight (e.g. clipboard write resolving). */
  copying?: boolean;
  /** Fires with the picked remote's URL after the user clicks Copy.
   *  Consumer is responsible for the actual `clipboard.writeText`
   *  call — keeps this component framework-free. */
  onCopy: (url: string) => void;
  /** Fires when the active option is `openable` and the user clicks
   *  Open. Consumer routes to the Tauri shell opener. */
  onOpen?: (url: string) => void;
  /** Fires when the user clicks the "Configure remote" hint in the
   * empty state — consumer routes to workspace settings. */
  onOpenWorkspaceSettings?: () => void;
}

export function SharePopover({
  remotes,
  copying,
  onCopy,
  onOpen,
  onOpenWorkspaceSettings,
}: SharePopoverProps) {
  const [picked, setPicked] = useState<string | null>(null);

  if (remotes.length === 0) {
    return (
      <Box
        data-testid="share-popover"
        data-state="empty"
        px={4}
        py={3}
        bg="bg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="6px"
        minW="280px"
      >
        <Text fontFamily="mono" fontSize="11px" color="fg.muted">
          No remote configured.
        </Text>
        {onOpenWorkspaceSettings && (
          <Text
            as="button"
            data-testid="share-popover-configure"
            mt={2}
            fontFamily="mono"
            fontSize="11px"
            color="brand.fg"
            onClick={onOpenWorkspaceSettings}
            cursor="pointer"
          >
            Configure a remote
          </Text>
        )}
      </Box>
    );
  }

  // Default the picker to "origin" when present, else the first remote.
  const defaultName =
    remotes.find((r) => r.name === "origin")?.name ?? remotes[0]!.name;
  const activeName = picked ?? defaultName;
  const active = remotes.find((r) => r.name === activeName) ?? remotes[0]!;

  return (
    <Box
      data-testid="share-popover"
      data-state="ready"
      data-remote-count={remotes.length}
      px={4}
      py={3}
      bg="bg.subtle"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      minW="320px"
    >
      <Text
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        textTransform="uppercase"
        mb={2}
      >
        Share repo URL
      </Text>
      {remotes.length > 1 && (
        <Flex gap={1} flexWrap="wrap" mb={2}>
          {remotes.map((r) => (
            <Btn
              key={r.name}
              variant="ghost"
              data-testid={`share-popover-remote-${r.name}`}
              data-active={r.name === activeName || undefined}
              onClick={() => setPicked(r.name)}
            >
              {r.name}
            </Btn>
          ))}
        </Flex>
      )}
      <Text
        data-testid="share-popover-url"
        as="div"
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        mb={2}
        truncate
        title={active.url}
      >
        {active.url}
      </Text>
      <Flex gap={2}>
        <Btn
          data-testid="share-popover-copy"
          variant="primary"
          disabled={copying}
          onClick={() => onCopy(active.url)}
        >
          {copying ? "Copying…" : "Copy URL"}
        </Btn>
        {onOpen && active.openable && (
          <Btn
            data-testid="share-popover-open"
            variant="ghost"
            onClick={() => onOpen(active.url)}
          >
            Open
          </Btn>
        )}
      </Flex>
    </Box>
  );
}
