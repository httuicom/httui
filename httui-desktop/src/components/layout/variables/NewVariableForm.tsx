// Canvas §6 Variables — new-variable inline form (Epic 43 Story 05).
//
// Compact form rendered above the variable list. Three fields: name,
// value (for the active env only — empty cells in other envs stay
// undefined per the story spec, so the var doesn't shadow OS env in
// other envs), and is_secret toggle. Save dispatches the parsed
// payload; the consumer wires the actual store write. Cancel discards.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Btn, Input } from "@/components/atoms";
import { Switch } from "@/components/ui/switch";

import { validateVariableName } from "./variable-name";

export interface NewVariablePayload {
  name: string;
  value: string;
  isSecret: boolean;
  /** Active env where the value should land. The other envs stay undefined. */
  env: string;
}

export interface NewVariableFormProps {
  /** Active env name shown in the value-row label. */
  activeEnv: string;
  /** Existing variable names (for the duplicate check). */
  existingNames?: ReadonlyArray<string>;
  onSubmit?: (payload: NewVariablePayload) => void;
  onCancel?: () => void;
}

export function NewVariableForm({
  activeEnv,
  existingNames = [],
  onSubmit,
  onCancel,
}: NewVariableFormProps) {
  const [name, setName] = useState("");
  const [value, setValue] = useState("");
  const [isSecret, setIsSecret] = useState(false);
  const [touched, setTouched] = useState(false);

  const validation = validateVariableName(name, existingNames);
  const showError = touched && !validation.ok;

  function handleSubmit() {
    setTouched(true);
    if (!validation.ok) return;
    onSubmit?.({
      name: name.trim(),
      value,
      isSecret,
      env: activeEnv,
    });
  }

  return (
    <Box
      data-testid="new-variable-form"
      px={5}
      py={3}
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.muted"
    >
      <Flex direction="column" gap={2}>
        <Box>
          <Input
            data-testid="new-variable-name"
            placeholder="API_BASE_URL"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
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
              data-testid="new-variable-name-error"
            >
              {validation.reason}
            </Text>
          )}
        </Box>

        <Flex align="center" gap={2}>
          <Text
            as="span"
            fontFamily="mono"
            fontSize="11px"
            color="fg.muted"
            w="68px"
            flexShrink={0}
            truncate
            data-testid="new-variable-active-env"
          >
            {activeEnv}
          </Text>
          <Box flex={1}>
            <Input
              data-testid="new-variable-value"
              placeholder="value"
              value={value}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleSubmit();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  onCancel?.();
                }
              }}
            />
          </Box>
        </Flex>

        <Flex align="center" justify="space-between">
          <Flex align="center" gap={2}>
            <Switch
              size="sm"
              checked={isSecret}
              data-testid="new-variable-is-secret"
              onCheckedChange={(e: { checked: boolean }) =>
                setIsSecret(e.checked)
              }
              aria-label="is_secret"
            />
            <Text
              fontSize="11px"
              color="fg.muted"
              data-testid="new-variable-is-secret-label"
            >
              {isSecret ? "Secret (keychain)" : "Public (envs/*.toml)"}
            </Text>
          </Flex>
          <Flex gap={2}>
            <Btn
              variant="ghost"
              data-testid="new-variable-cancel"
              onClick={onCancel}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="new-variable-save"
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
