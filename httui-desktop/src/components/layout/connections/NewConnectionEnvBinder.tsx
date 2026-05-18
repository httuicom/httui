// Canvas §5 — "Vincular ao ambiente" pills.
//
// Horizontal strip of env pills. Each entry is selectable; the active
// state gets accent-soft bg + accent border (canvas spec). A read-only
// env (e.g. prod under a guard) shows "(read-only)" inline and stays
// selectable but with a softer affordance — the page consumer enforces
// "no writes" elsewhere; the pill is purely visual. A trailing
// "+ novo" dashed pill dispatches `onCreateNew` when provided.
//
// Pure presentational — selection set lifted to the consumer.

import { Box, HStack, Text, chakra } from "@chakra-ui/react";

const PillButton = chakra("button");

export interface EnvBinderEntry {
  id: string;
  name: string;
  readOnly?: boolean;
}

export interface NewConnectionEnvBinderProps {
  envs: ReadonlyArray<EnvBinderEntry>;
  selectedIds: ReadonlyArray<string>;
  onToggle: (id: string) => void;
  onCreateNew?: () => void;
}

export function NewConnectionEnvBinder({
  envs,
  selectedIds,
  onToggle,
  onCreateNew,
}: NewConnectionEnvBinderProps) {
  const selected = new Set(selectedIds);
  return (
    <Box data-testid="new-connection-env-binder">
      <Text
        fontFamily="mono"
        fontSize="11px"
        fontWeight="bold"
        letterSpacing="0.08em"
        textTransform="uppercase"
        color="fg.muted"
        mb={2}
      >
        Vincular ao ambiente
      </Text>
      <HStack gap={1.5} wrap="wrap">
        {envs.map((env) => {
          const active = selected.has(env.id);
          return (
            <PillButton
              key={env.id}
              type="button"
              data-testid={`new-connection-env-pill-${env.id}`}
              data-active={active ? "true" : "false"}
              data-readonly={env.readOnly ? "true" : "false"}
              aria-pressed={active}
              onClick={() => onToggle(env.id)}
              display="inline-flex"
              alignItems="center"
              gap={1.5}
              h="22px"
              px="10px"
              borderRadius="999px"
              borderWidth="1px"
              borderStyle="solid"
              borderColor={active ? "brand.fg" : "border"}
              bg={active ? "brand.subtle" : "transparent"}
              color={active ? "fg" : "fg.muted"}
              fontSize="11px"
              fontWeight={active ? 600 : 500}
              cursor="pointer"
              _hover={{
                borderColor: active ? "brand.fg" : "fg.subtle",
                color: "fg",
              }}
            >
              <Text as="span">{env.name}</Text>
              {env.readOnly && (
                <Text
                  as="span"
                  fontFamily="mono"
                  fontSize="10px"
                  color="fg.subtle"
                >
                  (read-only)
                </Text>
              )}
            </PillButton>
          );
        })}
        {onCreateNew && (
          <PillButton
            type="button"
            data-testid="new-connection-env-pill-new"
            onClick={onCreateNew}
            display="inline-flex"
            alignItems="center"
            gap={1}
            h="22px"
            px="10px"
            borderRadius="999px"
            borderWidth="1px"
            borderStyle="dashed"
            borderColor="border"
            bg="transparent"
            color="fg.muted"
            fontSize="11px"
            fontWeight={500}
            cursor="pointer"
            _hover={{ borderColor: "fg.subtle", color: "fg" }}
          >
            + novo
          </PillButton>
        )}
      </HStack>
    </Box>
  );
}
