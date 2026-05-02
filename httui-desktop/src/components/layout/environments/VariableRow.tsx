import { useState } from "react";
import { Box, Flex, IconButton, Input, Text } from "@chakra-ui/react";
import { LuEye, LuEyeOff, LuLock, LuLockOpen, LuX } from "react-icons/lu";
import type { EnvVariable } from "@/lib/tauri/commands";

interface VariableRowProps {
  variable: EnvVariable;
  revealed: boolean;
  /** Real value resolved from the OS keychain. Used to surface a
   *  secret when the eye toggle is on; the masked `variable.value`
   *  comes back as empty string from `list_env_variables` so it
   *  can't fill the reveal on its own. */
  resolvedValue?: string;
  isLast: boolean;
  onSave: (value: string, isSecret?: boolean) => Promise<void>;
  onDelete: () => Promise<void>;
  onToggleReveal: () => void;
}

/**
 * One row in the env-vars editor: fixed key column + editable value, with
 * an encrypted/keychain toggle and a reveal toggle for secret values.
 *
 * Specific to env vars (not a generic key/value pair) because of the
 * `is_secret` semantics that route the value through the OS keychain.
 */
export function VariableRow({
  variable,
  revealed,
  resolvedValue,
  isLast,
  onSave,
  onDelete,
  onToggleReveal,
}: VariableRowProps) {
  const isSecret = variable.is_secret;
  const shouldMask = isSecret && !revealed;
  // Secrets come back masked from `list_env_variables`; for editing
  // we need the real value, which `resolvedValue` carries.
  const effectiveValue =
    isSecret && resolvedValue !== undefined ? resolvedValue : variable.value;

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(effectiveValue);

  const handleSave = async () => {
    if (editValue !== effectiveValue) {
      await onSave(editValue, isSecret);
    }
    setEditing(false);
  };

  const handleToggleSecret = async () => {
    await onSave(effectiveValue, !isSecret);
  };

  return (
    <Flex
      align="center"
      borderBottom={isLast ? undefined : "1px solid"}
      borderColor="border"
    >
      <Box
        px={2}
        py={1.5}
        fontFamily="mono"
        fontSize="xs"
        fontWeight="bold"
        color="fg.muted"
        minW="120px"
        bg="bg.subtle"
      >
        {variable.key}
      </Box>
      <Box borderLeft="1px solid" borderColor="border" alignSelf="stretch" />
      <Box flex={1} px={2} py={1.5}>
        {editing ? (
          <Input
            size="xs"
            variant="flushed"
            fontFamily="mono"
            fontSize="xs"
            value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            onBlur={handleSave}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSave();
              if (e.key === "Escape") {
                setEditValue(effectiveValue);
                setEditing(false);
              }
            }}
            autoFocus
          />
        ) : (
          <Text
            fontFamily="mono"
            fontSize="xs"
            cursor="pointer"
            onClick={() => {
              setEditValue(effectiveValue);
              setEditing(true);
            }}
            minH="20px"
          >
            {shouldMask ? "••••••••" : effectiveValue}
          </Text>
        )}
      </Box>
      <IconButton
        aria-label={isSecret ? "Mark as plain" : "Mark as secret"}
        size="2xs"
        variant="ghost"
        colorPalette={isSecret ? "purple" : "gray"}
        onClick={handleToggleSecret}
        title={isSecret ? "Encrypted in keychain" : "Click to encrypt"}
      >
        {isSecret ? <LuLock /> : <LuLockOpen />}
      </IconButton>
      {isSecret && (
        <IconButton
          aria-label={revealed ? "Hide value" : "Show value"}
          size="2xs"
          variant="ghost"
          onClick={onToggleReveal}
        >
          {revealed ? <LuEyeOff /> : <LuEye />}
        </IconButton>
      )}
      <IconButton
        aria-label="Delete variable"
        size="2xs"
        variant="ghost"
        colorPalette="red"
        mx={1}
        onClick={onDelete}
      >
        <LuX />
      </IconButton>
    </Flex>
  );
}
