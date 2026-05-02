// Status-bar env switcher dropdown — replaces the SegmentedEnvSwitcher
// in the TopBar. The trigger reads as a status-bar cell (dot + name)
// and the popover lists every env with a checkmark on the active one.
//
// Pure presentational over the environment store: parent wires
// `environments`, `activeEnvironment`, `onSwitch`.

import { Box, HStack, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuCheck } from "react-icons/lu";

import { Dot, type DotVariant } from "@/components/atoms";
import type { Environment } from "@/lib/tauri/commands";

const Trigger = chakra("button");

function envVariant(name: string | undefined | null): DotVariant {
  if (!name) return "idle";
  if (/^prod/i.test(name)) return "err";
  if (/^staging/i.test(name)) return "warn";
  return "ok";
}

export interface EnvMenuProps {
  /** All known environments. Empty array shows just the placeholder. */
  environments: Environment[];
  /** Active env (or null when none). Drives the trigger label + dot. */
  activeEnvironment: Environment | null;
  /** Switch handler. */
  onSwitch: (id: string) => void;
}

export function EnvMenu({
  environments,
  activeEnvironment,
  onSwitch,
}: EnvMenuProps) {
  const label = activeEnvironment?.name ?? "no env";

  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Trigger
          type="button"
          data-testid="status-env"
          data-atom="status-env-trigger"
          aria-label={`Environment ${label}`}
          bg="transparent"
          color="fg.1"
          fontFamily="mono"
          fontSize="11px"
          cursor="pointer"
          display="inline-flex"
          alignItems="center"
          gap={2}
          px={1}
          flexShrink={0}
          _hover={{ color: "fg" }}
        >
          <Dot variant={envVariant(activeEnvironment?.name)} />
          <Box as="span">{label}</Box>
        </Trigger>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            data-testid="env-menu"
            minW="200px"
            bg="bg"
            borderWidth="1px"
            borderColor="line"
            shadow="2xl"
          >
            {environments.length === 0 ? (
              <Box px={3} py={2} fontSize="11px" color="fg.3">
                No environments
              </Box>
            ) : (
              environments.map((env) => {
                const isActive = env.id === activeEnvironment?.id;
                return (
                  <Menu.Item
                    key={env.id}
                    value={env.id}
                    data-env-id={env.id}
                    data-active={isActive ? "true" : "false"}
                    onSelect={() => onSwitch(env.id)}
                    cursor="pointer"
                    px={2}
                    py={1.5}
                    borderRadius="3px"
                  >
                    <HStack gap={2} w="100%">
                      <Box
                        w="14px"
                        display="inline-flex"
                        justifyContent="center"
                      >
                        {isActive && <LuCheck size={12} />}
                      </Box>
                      <Dot variant={envVariant(env.name)} />
                      <Box
                        flex={1}
                        fontFamily="mono"
                        fontSize="12px"
                        fontWeight={isActive ? 600 : 500}
                      >
                        {env.name}
                      </Box>
                    </HStack>
                  </Menu.Item>
                );
              })
            )}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
