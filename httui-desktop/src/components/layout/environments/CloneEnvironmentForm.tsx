// Canvas §6 Environments — clone-from-env inline form (Epic 44 Story 02).
//
// Name input + 4 checkboxes (`Copy variables` default ON, `Copy
// connections-used pointers`, `Mark temporary`, `Mark personal` →
// writes to `<name>.local.toml` instead of `<name>.toml`). The
// consumer owns the actual file copy + write; this component only
// dispatches the parsed payload.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Btn, Input } from "@/components/atoms";
import { Checkbox } from "@/components/ui/checkbox";

import { validateEnvName } from "./env-name";

export interface CloneEnvironmentPayload {
  /** Source env filename — e.g. `staging.toml`. */
  sourceFilename: string;
  /** New env name (no `.toml` suffix; the form rejects suffixes). */
  name: string;
  copyVariables: boolean;
  copyConnectionsUsed: boolean;
  markTemporary: boolean;
  /** When true, target file is `<name>.local.toml` (gitignored). */
  markPersonal: boolean;
}

export interface CloneEnvironmentFormProps {
  /** The env we're cloning from. Identity = filename. */
  sourceFilename: string;
  /** Display name shown in the heading line. */
  sourceName: string;
  /** Existing filenames for the duplicate check. */
  existingFilenames?: ReadonlyArray<string>;
  onSubmit?: (payload: CloneEnvironmentPayload) => void;
  onCancel?: () => void;
}

export function CloneEnvironmentForm({
  sourceFilename,
  sourceName,
  existingFilenames = [],
  onSubmit,
  onCancel,
}: CloneEnvironmentFormProps) {
  const [name, setName] = useState("");
  const [copyVariables, setCopyVariables] = useState(true);
  const [copyConnectionsUsed, setCopyConnectionsUsed] = useState(false);
  const [markTemporary, setMarkTemporary] = useState(false);
  const [markPersonal, setMarkPersonal] = useState(false);
  const [touched, setTouched] = useState(false);

  const validation = validateEnvName(name, existingFilenames);
  const showError = touched && !validation.ok;
  const targetFilename = `${name.trim() || "<nome>"}${
    markPersonal ? ".local.toml" : ".toml"
  }`;

  function handleSubmit() {
    setTouched(true);
    if (!validation.ok) return;
    onSubmit?.({
      sourceFilename,
      name: name.trim(),
      copyVariables,
      copyConnectionsUsed,
      markTemporary,
      markPersonal,
    });
  }

  return (
    <Box
      data-testid="clone-environment-form"
      data-source={sourceFilename}
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
        data-testid="clone-environment-heading"
      >
        Clone from{" "}
        <Text as="span" fontFamily="mono" color="fg">
          {sourceName}
        </Text>
      </Text>

      <Flex direction="column" gap={2}>
        <Box>
          <Input
            data-testid="clone-environment-name"
            placeholder={`${sourceName}-copy`}
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
              data-testid="clone-environment-name-error"
            >
              {validation.reason}
            </Text>
          )}
        </Box>

        <Flex direction="column" gap={1}>
          <CheckRow
            testId="clone-copy-variables"
            label="Copy variables"
            checked={copyVariables}
            onChange={setCopyVariables}
          />
          <CheckRow
            testId="clone-copy-connections-used"
            label="Copy connections-used pointers"
            checked={copyConnectionsUsed}
            onChange={setCopyConnectionsUsed}
          />
          <CheckRow
            testId="clone-mark-temporary"
            label="Mark temporary"
            checked={markTemporary}
            onChange={setMarkTemporary}
          />
          <CheckRow
            testId="clone-mark-personal"
            label="Mark personal (.local.toml)"
            checked={markPersonal}
            onChange={setMarkPersonal}
          />
        </Flex>

        <Flex justify="space-between" align="center">
          <Text
            fontSize="11px"
            color="fg.subtle"
            data-testid="clone-target-hint"
          >
            creates{" "}
            <Text as="span" fontFamily="mono">
              envs/{targetFilename}
            </Text>
          </Text>
          <Flex gap={2}>
            <Btn
              variant="ghost"
              data-testid="clone-environment-cancel"
              onClick={onCancel}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="clone-environment-save"
              onClick={handleSubmit}
              disabled={touched && !validation.ok}
            >
              Clone
            </Btn>
          </Flex>
        </Flex>
      </Flex>
    </Box>
  );
}

function CheckRow({
  testId,
  label,
  checked,
  onChange,
}: {
  testId: string;
  label: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <Checkbox
      checked={checked}
      onCheckedChange={(e) => onChange(!!e.checked)}
      data-testid={testId}
    >
      <Text as="span" fontSize="11px" color="fg.muted">
        {label}
      </Text>
    </Checkbox>
  );
}
