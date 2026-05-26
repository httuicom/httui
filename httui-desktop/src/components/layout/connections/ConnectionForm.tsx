import {
  Box,
  Flex,
  HStack,
  VStack,
  Text,
  Input,
  Badge,
  Spinner,
  IconButton,
  Portal,
} from "@chakra-ui/react";
import { LuX, LuPlugZap, LuDatabase } from "react-icons/lu";
import { useCallback, useEffect, useReducer, useRef } from "react";

import type { Connection } from "@/lib/tauri/connections";
import {
  createConnection,
  updateConnection,
  testConnection,
} from "@/lib/tauri/connections";

import { DriverSelector, DRIVER_CONFIG } from "./form/DriverSelector";
import { SqliteFields } from "./form/SqliteFields";
import { NetworkFields } from "./form/NetworkFields";
import { AdvancedFields } from "./form/AdvancedFields";
import { buildConnectionPreview } from "./form/connection-string";
import {
  buildConnectionInput,
  connectionFormReducer,
  initConnectionFormState,
  validateConnection,
} from "./connection-form-state";

interface ConnectionFormProps {
  connection: Connection | null;
  onClose: () => void;
}

/** Modal form for creating / editing a database connection. All field
 * state lives in one `useReducer` (`connection-form-state.ts`) — the
 * old 18 `useState` + the props→state driver→port mirror effect were
 * a desync hazard with zero field validation (audit 02 §4 / 05 Part
 * B). The visual sections delegate to `form/*` sub-components. */
export function ConnectionForm({ connection, onClose }: ConnectionFormProps) {
  const isEdit = connection !== null;
  const overlayRef = useRef<HTMLDivElement>(null);

  const [s, dispatch] = useReducer(
    connectionFormReducer,
    connection,
    initConnectionFormState,
  );

  const handleSave = useCallback(async () => {
    const check = validateConnection(s);
    if (!check.ok) {
      dispatch({ type: "saveError", message: check.reason });
      return;
    }
    dispatch({ type: "saveStart" });

    try {
      const input = buildConnectionInput(s);

      if (isEdit && connection) {
        await updateConnection(connection.id, input);
      } else {
        await createConnection(input);
      }

      dispatch({ type: "saveDone" });
      onClose();
    } catch (err) {
      dispatch({
        type: "saveError",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [s, isEdit, connection, onClose]);

  const handleTest = useCallback(async () => {
    if (!isEdit || !connection) return;
    dispatch({ type: "testStart" });

    try {
      await testConnection(connection.id);
      dispatch({ type: "testSuccess" });
    } catch (err) {
      dispatch({
        type: "testFailure",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }, [isEdit, connection]);

  const handleOverlayClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === overlayRef.current) {
        onClose();
      }
    },
    [onClose],
  );

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  const isSqlite = s.driver === "sqlite";
  const driverColor = DRIVER_CONFIG[s.driver].color;

  return (
    <Portal>
      <Box
        ref={overlayRef}
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1000}
        display="flex"
        alignItems="center"
        justifyContent="center"
        onClick={handleOverlayClick}
      >
        <Box
          bg="bg"
          border="1px solid"
          borderColor="border"
          rounded="xl"
          shadow="2xl"
          w="440px"
          maxH="85vh"
          overflowY="auto"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <Flex
            align="center"
            px={5}
            py={3}
            borderBottom="1px solid"
            borderColor="border"
          >
            <HStack gap={2} flex={1}>
              <Box color={`${driverColor}.400`}>
                <LuDatabase size={16} />
              </Box>
              <Text fontWeight="semibold" fontSize="sm">
                {isEdit ? "Edit Connection" : "New Connection"}
              </Text>
            </HStack>
            <IconButton
              aria-label="Close"
              variant="ghost"
              size="xs"
              onClick={onClose}
            >
              <LuX />
            </IconButton>
          </Flex>

          <VStack gap={0} align="stretch">
            {/* Name + Driver picker */}
            <VStack gap={3} p={4} pb={3} align="stretch">
              <Input
                size="sm"
                value={s.name}
                onChange={(e) =>
                  dispatch({
                    type: "setField",
                    field: "name",
                    value: e.target.value,
                  })
                }
                placeholder="Connection name"
                fontWeight="medium"
              />
              <DriverSelector
                value={s.driver}
                onChange={(driver) =>
                  dispatch({ type: "setDriver", driver, isEdit })
                }
              />
            </VStack>

            {/* Driver-specific fields */}
            <Box bg="bg.subtle" mx={4} rounded="lg" p={3} mb={3}>
              <VStack gap={2.5} align="stretch">
                {isSqlite ? (
                  <SqliteFields
                    dbName={s.dbName}
                    onDbNameChange={(value) =>
                      dispatch({ type: "setField", field: "dbName", value })
                    }
                  />
                ) : (
                  <NetworkFields
                    driver={s.driver}
                    host={s.host}
                    onHostChange={(value) =>
                      dispatch({ type: "setField", field: "host", value })
                    }
                    port={s.port}
                    onPortChange={(value) =>
                      dispatch({ type: "setField", field: "port", value })
                    }
                    dbName={s.dbName}
                    onDbNameChange={(value) =>
                      dispatch({ type: "setField", field: "dbName", value })
                    }
                    username={s.username}
                    onUsernameChange={(value) =>
                      dispatch({ type: "setField", field: "username", value })
                    }
                    password={s.password}
                    onPasswordChange={(value) =>
                      dispatch({ type: "setField", field: "password", value })
                    }
                    sslMode={s.sslMode}
                    onSslModeChange={(value) =>
                      dispatch({ type: "setField", field: "sslMode", value })
                    }
                  />
                )}
              </VStack>
            </Box>

            {/* Connection-string preview */}
            <Box mx={4} mb={3}>
              <Text
                fontSize="2xs"
                fontFamily="mono"
                color="fg.muted"
                bg="bg.subtle"
                px={3}
                py={1.5}
                rounded="md"
                truncate
              >
                {buildConnectionPreview(
                  s.driver,
                  s.host,
                  s.port,
                  s.dbName,
                  s.username,
                )}
              </Text>
            </Box>

            <AdvancedFields
              open={s.showAdvanced}
              onToggle={() => dispatch({ type: "toggleAdvanced" })}
              timeoutMs={s.timeoutMs}
              onTimeoutMsChange={(value) =>
                dispatch({ type: "setField", field: "timeoutMs", value })
              }
              queryTimeoutMs={s.queryTimeoutMs}
              onQueryTimeoutMsChange={(value) =>
                dispatch({
                  type: "setField",
                  field: "queryTimeoutMs",
                  value,
                })
              }
              ttlSeconds={s.ttlSeconds}
              onTtlSecondsChange={(value) =>
                dispatch({ type: "setField", field: "ttlSeconds", value })
              }
              maxPoolSize={s.maxPoolSize}
              onMaxPoolSizeChange={(value) =>
                dispatch({ type: "setField", field: "maxPoolSize", value })
              }
            />

            {s.testResult && (
              <Box mx={4} mb={3}>
                <Badge
                  colorPalette={s.testResult === "success" ? "green" : "red"}
                  variant="subtle"
                  px={2}
                  py={1}
                  fontSize="xs"
                  w="100%"
                >
                  {s.testResult === "success"
                    ? "Connection successful"
                    : `Connection failed${
                        s.testError ? `: ${s.testError}` : ""
                      }`}
                </Badge>
              </Box>
            )}

            {s.error && (
              <Box mx={4} mb={3}>
                <Badge
                  colorPalette="red"
                  variant="subtle"
                  px={2}
                  py={1}
                  fontSize="xs"
                  w="100%"
                >
                  {s.error}
                </Badge>
              </Box>
            )}
          </VStack>

          {/* Footer */}
          <Flex
            px={4}
            py={3}
            borderTop="1px solid"
            borderColor="border"
            gap={2}
            justify="flex-end"
          >
            {isEdit && (
              <Box
                as="button"
                display="flex"
                alignItems="center"
                gap={1}
                px={3}
                py={1.5}
                rounded="md"
                fontSize="sm"
                bg="bg.subtle"
                _hover={{ bg: "bg.emphasized" }}
                onClick={handleTest}
                opacity={s.testing ? 0.5 : 1}
                pointerEvents={s.testing ? "none" : "auto"}
                mr="auto"
              >
                {s.testing ? <Spinner size="xs" /> : <LuPlugZap size={14} />}
                <Text fontSize="xs">Test</Text>
              </Box>
            )}
            <Box
              as="button"
              px={3}
              py={1.5}
              rounded="md"
              fontSize="sm"
              bg="bg.subtle"
              _hover={{ bg: "bg.emphasized" }}
              onClick={onClose}
            >
              Cancel
            </Box>
            <Box
              as="button"
              px={4}
              py={1.5}
              rounded="md"
              fontSize="sm"
              fontWeight="medium"
              bg={`${driverColor}.500`}
              color="white"
              _hover={{ bg: `${driverColor}.600` }}
              onClick={handleSave}
              opacity={s.saving || !s.name.trim() ? 0.5 : 1}
              pointerEvents={s.saving || !s.name.trim() ? "none" : "auto"}
            >
              {s.saving ? <Spinner size="xs" /> : isEdit ? "Save" : "Create"}
            </Box>
          </Flex>
        </Box>
      </Box>
    </Portal>
  );
}
