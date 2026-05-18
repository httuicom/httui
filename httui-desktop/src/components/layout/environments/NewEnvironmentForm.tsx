// Canvas §6 Environments — "+ New environment" inline form.
//
// Name only — creates an empty `envs/<name>.toml`. The consumer
// wires the actual file write. `Mark personal` belongs to the clone
// form per the spec; new envs are public by default.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Btn, Input } from "@/components/atoms";

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
  const [name, setName] = useState("");
  const [touched, setTouched] = useState(false);

  const validation = validateEnvName(name, existingFilenames);
  const showError = touched && !validation.ok;

  function handleSubmit() {
    setTouched(true);
    if (!validation.ok) return;
    onSubmit?.({ name: name.trim() });
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
              data-testid="new-environment-name-error"
            >
              {validation.reason}
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
              envs/{name.trim() || "<name>"}.toml
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
              disabled={touched && !validation.ok}
            >
              Save
            </Btn>
          </Flex>
        </Flex>
      </Flex>
    </Box>
  );
}
