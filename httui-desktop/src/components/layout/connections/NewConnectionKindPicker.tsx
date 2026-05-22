// Kind picker sidebar (220px) for the new-connection modal.
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
  /** Locks the picker in edit mode — driver can't change post-create. */
  disabled?: boolean;
  /** Header copy varies by mode. */
  mode?: "create" | "edit";
}

export function NewConnectionKindPicker({
  selectedKind,
  onSelectKind,
  disabled = false,
  mode = "create",
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
            {mode === "edit" ? "Edit connection" : "New connection"}
          </Text>
          <Text fontSize="11px" color="fg.muted" mt={0.5}>
            {mode === "edit" ? "Driver locked" : "Pick the kind"}
          </Text>
        </Box>

        <Stack gap={0.5} align="stretch">
          {CONNECTION_KIND_ORDER.map((kind) => {
            const meta = CONNECTION_KINDS[kind];
            const selected = selectedKind === kind;
            const isDisabled = disabled && !selected;
            return (
              <KindRowButton
                key={kind}
                type="button"
                data-testid={`new-connection-kind-${kind}`}
                data-selected={selected ? "true" : "false"}
                data-disabled={isDisabled ? "true" : undefined}
                aria-pressed={selected}
                aria-disabled={isDisabled}
                onClick={() => {
                  if (isDisabled) return;
                  onSelectKind(kind);
                }}
                display="flex"
                alignItems="center"
                gap={2}
                px={2}
                py="6px"
                borderRadius="6px"
                bg={selected ? "bg.emphasized" : "transparent"}
                borderLeftWidth="2px"
                borderLeftStyle="solid"
                borderLeftColor={selected ? "brand.fg" : "transparent"}
                cursor={isDisabled ? "not-allowed" : "pointer"}
                opacity={isDisabled ? 0.4 : 1}
                textAlign="left"
                border="none"
                _hover={
                  isDisabled
                    ? undefined
                    : { bg: selected ? "bg.emphasized" : "bg.muted" }
                }
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
