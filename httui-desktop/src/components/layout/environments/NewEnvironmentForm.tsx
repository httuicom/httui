
import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import { useInlineForm } from "@/hooks/useInlineForm";

import { validateEnvName } from "./env-name";

export interface NewEnvironmentPayload {
  name: string;
}

export interface NewEnvironmentFormProps {
  /** Existing env filenames for the duplicate check. */
  existingFilenames?: ReadonlyArray<string>;
  onSubmit?: (payload: NewEnvironmentPayload) => void;
  onCancel?: () => void;
}

export function NewEnvironmentForm({
  existingFilenames = [],
  onSubmit,
  onCancel,
}: NewEnvironmentFormProps) {
  const nameField = useInlineForm("", (n) =>
    validateEnvName(n, existingFilenames),
  );

  function handleSubmit() {
    if (!nameField.attemptSubmit()) return;
    onSubmit?.({ name: nameField.value.trim() });
  }

  return (
    <Box
      data-testid="new-environment-form"
      px={4}
      py={3}
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      bg="bg"
    >
      <Flex direction="column" gap={2}>
        <Box>
          <Input
            data-testid="new-environment-name"
            placeholder="staging"
            value={nameField.value}
            onChange={(e) => nameField.setValue(e.target.value)}
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
            aria-invalid={nameField.showError}
          />
          {nameField.showError && (
            <Text
              fontSize="11px"
              color="error"
              mt={1}
              data-testid="new-environment-name-error"
            >
              {nameField.error}
            </Text>
          )}
        </Box>

        <Flex justify="space-between" align="center">
          <Text
            fontSize="11px"
            color="fg.subtle"
            data-testid="new-environment-target-hint"
          >
            creates{" "}
            <Text as="span" fontFamily="mono">
              envs/{nameField.value.trim() || "<name>"}.toml
            </Text>
          </Text>
          <Flex gap={2}>
            <Btn
              variant="ghost"
              data-testid="new-environment-cancel"
              onClick={onCancel}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="new-environment-save"
              onClick={handleSubmit}
              disabled={nameField.showError}
            >
              Save
            </Btn>
          </Flex>
        </Flex>
      </Flex>
    </Box>
  );
}
