import {
  Badge,
  Box,
  Button,
  Flex,
  HStack,
  IconButton,
  Menu,
  Portal,
  Text,
} from "@chakra-ui/react";
import { LuPlay, LuSettings, LuSquare } from "react-icons/lu";
import type { HttpMethod } from "@/lib/blocks/http-message";
import type { HttpBodyMode } from "@/lib/blocks/http-body-modes";
import { METHOD_COLORS, type ExecutionState } from "./shared";

interface HttpToolbarProps {
  alias: string | undefined;
  method: HttpMethod;
  host: string | null;
  mode: "raw" | "form";
  bodyMode: HttpBodyMode;
  executionState: ExecutionState;
  onRun: () => void;
  onCancel: () => void;
  onOpenSettings: () => void;
  onToggleMode: (next: "raw" | "form") => void;
  onPickBodyMode: (next: HttpBodyMode) => void;
}

const BODY_MODES: HttpBodyMode[] = [
  "none",
  "json",
  "xml",
  "text",
  "form-urlencoded",
  "multipart",
  "binary",
];

export function HttpToolbar({
  alias,
  method,
  host,
  mode,
  bodyMode,
  executionState,
  onRun,
  onCancel,
  onOpenSettings,
  onToggleMode,
  onPickBodyMode,
}: HttpToolbarProps) {
  const running = executionState === "running";
  return (
    <Flex
      align="center"
      gap={2}
      px={3}
      py={1.5}
      bg="bg.subtle"
      borderTopRadius="md"
      fontSize="sm"
      minH="36px"
    >
      <Badge colorPalette="blue" variant="subtle" textTransform="uppercase">
        HTTP
      </Badge>
      {alias && (
        <Text
          fontFamily="mono"
          color="fg.muted"
          truncate
          maxW="14ch"
          aria-label="alias"
        >
          {alias}
        </Text>
      )}
      <Box
        px={1.5}
        py={0.5}
        borderRadius="sm"
        bg="bg.muted"
        fontSize="xs"
        fontFamily="mono"
        color={METHOD_COLORS[method]}
        fontWeight="semibold"
      >
        {method}
      </Box>
      {host && (
        <Text
          fontFamily="mono"
          color="fg.muted"
          fontSize="xs"
          truncate
          maxW="32ch"
        >
          {host}
        </Text>
      )}
      <Box flex={1} />
      <HStack
        gap={0}
        borderRadius="sm"
        borderWidth="1px"
        borderColor="border.muted"
        overflow="hidden"
        aria-label="View mode"
      >
        <Button
          size="2xs"
          variant={mode === "raw" ? "solid" : "ghost"}
          borderRadius="0"
          onClick={() => onToggleMode("raw")}
          aria-pressed={mode === "raw"}
        >
          raw
        </Button>
        <Button
          size="2xs"
          variant={mode === "form" ? "solid" : "ghost"}
          borderRadius="0"
          onClick={() => onToggleMode("form")}
          aria-pressed={mode === "form"}
        >
          form
        </Button>
      </HStack>
      <Menu.Root positioning={{ placement: "bottom-end" }}>
        <Menu.Trigger asChild>
          <Button
            size="2xs"
            variant="outline"
            aria-label={`Body mode: ${bodyMode}`}
            title="Set Content-Type for request body"
            fontFamily="mono"
          >
            {bodyMode}
          </Button>
        </Menu.Trigger>
        <Portal>
          <Menu.Positioner>
            <Menu.Content minW="180px" py={1}>
              {BODY_MODES.map((m) => (
                <Menu.Item
                  key={m}
                  value={m}
                  onSelect={() => onPickBodyMode(m)}
                  fontFamily="mono"
                >
                  {m}
                </Menu.Item>
              ))}
            </Menu.Content>
          </Menu.Positioner>
        </Portal>
      </Menu.Root>
      {running ? (
        <IconButton
          aria-label="Cancel request"
          size="xs"
          variant="ghost"
          colorPalette="red"
          onClick={onCancel}
        >
          <LuSquare />
        </IconButton>
      ) : (
        <IconButton
          aria-label="Run request"
          size="xs"
          variant="ghost"
          colorPalette="green"
          onClick={onRun}
        >
          <LuPlay />
        </IconButton>
      )}
      <IconButton
        aria-label="Block settings"
        size="xs"
        variant="ghost"
        onClick={onOpenSettings}
      >
        <LuSettings />
      </IconButton>
    </Flex>
  );
}
