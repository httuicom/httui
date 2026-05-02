import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Flex,
  VStack,
  Text,
  Input,
  IconButton,
  Badge,
  Portal,
} from "@chakra-ui/react";
import { LuPlus, LuX, LuCheck } from "react-icons/lu";
import { useEnvironmentStore } from "@/stores/environment";
import {
  resolveEnvVariables,
  type EnvVariable,
} from "@/lib/tauri/commands";
import { VariablesEditor } from "./VariablesEditor";

export function EnvironmentManager() {
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
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
  const [resolvedValues, setResolvedValues] = useState<Record<string, string>>(
    {},
  );
  const [newEnvName, setNewEnvName] = useState("");
  const [creating, setCreating] = useState(false);
  const [revealedKeys, setRevealedKeys] = useState<Set<string>>(new Set());

  // Select first environment on open or when list changes
  useEffect(() => {
    if (!managerOpen) return;
    if (selectedEnvId && environments.some((e) => e.id === selectedEnvId))
      return;
    setSelectedEnvId(environments[0]?.id ?? null);
  }, [managerOpen, environments, selectedEnvId]);

  // Load variables when selected environment changes — also fetch
  // the keychain-resolved map so the reveal eye has real values to
  // surface. Reset revealedKeys per env switch so secrets don't
  // leak across envs by accident.
  useEffect(() => {
    setRevealedKeys(new Set());
    if (!selectedEnvId) {
      setVariables([]);
      setResolvedValues({});
      return;
    }
    let cancelled = false;
    Promise.all([
      loadVariables(selectedEnvId),
      resolveEnvVariables(selectedEnvId).catch(() => ({})),
    ]).then(([vars, resolved]) => {
      if (cancelled) return;
      setVariables(vars);
      setResolvedValues(resolved);
    });
    return () => {
      cancelled = true;
    };
  }, [selectedEnvId, loadVariables]);

  const refreshVariables = useCallback(async () => {
    if (!selectedEnvId) return;
    const [vars, resolved] = await Promise.all([
      loadVariables(selectedEnvId),
      resolveEnvVariables(selectedEnvId).catch(() => ({})),
    ]);
    setVariables(vars);
    setResolvedValues(resolved);
  }, [selectedEnvId, loadVariables]);

  const handleCreate = useCallback(async () => {
    if (!newEnvName.trim()) return;
    await createEnvironment(newEnvName.trim());
    setNewEnvName("");
    setCreating(false);
  }, [newEnvName, createEnvironment]);

  const handleDuplicate = useCallback(
    async (id: string, name: string) => {
      await duplicateEnvironment(id, `${name} (copy)`);
    },
    [duplicateEnvironment],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      await deleteEnvironment(id);
      if (selectedEnvId === id) setSelectedEnvId(null);
    },
    [deleteEnvironment, selectedEnvId],
  );

  const handleSetVariable = useCallback(
    async (key: string, value: string, isSecret?: boolean) => {
      if (!selectedEnvId || !key.trim()) return;
      await setVariable(selectedEnvId, key, value, isSecret);
      await refreshVariables();
    },
    [selectedEnvId, setVariable, refreshVariables],
  );

  const handleDeleteVariable = useCallback(
    async (id: string) => {
      await deleteVariable(id);
      await refreshVariables();
    },
    [deleteVariable, refreshVariables],
  );

  const toggleReveal = (varId: string) => {
    setRevealedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(varId)) {
        next.delete(varId);
      } else {
        next.add(varId);
      }
      return next;
    });
  };

  if (!managerOpen) return null;

  return (
    <Portal>
      {/* Backdrop */}
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1400}
        onClick={closeManager}
      />
      {/* Panel */}
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
        {/* Header */}
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
          {/* Sidebar: environment list */}
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

            {creating ? (
              <Flex gap={1} align="center">
                <Input
                  size="xs"
                  placeholder="Name..."
                  value={newEnvName}
                  onChange={(e) => setNewEnvName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleCreate();
                    if (e.key === "Escape") setCreating(false);
                  }}
                  autoFocus
                />
                <IconButton
                  aria-label="Confirm"
                  size="2xs"
                  variant="ghost"
                  colorPalette="green"
                  onClick={handleCreate}
                >
                  <LuCheck />
                </IconButton>
              </Flex>
            ) : (
              <Flex
                align="center"
                gap={1}
                px={2}
                py={1}
                cursor="pointer"
                color="fg.muted"
                fontSize="xs"
                _hover={{ bg: "bg.subtle" }}
                rounded="md"
                onClick={() => setCreating(true)}
              >
                <LuPlus size={12} />
                New environment
              </Flex>
            )}
          </VStack>

          {/* Main: variables for selected environment */}
          <Box flex={1} overflow="auto" p={3}>
            {selectedEnvId ? (
              <VariablesEditor
                envName={
                  environments.find((e) => e.id === selectedEnvId)?.name ?? ""
                }
                isActive={activeEnvironment?.id === selectedEnvId}
                variables={variables}
                revealedKeys={revealedKeys}
                resolvedValues={resolvedValues}
                onSetActive={() => switchEnvironment(selectedEnvId)}
                onDuplicate={() =>
                  handleDuplicate(
                    selectedEnvId,
                    environments.find((e) => e.id === selectedEnvId)?.name ??
                      "",
                  )
                }
                onDelete={() => handleDelete(selectedEnvId)}
                onSetVariable={handleSetVariable}
                onDeleteVariable={handleDeleteVariable}
                onToggleReveal={toggleReveal}
              />
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
