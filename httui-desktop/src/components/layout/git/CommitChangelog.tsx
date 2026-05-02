// Epic 55 Story 03 — AI-generated commit changelog panel.
//
// Pure presentational. The consumer (Epic 48 commit dialog) owns the
// AI prompt + streaming pipeline, the `[ai] commit_changelog` user
// preference, the dismiss/regenerate toggle, and the `tab`-to-insert
// keyboard wiring (the panel marks each row as a button so a focused
// row already accepts Enter/Space; the consumer can map Tab to
// `.click()` if it owns the dialog focus chain).
//
// Visual spec from `flow.jsx FlowSave` + Epic 55 Story 03:
// - `bg.2` background, 6px radius, 14px padding
// - Header: 🤖 (accent) + "Auto-generated changelog" weight 600 +
//   flex spacer + `tab` kbd badge + dismiss × button
// - Body: bulleted list, 12px, fg.1, line-height 1.5
// - Each item: `• <blockId mono>: <text>`

import { Box, Flex, Text } from "@chakra-ui/react";
import { Kbd } from "@/components/atoms";

export interface CommitChangelogEntry {
  /** Block alias / id (e.g. "b06"); rendered mono. */
  blockId: string;
  /** One-line description (e.g. "POST → PATCH + Authorization"). */
  text: string;
}

export interface CommitChangelogProps {
  /** Streamed entries. Empty array + `loading=false` shows the
   *  empty state. */
  entries: ReadonlyArray<CommitChangelogEntry>;
  /** True while the AI sidecar is streaming entries. */
  loading?: boolean;
  /** Sidecar error to surface — when set, replaces the body copy. */
  error?: string | null;
  /** Click on an entry inserts that line into the commit description.
   *  Rows are rendered as buttons so Enter/Space and Tab-then-Enter
   *  also fire this. Omit to render entries as plain text. */
  onAccept?: (entry: CommitChangelogEntry) => void;
  /** Dismiss × button. Consumer persists the preference. */
  onDismiss?: () => void;
}

const EMPTY_HINT = "No block-level changes detected.";
const LOADING_HINT = "Generating changelog…";

export function CommitChangelog({
  entries,
  loading,
  error,
  onAccept,
  onDismiss,
}: CommitChangelogProps) {
  return (
    <Box
      data-testid="commit-changelog"
      data-state={
        error ? "error" : loading ? "loading" : entries.length ? "ready" : "empty"
      }
      bg="bg.muted"
      borderRadius="6px"
      p="14px"
    >
      <Flex align="center" gap={2} mb={2}>
        <Text
          as="span"
          fontSize="14px"
          color="accent"
          flexShrink={0}
          aria-hidden
        >
          🤖
        </Text>
        <Text
          as="span"
          fontSize="12px"
          fontWeight={600}
          color="fg"
          flexShrink={0}
          data-testid="commit-changelog-title"
        >
          Auto-generated changelog
        </Text>
        <Box flex={1} />
        <Kbd data-testid="commit-changelog-tab-hint">tab</Kbd>
        {onDismiss && (
          <Box
            as="button"
            type="button"
            data-testid="commit-changelog-dismiss"
            aria-label="Dismiss AI changelog"
            onClick={onDismiss}
            ml={1}
            px={1}
            py={0}
            fontSize="14px"
            lineHeight={1}
            color="fg.muted"
            bg="transparent"
            border="none"
            cursor="pointer"
            _hover={{ color: "fg" }}
          >
            ×
          </Box>
        )}
      </Flex>
      {error ? (
        <Text
          fontSize="12px"
          color="error"
          lineHeight={1.5}
          data-testid="commit-changelog-error"
        >
          {error}
        </Text>
      ) : loading ? (
        <Text
          fontSize="12px"
          color="fg.muted"
          lineHeight={1.5}
          data-testid="commit-changelog-loading"
        >
          {LOADING_HINT}
        </Text>
      ) : entries.length === 0 ? (
        <Text
          fontSize="12px"
          color="fg.muted"
          lineHeight={1.5}
          data-testid="commit-changelog-empty"
        >
          {EMPTY_HINT}
        </Text>
      ) : (
        <Box as="ul" listStyleType="none" m={0} p={0}>
          {entries.map((entry, i) => (
            <ChangelogRow
              key={`${entry.blockId}-${i}`}
              entry={entry}
              onAccept={onAccept}
            />
          ))}
        </Box>
      )}
    </Box>
  );
}

interface ChangelogRowProps {
  entry: CommitChangelogEntry;
  onAccept?: (entry: CommitChangelogEntry) => void;
}

function ChangelogRow({ entry, onAccept }: ChangelogRowProps) {
  const interactive = typeof onAccept === "function";
  return (
    <Box
      as={interactive ? "button" : "li"}
      type={interactive ? "button" : undefined}
      role={interactive ? undefined : "listitem"}
      onClick={interactive ? () => onAccept!(entry) : undefined}
      data-testid="commit-changelog-row"
      data-block-id={entry.blockId}
      display="flex"
      gap={1}
      width="100%"
      textAlign="left"
      bg="transparent"
      border="none"
      p={0}
      mb={1}
      cursor={interactive ? "pointer" : "default"}
      fontSize="12px"
      color="fg.1"
      lineHeight={1.5}
      _hover={interactive ? { color: "fg", bg: "bg.emphasized" } : undefined}
    >
      <Text as="span" flexShrink={0} aria-hidden>
        •
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        flexShrink={0}
        color="fg"
        data-testid="commit-changelog-row-id"
      >
        {entry.blockId}:
      </Text>
      <Text as="span" data-testid="commit-changelog-row-text">
        {entry.text}
      </Text>
    </Box>
  );
}
