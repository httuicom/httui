// Sidebar variables panel.
//
// Lists variables of the active environment. Secret-flagged values
// surface a key icon and render `••••` instead of the raw value;
// non-secret values render truncated with a tooltip.
//
// Reads from `useEnvironmentStore.loadVariables(activeEnv.id)` and
// re-fetches whenever the active env changes (or `variablesVersion`
// bumps after the manager edits something).

import { Box, Flex, HStack, IconButton, Text } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { LuKey, LuPencil } from "react-icons/lu";

import { useEnvironmentStore } from "@/stores/environment";
import type { EnvVariable } from "@/lib/tauri/commands";

const SECRET_MASK = "••••••••";

export function VariablesPanel() {
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const loadVariables = useEnvironmentStore((s) => s.loadVariables);
  const variablesVersion = useEnvironmentStore((s) => s.variablesVersion);
  const openManager = useEnvironmentStore((s) => s.openManager);

  const [variables, setVariables] = useState<EnvVariable[]>([]);

  useEffect(() => {
    let cancelled = false;
    if (!activeEnvironment) {
      setVariables([]);
      return;
    }
    loadVariables(activeEnvironment.id)
      .then((vars) => {
        if (!cancelled) setVariables(vars);
      })
      .catch(() => {
        if (!cancelled) setVariables([]);
      });
    return () => {
      cancelled = true;
    };
  }, [activeEnvironment, loadVariables, variablesVersion]);

  return (
    <Box data-testid="variables-panel">
      <HStack px={3} py={2} justify="space-between">
        <Text
          fontSize="xs"
          fontWeight="semibold"
          color="fg.subtle"
          textTransform="uppercase"
          letterSpacing="wider"
        >
          Variables
        </Text>
        <IconButton
          aria-label="Edit variables"
          variant="ghost"
          size="xs"
          onClick={openManager}
        >
          <LuPencil />
        </IconButton>
      </HStack>

      {!activeEnvironment ? (
        <Box px={3} py={4} textAlign="center">
          <Text fontSize="sm" color="fg.subtle">
            No active environment
          </Text>
        </Box>
      ) : variables.length === 0 ? (
        <Box px={3} py={4} textAlign="center">
          <Text fontSize="sm" color="fg.subtle">
            No variables
          </Text>
        </Box>
      ) : (
        <Box px={1} pb={2}>
          {variables.map((v) => (
            <Flex
              key={v.id}
              data-var-key={v.key}
              data-secret={v.is_secret ? "true" : "false"}
              align="center"
              gap={2}
              px={2}
              py={1}
              mx={1}
              rounded="md"
              fontSize="xs"
              fontFamily="mono"
              title={v.is_secret ? `${v.key} (secret)` : `${v.key}=${v.value}`}
            >
              <Box w="14px" display="inline-flex" justifyContent="center">
                {v.is_secret && (
                  <LuKey
                    size={11}
                    aria-label="secret"
                    data-testid={`var-key-icon-${v.key}`}
                  />
                )}
              </Box>
              <Text flex={1} truncate color="fg.1">
                {v.key}
              </Text>
              <Text color="fg.subtle" maxW="80px" truncate>
                {v.is_secret ? SECRET_MASK : v.value}
              </Text>
            </Flex>
          ))}
        </Box>
      )}
    </Box>
  );
}
