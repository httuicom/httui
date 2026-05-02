// Breadcrumb `workspace › project › file` — canvas §4.
//
// Last segment is `--fg`, earlier segments `--fg-2`. Trailing 6×6
// `dot-warn` indicator shown when the active tab is dirty. Each
// segment is a button: clicking the workspace switches vault list,
// clicking middle path segments navigates to a parent (no-op for now
// — folders aren't navigable as runbook stand-ins), and clicking the
// active file segment is a no-op (already focused).

import { Box, HStack, Text, chakra } from "@chakra-ui/react";

import { Dot } from "@/components/atoms";

const Segment = chakra("button");

export interface BreadcrumbNavProps {
  /** Vault root display name (basename of `vaultPath`). */
  workspace: string | null;
  /** Active tab's file path, relative or absolute. `null` when no tab is open. */
  filePath: string | null;
  /** Whether the active tab has unsaved edits. */
  unsaved: boolean;
  /** Optional click on the workspace segment (vault picker). */
  onWorkspaceClick?: () => void;
}

function deriveSegments(filePath: string | null): string[] {
  if (!filePath) return [];
  // Drop leading vault path; show only the runbook-relative chain.
  const trimmed = filePath.replace(/^.*?\/runbooks\//, "");
  return trimmed.split("/").filter(Boolean);
}

export function BreadcrumbNav({
  workspace,
  filePath,
  unsaved,
  onWorkspaceClick,
}: BreadcrumbNavProps) {
  const segments = deriveSegments(filePath);

  if (!workspace) {
    return (
      <Text data-atom="breadcrumb" color="fg.3" fontSize="13px">
        no vault
      </Text>
    );
  }

  return (
    <HStack
      data-atom="breadcrumb"
      gap={1}
      fontSize="13px"
      minW={0}
      overflow="hidden"
      flexShrink={1}
    >
      <Segment
        type="button"
        data-segment="workspace"
        onClick={onWorkspaceClick}
        bg="transparent"
        color={segments.length === 0 ? "fg" : "fg.2"}
        fontWeight={500}
        cursor={onWorkspaceClick ? "pointer" : "default"}
        _hover={onWorkspaceClick ? { color: "fg" } : undefined}
        px={1}
        maxWidth="160px"
        overflow="hidden"
        textOverflow="ellipsis"
        whiteSpace="nowrap"
        title={workspace}
        flexShrink={0}
      >
        {workspace}
      </Segment>
      {segments.map((seg, idx) => {
        const isLast = idx === segments.length - 1;
        return (
          <HStack key={`${idx}-${seg}`} gap={1}>
            <Box as="span" aria-hidden color="fg.3" fontSize="12px">
              ›
            </Box>
            <Text
              data-segment={isLast ? "file" : "folder"}
              data-active={isLast ? "true" : "false"}
              color={isLast ? "fg" : "fg.2"}
              fontWeight={isLast ? 500 : 400}
              maxWidth="240px"
              overflow="hidden"
              textOverflow="ellipsis"
              whiteSpace="nowrap"
              title={seg}
            >
              {seg}
            </Text>
            {isLast && unsaved && (
              <Dot variant="warn" data-testid="dirty-indicator" />
            )}
          </HStack>
        );
      })}
    </HStack>
  );
}
