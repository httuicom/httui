import type { ReactNode } from "react";
import {
  Box,
  Flex,
  HStack,
  Input,
  Badge,
  Spinner,
  IconButton,
} from "@chakra-ui/react";
import {
  LuPenLine,
  LuColumns2,
  LuMonitorCheck,
  LuPlay,
  LuSquare,
  LuX,
} from "react-icons/lu";
import type { DisplayMode, ExecutionState } from "./ExecutableBlock";

interface StandaloneBlockShellProps {
  blockType: string;
  alias: string;
  displayMode: DisplayMode;
  executionState: ExecutionState;
  onAliasChange: (alias: string) => void;
  onDisplayModeChange: (mode: DisplayMode) => void;
  onRun: () => void;
  onCancel: () => void;
  inputSlot: ReactNode;
  outputSlot: ReactNode;
  selected?: boolean;
  statusText?: string | null;
  splitDirection?: "row" | "column";
  /** Extra text shown after alias in header (e.g. connection name) */
  headerMeta?: string | null;
  /** Called when user clicks the delete button */
  onDelete?: () => void;
}

const STATE_COLORS: Record<ExecutionState, string> = {
  idle: "gray",
  running: "blue",
  success: "green",
  error: "red",
  cached: "purple",
};

const STATE_LABELS: Record<ExecutionState, string> = {
  idle: "idle",
  running: "running",
  success: "success",
  error: "error",
  cached: "cached",
};

const BLOCK_LABELS: Record<string, string> = {
  http: "HTTP",
  db: "DB",
};

const MODE_ICONS: { mode: DisplayMode; label: string; icon: ReactNode }[] = [
  { mode: "input", label: "Input", icon: <LuPenLine /> },
  { mode: "split", label: "Split", icon: <LuColumns2 /> },
  { mode: "output", label: "Output", icon: <LuMonitorCheck /> },
];

export function StandaloneBlockShell({
  blockType,
  alias,
  displayMode,
  executionState,
  onAliasChange,
  onDisplayModeChange,
  onRun,
  onCancel,
  inputSlot,
  outputSlot,
  selected = false,
  statusText,
  splitDirection,
  headerMeta: _headerMeta,
  onDelete,
}: StandaloneBlockShellProps) {
  const isRunning = executionState === "running";
  const showInput = displayMode === "input" || displayMode === "split";
  const showOutput = displayMode === "output" || displayMode === "split";

  return (
    <Box
      border="1px solid"
      borderColor={selected ? "brand.500" : "border"}
      rounded="lg"
      overflow="hidden"
      my={2}
      bg="bg"
    >
      {/* Header */}
      <Flex
        align="center"
        gap={2}
        px={3}
        py={1.5}
        bg="bg.subtle"
        borderBottom="1px solid"
        borderColor="border"
      >
        <Badge
          size="sm"
          colorPalette={STATE_COLORS[executionState]}
          variant="solid"
          fontFamily="mono"
          fontSize="xs"
        >
          {BLOCK_LABELS[blockType] ?? blockType.toUpperCase()}
        </Badge>

        <Input
          size="xs"
          variant="flushed"
          placeholder="alias..."
          value={alias}
          onChange={(e) => onAliasChange(e.target.value)}
          fontFamily="mono"
          fontSize="xs"
          maxW="140px"
          color="fg.muted"
          onClick={(e) => e.stopPropagation()}
        />

        <HStack gap={0} ml="auto">
          {MODE_ICONS.map(({ mode, label, icon }) => (
            <IconButton
              key={mode}
              aria-label={label}
              size="2xs"
              variant={displayMode === mode ? "solid" : "ghost"}
              colorPalette="gray"
              onClick={(e) => {
                e.stopPropagation();
                onDisplayModeChange(mode);
              }}
              fontSize="xs"
            >
              {icon}
            </IconButton>
          ))}
        </HStack>

        <Badge
          size="sm"
          colorPalette={STATE_COLORS[executionState]}
          variant="subtle"
        >
          {isRunning && <Spinner size="xs" mr={1} />}
          {isRunning && statusText ? statusText : STATE_LABELS[executionState]}
        </Badge>

        <IconButton
          aria-label={isRunning ? "Cancel" : "Run"}
          size="xs"
          variant="ghost"
          colorPalette={isRunning ? "red" : "green"}
          onClick={(e) => {
            e.stopPropagation();
            if (isRunning) {
              onCancel();
            } else {
              onRun();
            }
          }}
        >
          {isRunning ? <LuSquare /> : <LuPlay />}
        </IconButton>

        {onDelete && (
          <IconButton
            aria-label="Delete block"
            size="xs"
            variant="ghost"
            colorPalette="gray"
            opacity={0.5}
            _hover={{ opacity: 1, colorPalette: "red" }}
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
          >
            <LuX />
          </IconButton>
        )}
      </Flex>

      {/* Content area */}
      <Flex
        direction={
          displayMode === "split"
            ? splitDirection === "column"
              ? "column"
              : { base: "column", md: "row" }
            : "column"
        }
        minH="40px"
      >
        <Box
          flex={showInput ? 1 : undefined}
          minW={displayMode === "split" ? "0" : undefined}
          borderRightWidth={
            displayMode === "split" && splitDirection !== "column"
              ? { base: "0", md: "1px" }
              : undefined
          }
          borderBottomWidth={
            displayMode === "split"
              ? splitDirection === "column"
                ? "1px"
                : { base: "1px", md: "0" }
              : undefined
          }
          borderStyle="solid"
          borderColor="border"
          overflow="hidden"
          css={{
            maxHeight: showInput ? "2000px" : "0",
            opacity: showInput ? 1 : 0,
            transition: "max-height 0.25s ease, opacity 0.2s ease",
          }}
        >
          {inputSlot}
        </Box>

        <Box
          flex={showOutput ? 1 : undefined}
          minW={displayMode === "split" ? "0" : undefined}
          overflow="hidden"
          css={{
            maxHeight: showOutput ? "2000px" : "0",
            opacity: showOutput ? 1 : 0,
            transition: "max-height 0.25s ease, opacity 0.2s ease",
          }}
        >
          {executionState === "idle" ? (
            <Flex
              align="center"
              justify="center"
              h="100%"
              minH="40px"
              color="fg.muted"
              fontSize="sm"
            >
              Run to see results
            </Flex>
          ) : (
            outputSlot
          )}
        </Box>
      </Flex>
    </Box>
  );
}
