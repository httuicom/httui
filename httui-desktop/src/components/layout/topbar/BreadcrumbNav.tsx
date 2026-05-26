// Breadcrumb `workspace › project › file` — canvas §4.
//
// Last segment is `--fg`, earlier segments `--fg-2`. Trailing 6×6
// `dot-warn` indicator shown when the active tab is dirty. Each
// segment is a button: clicking the workspace switches vault list,
// clicking middle path segments navigates to a parent (no-op for now
// — folders aren't navigable as runbook stand-ins), and clicking the
// active file segment is a no-op (already focused).

import { Box, HStack, Text, chakra } from "@chakra-ui/react";
import type { ReactNode } from "react";

import { Dot } from "@/components/atoms";

const Segment = chakra("button");

export interface BreadcrumbNavProps {
  /** Vault root display name (basename of `vaultPath`). */
  workspace: string | null;
  /** Active tab's file path, relative or absolute. `null` when no tab is open. */
  filePath: string | null;
  /** Whether the active tab has unsaved edits. */
  unsaved: boolean;
  /** Optional click on the workspace segment (vault picker). Ignored
   *  when `workspaceSlot` is provided. */
  onWorkspaceClick?: () => void;
  /** Optional override for the workspace segment. When provided,
   *  replaces the plain button (e.g. with a vault-picker dropdown).
   *  Receives no props — render with knowledge of `workspace` /
   *  `isLeaf` from the consumer. */
  workspaceSlot?: ReactNode;
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
  workspaceSlot,
}: BreadcrumbNavProps) {
  const segments = deriveSegments(filePath);

  if (!workspace && !workspaceSlot) {
    return (
      <Text data-atom="breadcrumb" color="fg.subtle" fontSize="13px">
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
      {workspaceSlot ?? (
        <Segment
          type="button"
          data-segment="workspace"
          onClick={onWorkspaceClick}
          bg="transparent"
          color={segments.length === 0 ? "fg" : "fg.muted"}
          fontWeight={500}
          cursor={onWorkspaceClick ? "pointer" : "default"}
          _hover={onWorkspaceClick ? { color: "fg" } : undefined}
          px={1}
          maxWidth="160px"
          overflow="hidden"
          textOverflow="ellipsis"
          whiteSpace="nowrap"
          title={workspace ?? undefined}
          flexShrink={0}
        >
          {workspace}
        </Segment>
      )}
      {segments.map((seg, idx) => {
        const isLast = idx === segments.length - 1;
        return (
          <HStack key={`${idx}-${seg}`} gap={1}>
            <Box as="span" aria-hidden color="fg.subtle" fontSize="12px">
              ›
            </Box>
            <Text
              data-segment={isLast ? "file" : "folder"}
              data-active={isLast ? "true" : "false"}
              color={isLast ? "fg" : "fg.muted"}
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
