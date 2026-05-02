// Epic 48 Story 03 — "audit substitution" header.
//
// The canvas's §9 "Audit log" feature is replaced by the git log
// view; per the epic spec, the panel header should make this
// substitution explicit so users find their audit trail here.

import { Box, Text } from "@chakra-ui/react";

export interface GitAuditHeaderProps {
  /** Optional click handler — wires e.g. "open in external git
   *  client" or "copy to clipboard". Story 03 ships the visual; the
   *  consumer can attach an action later. */
  onLearnMore?: () => void;
}

export function GitAuditHeader({ onLearnMore }: GitAuditHeaderProps) {
  return (
    <Box
      data-testid="git-audit-header"
      px={3}
      py={2}
      bg="bg.muted"
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Text
        fontFamily="mono"
        fontSize="10px"
        color="fg.muted"
        textTransform="uppercase"
      >
        Audit log
      </Text>
      <Text
        as={onLearnMore ? "button" : "div"}
        data-testid="git-audit-header-body"
        fontFamily="mono"
        fontSize="11px"
        color="fg.subtle"
        mt={1}
        textAlign="left"
        onClick={onLearnMore}
        cursor={onLearnMore ? "pointer" : undefined}
      >
        This is your audit log. Every change is a commit.
      </Text>
    </Box>
  );
}
