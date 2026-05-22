import { Box, HStack, Text } from "@chakra-ui/react";

import { Kbd } from "@/components/atoms";

export function EmptyVaultFooter() {
  return (
    <HStack
      data-atom="empty-vault-footer"
      data-testid="empty-vault-footer"
      gap="14px"
      mt="36px"
      justify="center"
      color="fg.muted"
      fontSize="12px"
    >
      <HStack gap={2}>
        <Text>ou cole uma URL</Text>
        <Kbd>⌘V</Kbd>
        <Text>e geramos o bloco</Text>
      </HStack>
      <Box aria-hidden color="fg.subtle">
        |
      </Box>
      <Text
        data-testid="empty-vault-tour"
        opacity={0.6}
        cursor="not-allowed"
        title="Coming in v1.x"
      >
        ▶ Tour interativo (2 min)
      </Text>
    </HStack>
  );
}
