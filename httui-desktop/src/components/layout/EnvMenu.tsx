// Status-bar env switcher dropdown — replaces the SegmentedEnvSwitcher
// in the TopBar. The trigger reads as a status-bar cell (dot + name)
// and the popover lists every env with a checkmark on the active one.
//
// Pure presentational over the environment store: parent wires
// `environments`, `activeEnvironment`, `onSwitch`.
//
// optionally controlled (`open` / `onOpenChange`) so
// ⌘E can open it; first 9 envs get numeric shortcuts (1-9); a
// "Clone <active>" quick action sits at the foot of the list.

import { Box, HStack, Menu, Portal, chakra } from "@chakra-ui/react";
import { useEffect } from "react";
import { LuCheck, LuCopy } from "react-icons/lu";

import { Dot, type DotVariant } from "@/components/atoms";
import type { Environment } from "@/lib/tauri/commands";

const Trigger = chakra("button");

/** Max envs that get a `1`-`9` numeric shortcut; the rest are
 * click/typeahead only (the dropdown scrolls). */
const MAX_NUMERIC = 9;

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
  /** Controlled open state (⌘E). Omit for uncontrolled (click-only). */
  open?: boolean;
  /** Fires on every open/close transition when controlled. */
  onOpenChange?: (open: boolean) => void;
  /** When set, renders a "Clone <active>" footer item. */
  onRequestClone?: () => void;
}

export function EnvMenu({
  environments,
  activeEnvironment,
  onSwitch,
  open,
  onOpenChange,
  onRequestClone,
}: EnvMenuProps) {
  const label = activeEnvironment?.name ?? "no env";
  const controlled = open !== undefined;

  // Numeric shortcuts (1-9) while the dropdown is open. Only wired in
  // controlled mode — the ⌘E path is the only one that needs them.
  useEffect(() => {
    if (!controlled || !open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const n = Number(e.key);
      if (!Number.isInteger(n) || n < 1 || n > MAX_NUMERIC) return;
      const env = environments[n - 1];
      if (!env) return;
      e.preventDefault();
      onSwitch(env.id);
      onOpenChange?.(false);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [controlled, open, environments, onSwitch, onOpenChange]);

  return (
    <Menu.Root
      {...(controlled
        ? { open, onOpenChange: (e) => onOpenChange?.(e.open) }
        : {})}
    >
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
            minW="220px"
            maxH="320px"
            overflowY="auto"
            bg="bg"
            borderWidth="1px"
            borderColor="border"
            shadow="2xl"
          >
            {environments.length === 0 ? (
              <Box px={3} py={2} fontSize="11px" color="fg.subtle">
                No environments
              </Box>
            ) : (
              environments.map((env, i) => {
                const isActive = env.id === activeEnvironment?.id;
                const numeric = i < MAX_NUMERIC ? String(i + 1) : null;
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
                      {numeric && (
                        <Box
                          as="span"
                          data-testid={`env-numeric-${numeric}`}
                          minW="16px"
                          textAlign="center"
                          fontFamily="mono"
                          fontSize="10px"
                          color="fg.subtle"
                          borderWidth="1px"
                          borderColor="border"
                          borderRadius="3px"
                          px={1}
                        >
                          {numeric}
                        </Box>
                      )}
                    </HStack>
                  </Menu.Item>
                );
              })
            )}

            {onRequestClone && activeEnvironment && (
              <>
                <Menu.Separator />
                <Menu.Item
                  value="__clone__"
                  data-testid="env-menu-clone"
                  onSelect={() => onRequestClone()}
                  cursor="pointer"
                  px={2}
                  py={1.5}
                  borderRadius="3px"
                >
                  <HStack gap={2} w="100%">
                    <Box w="14px" display="inline-flex" justifyContent="center">
                      <LuCopy size={12} />
                    </Box>
                    <Box
                      flex={1}
                      fontFamily="mono"
                      fontSize="12px"
                      color="fg.muted"
                    >
                      Clone {activeEnvironment.name}
                    </Box>
                  </HStack>
                </Menu.Item>
              </>
            )}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
