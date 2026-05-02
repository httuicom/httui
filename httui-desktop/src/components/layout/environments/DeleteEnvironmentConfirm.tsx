// Canvas §6 Environments — delete confirmation (Epic 44 Story 04).
//
// Inline confirmation banner. Story 04 spec: "this will delete
// envs/X.toml and any .local.toml siblings". The component renders
// the warning text + Confirm/Cancel; the consumer wires the actual
// Tauri delete call (EnvironmentsStore::delete_env). Promotes the
// .local.toml note into the body when isPersonal=false (i.e. there
// MAY be a sibling file the user has on disk).

import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

import type { EnvironmentSummary } from "./envs-meta";

export interface DeleteEnvironmentConfirmProps {
  env: EnvironmentSummary;
  onConfirm?: (filename: string) => void;
  onCancel?: () => void;
}

export function DeleteEnvironmentConfirm({
  env,
  onConfirm,
  onCancel,
}: DeleteEnvironmentConfirmProps) {
  return (
    <Box
      data-testid="delete-environment-confirm"
      data-target={env.filename}
      px={5}
      py={3}
      borderTopWidth="1px"
      borderTopColor="border"
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.muted"
    >
      <Flex direction="column" gap={2}>
        <Text
          fontSize="11px"
          color="fg"
          fontWeight="bold"
          data-testid="delete-environment-confirm-heading"
        >
          Delete{" "}
          <Text as="span" fontFamily="mono">
            {env.name}
          </Text>
          ?
        </Text>
        <Text
          fontSize="11px"
          color="fg.muted"
          data-testid="delete-environment-confirm-body"
        >
          {env.isPersonal ? (
            <>
              This deletes{" "}
              <Text as="span" fontFamily="mono">
                envs/{env.filename}
              </Text>{" "}
              from your machine (it's gitignored — won't affect the team).
            </>
          ) : (
            <>
              This deletes{" "}
              <Text as="span" fontFamily="mono">
                envs/{env.filename}
              </Text>{" "}
              and any{" "}
              <Text as="span" fontFamily="mono">
                .local.toml
              </Text>{" "}
              siblings on this machine.
            </>
          )}
        </Text>

        <Flex justify="flex-end" gap={2}>
          <Btn
            variant="ghost"
            data-testid="delete-environment-cancel"
            onClick={onCancel}
          >
            Cancel
          </Btn>
          <Btn
            variant="primary"
            data-testid="delete-environment-confirm-submit"
            onClick={() => onConfirm?.(env.filename)}
          >
            Delete
          </Btn>
        </Flex>
      </Flex>
    </Box>
  );
}
