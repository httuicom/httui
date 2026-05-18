// Canvas §6 Environments — single env card.
//
// Renders the summary + chips + active pill. Click anywhere on the
// card body fires `onActivate(filename)`. The ⋮ menu in the top-right
// surfaces row-level actions (Clone / Rename / Delete).
//
// Activation animations are pure-Chakra (no extra dep): the ACTIVE
// pill mounts/unmounts via `<Presence>` with scale-in/scale-out, and
// the card itself runs a one-shot pulse keyframe (border ring + bg
// flash) the first render after `env.isActive` flips false → true.

import {
  Box,
  Flex,
  IconButton,
  Menu,
  Portal,
  Text,
  chakra,
} from "@chakra-ui/react";
import { useEffect, useRef, useState } from "react";
import { LuCopy, LuEllipsisVertical, LuPencil, LuTrash2 } from "react-icons/lu";

import type { EnvironmentSummary } from "./envs-meta";

export interface EnvironmentCardProps {
  env: EnvironmentSummary;
  onActivate?: (filename: string) => void;
  onClone?: (filename: string) => void;
  onRename?: (filename: string) => void;
  onDelete?: (filename: string) => void;
}

const PULSE_MS = 800;

export function EnvironmentCard({
  env,
  onActivate,
  onClone,
  onRename,
  onDelete,
}: EnvironmentCardProps) {
  const interactive = !!onActivate;
  const BodyComp = interactive ? chakra.button : chakra.div;
  const hasActions = !!(onClone || onRename || onDelete);

  // Detect false → true transition on isActive to fire a one-shot
  // pulse animation. Skip the initial mount so every card on first
  // render does not pulse at once.
  const wasActiveRef = useRef(env.isActive);
  const [pulse, setPulse] = useState(false);
  useEffect(() => {
    if (env.isActive && !wasActiveRef.current) {
      wasActiveRef.current = true;
      setPulse(true);
      const t = window.setTimeout(() => setPulse(false), PULSE_MS);
      return () => window.clearTimeout(t);
    }
    wasActiveRef.current = env.isActive;
    return undefined;
  }, [env.isActive]);

  return (
    <Box
      data-testid={`environment-card-${env.filename}`}
      data-active={env.isActive || undefined}
      data-personal={env.isPersonal || undefined}
      data-temporary={env.isTemporary || undefined}
      data-pulse={pulse || undefined}
      borderWidth="1px"
      borderColor={env.isActive ? "brand.fg" : "border"}
      bg={env.isActive ? "bg.subtle" : "bg.muted"}
      borderRadius="6px"
      position="relative"
      transition="border-color 200ms ease, background-color 200ms ease"
      css={{
        '&[data-pulse="true"]': {
          animation: `envCardPulse ${PULSE_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`,
        },
        "@keyframes envCardPulse": {
          "0%": {
            boxShadow: "0 0 0 0 var(--chakra-colors-brand-fg)",
            transform: "scale(1)",
          },
          "20%": {
            boxShadow: "0 0 0 4px var(--chakra-colors-brand-fg)",
            transform: "scale(1.04)",
          },
          "100%": {
            boxShadow: "0 0 0 14px transparent",
            transform: "scale(1)",
          },
        },
        "@media (prefers-reduced-motion: reduce)": {
          '&[data-pulse="true"]': { animation: "none" },
        },
      }}
      _hover={
        interactive
          ? {
              bg: "bg.subtle",
              borderColor: env.isActive ? "brand.fg" : "fg.subtle",
            }
          : undefined
      }
    >
      <BodyComp
        type={interactive ? "button" : undefined}
        onClick={interactive ? () => onActivate?.(env.filename) : undefined}
        textAlign="left"
        cursor={interactive ? "pointer" : "default"}
        bg="transparent"
        border="none"
        width="100%"
        px={3}
        py={2.5}
      >
        <Flex justify="space-between" align="flex-start" gap={2} mb={1.5}>
          <Text
            fontFamily="serif"
            fontSize="14px"
            fontWeight={500}
            color="fg"
            truncate
            data-testid={`environment-card-${env.filename}-name`}
          >
            {env.name}
          </Text>
          {env.isActive && (
            <Box
              data-testid={`environment-card-${env.filename}-active-pill`}
              data-env-active-pill="true"
              fontFamily="mono"
              fontSize="9px"
              fontWeight="bold"
              letterSpacing="0.04em"
              color="brand.contrast"
              bg="brand.fg"
              borderRadius="999px"
              px={1.5}
              py={0.5}
              mr={hasActions ? "28px" : 0}
              transformOrigin="center"
            >
              ACTIVE
            </Box>
          )}
        </Flex>

        <Flex
          gap={3}
          fontFamily="mono"
          fontSize="11px"
          color="fg.muted"
          mb={1.5}
        >
          <Text data-testid={`environment-card-${env.filename}-vars`}>
            {env.varCount} {env.varCount === 1 ? "var" : "vars"}
          </Text>
          <Text data-testid={`environment-card-${env.filename}-conns`}>
            {env.connectionsUsedCount === 0
              ? "all conns"
              : `${env.connectionsUsedCount} ${
                  env.connectionsUsedCount === 1 ? "conn" : "conns"
                }`}
          </Text>
        </Flex>

        <Flex gap={1} flexWrap="wrap">
          {env.isPersonal && (
            <Chip testId={`environment-card-${env.filename}-personal-chip`}>
              personal
            </Chip>
          )}
          {env.isTemporary && (
            <Chip testId={`environment-card-${env.filename}-temporary-chip`}>
              temporary
            </Chip>
          )}
        </Flex>

        {env.description && (
          <Text
            fontSize="10px"
            color="fg.subtle"
            mt={1.5}
            truncate
            title={env.description}
            data-testid={`environment-card-${env.filename}-description`}
          >
            {env.description}
          </Text>
        )}
      </BodyComp>

      {hasActions && (
        <Box position="absolute" top={1.5} right={1.5}>
          <Menu.Root positioning={{ placement: "bottom-end" }}>
            <Menu.Trigger asChild>
              <IconButton
                data-testid={`environment-card-${env.filename}-more`}
                aria-label="Environment actions"
                variant="ghost"
                size="xs"
                onClick={(e) => e.stopPropagation()}
              >
                <LuEllipsisVertical />
              </IconButton>
            </Menu.Trigger>
            <Portal>
              <Menu.Positioner>
                <Menu.Content>
                  {onClone && (
                    <Menu.Item
                      value="clone"
                      onSelect={() => onClone(env.filename)}
                    >
                      <LuCopy />
                      Clone
                    </Menu.Item>
                  )}
                  {onRename && (
                    <Menu.Item
                      value="rename"
                      onSelect={() => onRename(env.filename)}
                    >
                      <LuPencil />
                      Rename
                    </Menu.Item>
                  )}
                  {onDelete && (
                    <Menu.Item
                      value="delete"
                      onSelect={() => onDelete(env.filename)}
                      color="red.fg"
                    >
                      <LuTrash2 />
                      Delete
                    </Menu.Item>
                  )}
                </Menu.Content>
              </Menu.Positioner>
            </Portal>
          </Menu.Root>
        </Box>
      )}
    </Box>
  );
}

function Chip({
  children,
  testId,
}: {
  children: React.ReactNode;
  testId: string;
}) {
  return (
    <Box
      as="span"
      data-testid={testId}
      fontFamily="mono"
      fontSize="9px"
      color="fg.muted"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="4px"
      px={1.5}
      py={0.5}
    >
      {children}
    </Box>
  );
}
