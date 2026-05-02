import {
  Badge,
  Box,
  Flex,
  HStack,
  IconButton,
  Text,
  VStack,
} from "@chakra-ui/react";
import { LuCopy, LuTrash2 } from "react-icons/lu";
import type { EnvVariable } from "@/lib/tauri/commands";
import { KeyValueAddRow } from "@/components/ui/KeyValueAddRow";
import { VariableRow } from "./VariableRow";

interface VariablesEditorProps {
  envName: string;
  isActive: boolean;
  variables: EnvVariable[];
  revealedKeys: Set<string>;
  /** Keychain-resolved values keyed by `EnvVariable.key`. Optional —
   *  when missing, secret rows render `••••••••` even if revealed. */
  resolvedValues?: Record<string, string>;
  onSetActive: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onSetVariable: (
    key: string,
    value: string,
    isSecret?: boolean,
  ) => Promise<void>;
  onDeleteVariable: (id: string) => Promise<void>;
  onToggleReveal: (varId: string) => void;
}

export function VariablesEditor({
  envName,
  isActive,
  variables,
  revealedKeys,
  resolvedValues,
  onSetActive,
  onDuplicate,
  onDelete,
  onSetVariable,
  onDeleteVariable,
  onToggleReveal,
}: VariablesEditorProps) {
  return (
    <VStack align="stretch" gap={3}>
      {/* Env header with actions */}
      <Flex align="center" gap={2}>
        <Text fontWeight="semibold" fontSize="sm">
          {envName}
        </Text>
        {isActive ? (
          <Badge colorPalette="green" variant="subtle" size="sm">
            active
          </Badge>
        ) : (
          <Badge
            as="button"
            colorPalette="gray"
            variant="outline"
            size="sm"
            cursor="pointer"
            onClick={onSetActive}
          >
            Set active
          </Badge>
        )}
        <HStack gap={0} ml="auto">
          <IconButton
            aria-label="Duplicate"
            size="xs"
            variant="ghost"
            onClick={onDuplicate}
          >
            <LuCopy />
          </IconButton>
          <IconButton
            aria-label="Delete"
            size="xs"
            variant="ghost"
            colorPalette="red"
            onClick={onDelete}
          >
            <LuTrash2 />
          </IconButton>
        </HStack>
      </Flex>

      {/* Variable list */}
      <Box
        border="1px solid"
        borderColor="border"
        rounded="md"
        overflow="hidden"
      >
        {variables.map((v, i) => (
          <VariableRow
            key={v.id}
            variable={v}
            revealed={revealedKeys.has(v.id)}
            resolvedValue={resolvedValues?.[v.key]}
            isLast={i === variables.length - 1}
            onSave={(value, isSecret) => onSetVariable(v.key, value, isSecret)}
            onDelete={() => onDeleteVariable(v.id)}
            onToggleReveal={() => onToggleReveal(v.id)}
          />
        ))}

        {/* Add new variable */}
        <KeyValueAddRow
          onAdd={(key, value) => onSetVariable(key, value)}
          withTopBorder={variables.length > 0}
        />
      </Box>

      <Text fontSize="xs" color="fg.muted">
        Use{" "}
        <Text as="span" fontFamily="mono">
          {"{{KEY}}"}
        </Text>{" "}
        in HTTP blocks to reference variables from the active environment.
      </Text>
    </VStack>
  );
}
