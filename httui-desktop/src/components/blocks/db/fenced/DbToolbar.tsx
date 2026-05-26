import { Badge, Box, Flex, HStack, IconButton, Text } from "@chakra-ui/react";
import { LuListTree, LuPlay, LuSettings, LuSquare } from "react-icons/lu";
import type { DbBlockMetadata } from "@/lib/blocks/db-fence";
import type { Connection } from "@/lib/tauri/connections";
import type { ExecutionState } from "./shared";

interface DbToolbarProps {
  metadata: DbBlockMetadata;
  activeConnection: Connection | null;
  executionState: ExecutionState;
  onRun: () => void;
  onCancel: () => void;
  onExplain: () => void;
  onOpenSettings: () => void;
}

export function DbToolbar({
  metadata,
  activeConnection,
  executionState,
  onRun,
  onCancel,
  onExplain,
  onOpenSettings,
}: DbToolbarProps) {
  const running = executionState === "running";
  const dialectLabel = metadata.dialect.toLowerCase();
  const connLabel = activeConnection?.name ?? metadata.connection ?? undefined;
  const isReadonly = activeConnection?.is_readonly ?? false;

  return (
    <Flex
      className="cm-db-toolbar"
      gap={3}
      align="center"
      justify="space-between"
      minW={0}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {/* Identity row — [DB] alias / connection [RO|RW] [dialect-pill].
          Text stays at 14/13px so it never competes with the SQL below;
          the visual hierarchy (alias > connection > mode/dialect) comes
          from colour + weight, not font size. The RO/RW pill is only
          rendered when a connection is resolved — an un-set connection
          would make the pill meaningless. */}
      <HStack gap={3} align="center" minW={0} flex="1" overflow="hidden">
        <Badge
          colorPalette="blue"
          variant="solid"
          size="sm"
          flexShrink={0}
          fontFamily="mono"
          letterSpacing="0.05em"
          px={2}
          py={1}
          rounded="md"
        >
          DB
        </Badge>
        {metadata.alias && (
          <Text
            fontSize="14px"
            fontFamily="heading"
            fontWeight="700"
            whiteSpace="nowrap"
            overflow="hidden"
            textOverflow="ellipsis"
            flexShrink={1}
            minW={0}
            letterSpacing="-0.01em"
          >
            {metadata.alias}
          </Text>
        )}
        {connLabel && (
          <>
            <Box
              as="span"
              flexShrink={0}
              color="fg.muted"
              opacity={0.4}
              fontSize="sm"
              fontWeight="300"
            >
              /
            </Box>
            <Text
              fontSize="13px"
              fontFamily="heading"
              fontWeight="500"
              color="fg.muted"
              whiteSpace="nowrap"
              overflow="hidden"
              textOverflow="ellipsis"
              minW={0}
              flexShrink={1}
            >
              {connLabel}
            </Text>
          </>
        )}
        {activeConnection && (
          <Badge
            size="xs"
            variant="subtle"
            colorPalette={isReadonly ? "orange" : "green"}
            flexShrink={0}
            fontFamily="mono"
            fontWeight="600"
            letterSpacing="0.04em"
            px={2}
            py={0.5}
            rounded="md"
            title={
              isReadonly
                ? "Read-only: mutations prompt for confirmation"
                : "Read-write: mutations run without confirmation"
            }
          >
            {isReadonly ? "RO" : "RW"}
          </Badge>
        )}
        <Badge
          size="xs"
          variant="subtle"
          colorPalette="gray"
          flexShrink={0}
          fontFamily="mono"
          fontWeight="500"
          textTransform="lowercase"
          letterSpacing="0.02em"
          px={2}
          py={0.5}
          rounded="md"
        >
          {dialectLabel}
        </Badge>
      </HStack>

      {/* Actions — icon-only ghost buttons matching the HTTP block pattern.
          Order (left → right): Run/Cancel · EXPLAIN · Settings (spec §2.3).
          Run is colour-only (green icon), cancel inherits red. The remaining
          actions use the muted fg pair so they recede visually while the
          query is idle. */}
      <HStack gap={0} flexShrink={0}>
        {running ? (
          <IconButton
            size="xs"
            variant="ghost"
            colorPalette="red"
            aria-label="Cancel"
            onClick={onCancel}
            title="Cancel (⌘.)"
          >
            <LuSquare />
          </IconButton>
        ) : (
          <IconButton
            size="xs"
            variant="ghost"
            colorPalette="green"
            aria-label="Run"
            onClick={onRun}
            title="Run (⌘↵)"
            disabled={!activeConnection}
          >
            <LuPlay />
          </IconButton>
        )}
        <IconButton
          size="xs"
          variant="ghost"
          colorPalette="gray"
          aria-label="EXPLAIN"
          onClick={onExplain}
          title="EXPLAIN (⌘⇧E)"
          disabled={!activeConnection || running}
        >
          <LuListTree />
        </IconButton>
        <IconButton
          size="xs"
          variant="ghost"
          colorPalette="gray"
          aria-label="Settings"
          onClick={onOpenSettings}
          title="Settings"
        >
          <LuSettings />
        </IconButton>
      </HStack>
    </Flex>
  );
}
