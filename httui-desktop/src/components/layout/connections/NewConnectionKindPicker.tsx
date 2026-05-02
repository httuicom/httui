// Canvas §5 — "Nova conexão" modal sidebar pick-kind (Epic 42 Story 06).
//
// 220px column on the left of the modal. Header "Nova conexão" serif
// 16 + sub "Escolha o tipo". Lists all 9 kinds from `connection-kinds.ts`
// in canvas order; the active row gets `bg.3` background + accent left
// border so the modal mirrors the page sidebar idiom without showing
// counts (no existing-connection list inside the modal).
//
// Pure presentational — selection lifted to the consumer.

import { Box, Stack, Text, chakra } from "@chakra-ui/react";

import { ConnectionKindIcon } from "./ConnectionKindIcon";
import {
  CONNECTION_KIND_ORDER,
  CONNECTION_KINDS,
  type ConnectionKind,
} from "./connection-kinds";

const KindRowButton = chakra("button");

export interface NewConnectionKindPickerProps {
  selectedKind: ConnectionKind;
  onSelectKind: (kind: ConnectionKind) => void;
}

export function NewConnectionKindPicker({
  selectedKind,
  onSelectKind,
}: NewConnectionKindPickerProps) {
  return (
    <Box
      data-testid="new-connection-kind-picker"
      w="220px"
      h="full"
      borderRightWidth="1px"
      borderRightColor="border"
      bg="bg.subtle"
      overflowY="auto"
      p={4}
    >
      <Stack gap={3} align="stretch">
        <Box>
          <Text
            fontFamily="serif"
            fontSize="16px"
            fontWeight={500}
            color="fg"
            lineHeight={1.2}
          >
            Nova conexão
          </Text>
          <Text fontSize="11px" color="fg.muted" mt={0.5}>
            Escolha o tipo
          </Text>
        </Box>

        <Stack gap={0.5} align="stretch">
          {CONNECTION_KIND_ORDER.map((kind) => {
            const meta = CONNECTION_KINDS[kind];
            const selected = selectedKind === kind;
            return (
              <KindRowButton
                key={kind}
                type="button"
                data-testid={`new-connection-kind-${kind}`}
                data-selected={selected ? "true" : "false"}
                aria-pressed={selected}
                onClick={() => onSelectKind(kind)}
                display="flex"
                alignItems="center"
                gap={2}
                px={2}
                py="6px"
                borderRadius="6px"
                bg={selected ? "bg.emphasized" : "transparent"}
                borderLeftWidth="2px"
                borderLeftStyle="solid"
                borderLeftColor={selected ? "accent" : "transparent"}
                cursor="pointer"
                textAlign="left"
                border="none"
                _hover={{ bg: selected ? "bg.emphasized" : "bg.muted" }}
              >
                <ConnectionKindIcon kind={kind} size={18} />
                <Text
                  flex={1}
                  fontSize="13px"
                  fontWeight={selected ? 600 : 500}
                  color="fg"
                  truncate
                >
                  {meta.label}
                </Text>
              </KindRowButton>
            );
          })}
        </Stack>
      </Stack>
    </Box>
  );
}
