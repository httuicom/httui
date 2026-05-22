import { Box, Flex, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";
import type { IconType } from "react-icons";

export interface SidebarHintCardProps {
  icon: IconType;
  title: string;
  children: ReactNode;
  testId?: string;
}

export function SidebarHintCard({
  icon: Icon,
  title,
  children,
  testId,
}: SidebarHintCardProps) {
  return (
    <Box
      data-testid={testId}
      fontSize="10px"
      lineHeight={1.4}
      color="fg.muted"
      bg="bg.muted"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      p={2.5}
    >
      <Flex align="center" gap={1.5} mb={1}>
        <Icon size={11} aria-hidden />
        <Text as="span" fontWeight={600} color="fg.muted">
          {title}
        </Text>
      </Flex>
      {children}
    </Box>
  );
}
