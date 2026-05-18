// Canvas §6 Variables — new-variable inline form.
//
// Single table-style row inserted above the variable list, inspired
// by the legacy environments drawer footer: [KEY input] [VALUE input]
// [lock toggle] [+ save] [× cancel]. Error message + visibility hint
// surface below the row. Save dispatches the parsed payload; the
// consumer wires the actual store write.

import { Box, Flex, IconButton, Text } from "@chakra-ui/react";
import { useState } from "react";
import { LuLock, LuLockOpen, LuPlus, LuX } from "react-icons/lu";

import { Input } from "@/components/atoms";

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
    <Box data-testid="new-variable-form" mx={5} my={2}>
      <Flex
        align="stretch"
        borderWidth="1px"
        borderColor="brand.fg"
        borderRadius="6px"
        bg="bg"
        overflow="hidden"
      >
        <Box flex="0 0 38%" minW={0}>
          <Input
            data-testid="new-variable-name"
            placeholder="KEY"
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
            css={{
              border: "none",
              borderRadius: 0,
              fontFamily: "var(--chakra-fonts-mono)",
              fontSize: "12px",
            }}
          />
        </Box>
        <Box w="1px" bg="border" flexShrink={0} />
        <Box flex={1} minW={0}>
          <Input
            data-testid="new-variable-value"
            placeholder={isSecret ? "secret value" : "value"}
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
            css={{
              border: "none",
              borderRadius: 0,
              fontFamily: "var(--chakra-fonts-mono)",
              fontSize: "12px",
            }}
          />
        </Box>
        <Flex
          align="center"
          gap={1}
          px={2}
          borderLeftWidth="1px"
          borderLeftColor="border"
          bg="bg.subtle"
          flexShrink={0}
        >
          <IconButton
            aria-label={isSecret ? "Mark as public" : "Mark as secret"}
            title={
              isSecret
                ? "Secret — value lives in keychain"
                : "Public — value lives in envs/*.toml"
            }
            data-testid="new-variable-is-secret"
            data-is-secret={isSecret || undefined}
            size="2xs"
            variant="ghost"
            color={isSecret ? "brand.fg" : "fg.subtle"}
            onClick={() => setIsSecret((v) => !v)}
          >
            {isSecret ? <LuLock /> : <LuLockOpen />}
          </IconButton>
          <IconButton
            aria-label="Save variable"
            title="Save"
            data-testid="new-variable-save"
            size="2xs"
            variant="ghost"
            colorPalette="green"
            onClick={handleSubmit}
            disabled={touched && !validation.ok}
          >
            <LuPlus />
          </IconButton>
          <IconButton
            aria-label="Cancel"
            title="Cancel"
            data-testid="new-variable-cancel"
            size="2xs"
            variant="ghost"
            color="fg.subtle"
            onClick={onCancel}
          >
            <LuX />
          </IconButton>
        </Flex>
      </Flex>

      <Flex justify="space-between" mt={1.5} px={1} gap={3}>
        <Box minW={0}>
          {showError && !validation.ok ? (
            <Text
              fontSize="11px"
              color="error"
              data-testid="new-variable-name-error"
              truncate
            >
              {validation.reason}
            </Text>
          ) : (
            <Text
              fontSize="11px"
              color="fg.subtle"
              data-testid="new-variable-hint"
            >
              landing in env{" "}
              <Text as="span" fontFamily="mono" color="fg.muted">
                {activeEnv}
              </Text>{" "}
              ·{" "}
              <Text
                as="span"
                fontFamily="mono"
                color={isSecret ? "brand.fg" : "fg.muted"}
                data-testid="new-variable-is-secret-label"
              >
                {isSecret ? "secret (keychain)" : "public (envs/*.toml)"}
              </Text>
            </Text>
          )}
        </Box>
        <Text
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
        >
          ↵ save · esc cancel
        </Text>
      </Flex>
    </Box>
  );
}
