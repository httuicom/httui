// Canvas §6 Environments — rename inline form (Epic 44 Story 04).
//
// Pre-filled with the source name. Reuses `validateEnvName` so the
// rejection set is identical to create/clone. Important: the source
// env's own filename is filtered out of the duplicate check so the
// user can land on the same name (no-op rename) or change case
// without colliding with itself.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Btn, Input } from "@/components/atoms";

import { validateEnvName } from "./env-name";
import type { EnvironmentSummary } from "./envs-meta";

export interface RenameEnvironmentPayload {
  /** Source env filename (identity). */
  sourceFilename: string;
  /** Target name (no `.toml` suffix; the form rejects suffixes). */
  newName: string;
}

export interface RenameEnvironmentFormProps {
  env: EnvironmentSummary;
  /** Existing filenames in the vault for the duplicate check. */
  existingFilenames?: ReadonlyArray<string>;
  onSubmit?: (payload: RenameEnvironmentPayload) => void;
  onCancel?: () => void;
}

export function RenameEnvironmentForm({
  env,
  existingFilenames = [],
  onSubmit,
  onCancel,
}: RenameEnvironmentFormProps) {
  const [name, setName] = useState(env.name);
  const [touched, setTouched] = useState(false);

  // Filter out the source filename so renaming to the same name (or
  // changing case) doesn't trip the duplicate check.
  const others = existingFilenames.filter((f) => f !== env.filename);
  const validation = validateEnvName(name, others);
  const showError = touched && !validation.ok;
  const noChange = name.trim() === env.name;

  function handleSubmit() {
    setTouched(true);
    if (!validation.ok) return;
    if (noChange) {
      onCancel?.();
      return;
    }
    onSubmit?.({ sourceFilename: env.filename, newName: name.trim() });
  }

  return (
    <Box
      data-testid="rename-environment-form"
      data-source={env.filename}
      px={5}
      py={3}
      borderTopWidth="1px"
      borderTopColor="border"
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.muted"
    >
      <Text
        fontSize="11px"
        color="fg.muted"
        mb={2}
        data-testid="rename-environment-heading"
      >
        Rename{" "}
        <Text as="span" fontFamily="mono" color="fg">
          {env.name}
        </Text>
      </Text>

      <Flex direction="column" gap={2}>
        <Box>
          <Input
            data-testid="rename-environment-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                handleSubmit();
              } else if (e.key === "Escape") {
                e.preventDefault();
                onCancel?.();
              }
            }}
            autoFocus
            aria-invalid={showError}
          />
          {showError && !validation.ok && (
            <Text
              fontSize="11px"
              color="error"
              mt={1}
              data-testid="rename-environment-name-error"
            >
              {validation.reason}
            </Text>
          )}
        </Box>

        <Flex justify="space-between" align="center">
          <Text
            fontSize="11px"
            color="fg.subtle"
            data-testid="rename-environment-target-hint"
          >
            renames to{" "}
            <Text as="span" fontFamily="mono">
              envs/{name.trim() || "<name>"}
              {env.isPersonal ? ".local.toml" : ".toml"}
            </Text>
          </Text>
          <Flex gap={2}>
            <Btn
              variant="ghost"
              data-testid="rename-environment-cancel"
              onClick={onCancel}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="rename-environment-save"
              onClick={handleSubmit}
              disabled={touched && !validation.ok}
            >
              Rename
            </Btn>
          </Flex>
        </Flex>
      </Flex>
    </Box>
  );
}
