import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

export interface GitConflictBannerProps {
  /** Vault-relative paths reported as unmerged by `git_status`. */
  conflicts: ReadonlyArray<string>;
  /** True while a resolution operation is in flight (accepting one
   *  side or marking resolved). Disables every button. */
  busy?: boolean;
  /** Opens the conflicted file in the existing DiffViewer with
   *  theirs/yours panels. */
  onOpenDiff?: (path: string) => void;
  /** Routes to `git checkout --ours <path>`. */
  onAcceptYours?: (path: string) => void;
  /** Routes to `git checkout --theirs <path>`. */
  onAcceptTheirs?: (path: string) => void;
}

export function GitConflictBanner({
  conflicts,
  busy,
  onOpenDiff,
  onAcceptYours,
  onAcceptTheirs,
}: GitConflictBannerProps) {
  if (conflicts.length === 0) return null;

  const noun = conflicts.length === 1 ? "conflict" : "conflicts";

  return (
    <Box
      data-testid="git-conflict-banner"
      data-busy={busy || undefined}
      data-count={conflicts.length}
      borderWidth="1px"
      borderColor="error"
      borderRadius="6px"
      bg="bg.subtle"
      p={3}
      mb={2}
    >
      <Text
        fontFamily="mono"
        fontSize="11px"
        fontWeight={600}
        color="error"
        mb={2}
      >
        {conflicts.length} {noun} to resolve
      </Text>
      <Flex direction="column" gap={1}>
        {conflicts.map((path) => (
          <ConflictRow
            key={path}
            path={path}
            busy={busy}
            onOpenDiff={onOpenDiff}
            onAcceptYours={onAcceptYours}
            onAcceptTheirs={onAcceptTheirs}
          />
        ))}
      </Flex>
    </Box>
  );
}

function ConflictRow({
  path,
  busy,
  onOpenDiff,
  onAcceptYours,
  onAcceptTheirs,
}: {
  path: string;
  busy?: boolean;
  onOpenDiff?: (path: string) => void;
  onAcceptYours?: (path: string) => void;
  onAcceptTheirs?: (path: string) => void;
}) {
  return (
    <Flex
      data-testid={`git-conflict-row-${path}`}
      align="center"
      gap={2}
      px={2}
      py={1}
      bg="bg.muted"
      borderRadius="4px"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        flex={1}
        truncate
        title={path}
      >
        {path}
      </Text>
      {onOpenDiff && (
        <Btn
          data-testid={`git-conflict-row-${path}-resolve`}
          variant="ghost"
          disabled={busy}
          onClick={() => onOpenDiff(path)}
        >
          Resolve…
        </Btn>
      )}
      {onAcceptYours && (
        <Btn
          data-testid={`git-conflict-row-${path}-accept-yours`}
          variant="ghost"
          disabled={busy}
          onClick={() => onAcceptYours(path)}
        >
          Accept yours
        </Btn>
      )}
      {onAcceptTheirs && (
        <Btn
          data-testid={`git-conflict-row-${path}-accept-theirs`}
          variant="ghost"
          disabled={busy}
          onClick={() => onAcceptTheirs(path)}
        >
          Accept theirs
        </Btn>
      )}
    </Flex>
  );
}
