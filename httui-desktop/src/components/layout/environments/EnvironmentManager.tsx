// Environment manager drawer (V5 cenário 10 — refactor).
//
// Quick-edit drawer kept as a UX shortcut alongside the dedicated
// Environments tab. Sidebar lists envs (click to switch focus),
// main area renders the V5 components: a `VariableValueRow` per
// variable + an inline `NewVariableForm` for adding new ones. Same
// store / IPC stack as the full Environments page — no logic
// duplication, just a different surface.

import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Box,
  Flex,
  HStack,
  IconButton,
  Portal,
  Text,
  VStack,
} from "@chakra-ui/react";
import { LuCopy, LuTrash2, LuX } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";
import { useEnvironmentStore } from "@/stores/environment";
import { resolveEnvVariables, type EnvVariable } from "@/lib/tauri/commands";

import { NewVariableForm } from "../variables/NewVariableForm";
import { VariableValueRow } from "../variables/VariableValueRow";

export function EnvironmentManager() {
  const environments = useEnvironmentStore((s) => s.environments);
  const managerOpen = useEnvironmentStore((s) => s.managerOpen);
  const closeManager = useEnvironmentStore((s) => s.closeManager);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);
  const createEnvironment = useEnvironmentStore((s) => s.createEnvironment);
  const deleteEnvironment = useEnvironmentStore((s) => s.deleteEnvironment);
  const duplicateEnvironment = useEnvironmentStore(
    (s) => s.duplicateEnvironment,
  );
  const loadVariables = useEnvironmentStore((s) => s.loadVariables);
  const setVariable = useEnvironmentStore((s) => s.setVariable);
  const deleteVariable = useEnvironmentStore((s) => s.deleteVariable);

  const [selectedEnvId, setSelectedEnvId] = useState<string | null>(null);
  const [variables, setVariables] = useState<EnvVariable[]>([]);
  const [creating, setCreating] = useState(false);
  const [newEnvName, setNewEnvName] = useState("");
  const [newEnvCreating, setNewEnvCreating] = useState(false);

  useEffect(() => {
    if (!managerOpen) return;
    if (selectedEnvId && environments.some((e) => e.id === selectedEnvId))
      return;
    setSelectedEnvId(environments[0]?.id ?? null);
  }, [managerOpen, environments, selectedEnvId]);

  const refreshVars = useCallback(async () => {
    if (!selectedEnvId) {
      setVariables([]);
      return;
    }
    const vars = await loadVariables(selectedEnvId);
    setVariables(vars);
  }, [selectedEnvId, loadVariables]);

  useEffect(() => {
    let cancelled = false;
    void refreshVars().then(() => {
      if (cancelled) return;
    });
    return () => {
      cancelled = true;
    };
  }, [refreshVars]);

  const selectedEnv = environments.find((e) => e.id === selectedEnvId) ?? null;

  const handleCreateEnv = useCallback(async () => {
    const name = newEnvName.trim();
    if (!name) return;
    await createEnvironment(name);
    setNewEnvName("");
    setNewEnvCreating(false);
  }, [newEnvName, createEnvironment]);

  const handleDeleteEnv = useCallback(async () => {
    if (!selectedEnv) return;
    await deleteEnvironment(selectedEnv.id);
    setSelectedEnvId(null);
  }, [selectedEnv, deleteEnvironment]);

  const handleDuplicateEnv = useCallback(async () => {
    if (!selectedEnv) return;
    await duplicateEnvironment(selectedEnv.id, `${selectedEnv.name}-copy`);
  }, [selectedEnv, duplicateEnvironment]);

  const handleCommitValue = useCallback(
    async (env: string, key: string, next: string, isSecret: boolean) => {
      const target = environments.find((e) => e.name === env);
      if (!target) return;
      await setVariable(target.id, key, next, isSecret);
      await refreshVars();
    },
    [environments, setVariable, refreshVars],
  );

  const handleNewVariable = useCallback(
    async (payload: {
      name: string;
      value: string;
      isSecret: boolean;
      env: string;
    }) => {
      const target = environments.find((e) => e.name === payload.env);
      if (!target) return;
      await setVariable(
        target.id,
        payload.name,
        payload.value,
        payload.isSecret,
      );
      setCreating(false);
      await refreshVars();
    },
    [environments, setVariable, refreshVars],
  );

  if (!managerOpen) return null;

  return (
    <Portal>
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1400}
        onClick={closeManager}
      />
      <Box
        position="fixed"
        top={0}
        right={0}
        h="100vh"
        w="640px"
        maxW="90vw"
        bg="bg"
        borderLeftWidth="1px"
        borderColor="border"
        zIndex={1401}
        display="flex"
        flexDirection="column"
      >
        <Flex
          align="center"
          justify="space-between"
          px={4}
          py={3}
          borderBottomWidth="1px"
          borderColor="border"
        >
          <Text fontWeight="semibold" fontSize="sm">
            Environments
          </Text>
          <IconButton
            aria-label="Close"
            variant="ghost"
            size="sm"
            onClick={closeManager}
          >
            <LuX />
          </IconButton>
        </Flex>

        <Flex flex={1} overflow="hidden">
          <VStack
            w="180px"
            flexShrink={0}
            borderRightWidth="1px"
            borderColor="border"
            p={2}
            gap={1}
            align="stretch"
            overflow="auto"
          >
            {environments.map((env) => (
              <Flex
                key={env.id}
                align="center"
                gap={1}
                px={2}
                py={1.5}
                rounded="md"
                cursor="pointer"
                bg={selectedEnvId === env.id ? "bg.subtle" : undefined}
                _hover={{ bg: "bg.subtle" }}
                onClick={() => setSelectedEnvId(env.id)}
              >
                <Text fontSize="xs" flex={1} truncate>
                  {env.name}
                </Text>
                {env.is_active && (
                  <Badge size="xs" colorPalette="green" variant="subtle">
                    active
                  </Badge>
                )}
              </Flex>
            ))}

            {newEnvCreating ? (
              <Flex gap={1} align="center">
                <Input
                  data-testid="env-mgr-new-env-name"
                  placeholder="Name…"
                  value={newEnvName}
                  onChange={(e) => setNewEnvName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") void handleCreateEnv();
                    if (e.key === "Escape") setNewEnvCreating(false);
                  }}
                  autoFocus
                />
              </Flex>
            ) : (
              <Btn variant="ghost" onClick={() => setNewEnvCreating(true)}>
                + New env
              </Btn>
            )}
          </VStack>

          <Box flex={1} overflow="auto">
            {selectedEnv ? (
              <Box>
                <Flex
                  align="center"
                  gap={2}
                  px={4}
                  py={3}
                  borderBottomWidth="1px"
                  borderColor="border"
                >
                  <Text fontWeight="semibold" fontSize="sm">
                    {selectedEnv.name}
                  </Text>
                  {selectedEnv.is_active ? (
                    <Badge colorPalette="green" variant="subtle" size="sm">
                      active
                    </Badge>
                  ) : (
                    <Btn
                      variant="ghost"
                      onClick={() => switchEnvironment(selectedEnv.id)}
                    >
                      Set active
                    </Btn>
                  )}
                  <HStack gap={0} ml="auto">
                    <IconButton
                      aria-label="Duplicate"
                      size="xs"
                      variant="ghost"
                      onClick={handleDuplicateEnv}
                    >
                      <LuCopy />
                    </IconButton>
                    <IconButton
                      aria-label="Delete"
                      size="xs"
                      variant="ghost"
                      colorPalette="red"
                      onClick={handleDeleteEnv}
                    >
                      <LuTrash2 />
                    </IconButton>
                  </HStack>
                </Flex>

                <Box>
                  {variables.map((v) => (
                    <VariableValueRow
                      key={v.id}
                      env={selectedEnv.name}
                      keyLabel={v.key}
                      value={v.is_secret ? undefined : v.value}
                      isSecret={v.is_secret}
                      fetchSecret={async () => {
                        const map = await resolveEnvVariables(selectedEnv.id);
                        return map[v.key];
                      }}
                      onCommit={(env, next) =>
                        handleCommitValue(env, v.key, next, v.is_secret)
                      }
                      onDelete={async () => {
                        await deleteVariable(v.id);
                        await refreshVars();
                      }}
                    />
                  ))}
                </Box>

                <Box mt={3}>
                  {creating ? (
                    <NewVariableForm
                      activeEnv={selectedEnv.name}
                      existingNames={variables.map((v) => v.key)}
                      onSubmit={handleNewVariable}
                      onCancel={() => setCreating(false)}
                    />
                  ) : (
                    <Box px={4} py={2}>
                      <Btn variant="ghost" onClick={() => setCreating(true)}>
                        + New variable
                      </Btn>
                    </Box>
                  )}
                </Box>

                <Text fontSize="xs" color="fg.muted" px={4} py={3}>
                  Use{" "}
                  <Text as="span" fontFamily="mono">
                    {"{{KEY}}"}
                  </Text>{" "}
                  in HTTP / DB blocks to reference variables from the active
                  environment.
                </Text>
              </Box>
            ) : (
              <Flex
                align="center"
                justify="center"
                h="100%"
                color="fg.muted"
                fontSize="sm"
              >
                {environments.length === 0
                  ? "Create an environment to get started"
                  : "Select an environment"}
              </Flex>
            )}
          </Box>
        </Flex>
      </Box>
    </Portal>
  );
}
