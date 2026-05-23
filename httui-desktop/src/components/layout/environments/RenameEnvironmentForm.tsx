import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import { useInlineForm } from "@/hooks/useInlineForm";

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

  // Exclude source so same-name or case-only renames pass the duplicate check.
  const others = existingFilenames.filter((f) => f !== env.filename);
  const nameField = useInlineForm(env.name, (n) => validateEnvName(n, others));
  const noChange = nameField.value.trim() === env.name;

  function handleSubmit() {
    if (!nameField.attemptSubmit()) return;
    if (noChange) {
      onCancel?.();
      return;
    }
    onSubmit?.({
      sourceFilename: env.filename,
      newName: nameField.value.trim(),
    });
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
              data-testid="rename-environment-name-error"
            >
              {nameField.error}
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
              envs/{nameField.value.trim() || "<name>"}
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
              disabled={nameField.showError}
            >
              Rename
            </Btn>
          </Flex>
        </Flex>
      </Flex>
    </Box>
  );
}
