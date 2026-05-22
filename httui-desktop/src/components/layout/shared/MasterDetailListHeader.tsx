import { Box, Flex, HStack, Heading } from "@chakra-ui/react";
import type { ReactNode } from "react";

export interface MasterDetailListHeaderProps {
  title: string;
  /** Renders directly under the H1 (e.g. status counts, resolution hint). */
  subtitleSlot?: ReactNode;
  /** Right-aligned action buttons. */
  actionsSlot?: ReactNode;
  testId?: string;
}

export function MasterDetailListHeader({
  title,
  subtitleSlot,
  actionsSlot,
  testId,
}: MasterDetailListHeaderProps) {
  return (
    <Flex
      data-testid={testId}
      align="flex-start"
      justify="space-between"
      gap={4}
    >
      <Box minW={0}>
        <Heading
          as="h1"
          fontFamily="serif"
          fontSize="26px"
          fontWeight={500}
          lineHeight={1.1}
          color="fg"
        >
          {title}
        </Heading>
        {subtitleSlot ? <Box mt={1}>{subtitleSlot}</Box> : null}
      </Box>
      {actionsSlot ? (
        <HStack gap={2} flexShrink={0}>
          {actionsSlot}
        </HStack>
      ) : null}
    </Flex>
  );
}
