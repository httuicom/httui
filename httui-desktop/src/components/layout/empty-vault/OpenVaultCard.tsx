// "Open vault" card — V1 vertical 1.
//
// One of the three first-screen cards. Picks an existing folder via
// the OS directory picker; the consumer wires the actual switchVault
// call to the workspace store. Pure presentational + click handler;
// busy/error states live one level up at EmptyVaultScreen.

import { Box, Stack, Text, chakra } from "@chakra-ui/react";

const CardBox = chakra("button");

export interface OpenVaultCardProps {
  /** Click → consumer opens the directory picker. */
  onOpenClick: () => void;
  /** Disable while another card is mid-flow. */
  busy?: boolean;
}

export function OpenVaultCard({
  onOpenClick,
  busy = false,
}: OpenVaultCardProps) {
  return (
    <CardBox
      type="button"
      data-atom="open-vault-card"
      data-testid="open-vault-card"
      onClick={onOpenClick}
      disabled={busy}
      aria-label="Open existing vault"
      textAlign="left"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="12px"
      p="22px"
      minH="260px"
      cursor={busy ? "not-allowed" : "pointer"}
      opacity={busy ? 0.6 : 1}
      _hover={busy ? undefined : { borderColor: "fg.subtle" }}
    >
      <Stack gap={3} h="full" align="stretch">
        <Box
          aria-hidden
          w="32px"
          h="32px"
          borderRadius="6px"
          bg="color-mix(in oklab, oklch(0.62 0.10 230) 14%, transparent)"
          display="inline-flex"
          alignItems="center"
          justifyContent="center"
          color="oklch(0.62 0.10 230)"
          fontSize="16px"
          data-testid="open-vault-icon"
        >
          ▤
        </Box>
        <Text
          fontFamily="var(--chakra-fonts-serif)"
          fontSize="18px"
          fontWeight={600}
          color="fg"
          data-testid="open-vault-title"
        >
          Open vault
        </Text>
        <Text
          fontSize="12px"
          color="fg.muted"
          lineHeight={1.4}
          data-testid="open-vault-body"
        >
          Abra uma pasta existente do seu computador. Funciona com qualquer
          repositório git que já tenha runbooks.
        </Text>
        <Text
          fontSize="11px"
          color="brand.fg"
          mt={1.5}
          fontWeight={600}
          data-testid="open-vault-cta"
        >
          Escolher pasta →
        </Text>
      </Stack>
    </CardBox>
  );
}
