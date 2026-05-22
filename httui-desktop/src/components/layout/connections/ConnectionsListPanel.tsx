// Center column of the Connections page: header, search, list rows, empty state.
// Pure presentational — counts come from the consumer.

import { Flex, HStack, Stack, Text } from "@chakra-ui/react";
import { LuPlus } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";
import { MasterDetailListHeader } from "@/components/layout/shared";
import { ConnectionListRow, type ListRowItem } from "./ConnectionListRow";

export interface ListStatusCounts {
  total: number;
  ok: number;
  slow: number;
  down: number;
}

export interface ConnectionsListPanelProps {
  status: ListStatusCounts;
  searchValue: string;
  onSearchChange: (value: string) => void;
  onCreateNew: () => void;
  /** Empty array → empty-state hint renders. */
  rows?: ListRowItem[];
  selectedId?: string | null;
  onSelectRow?: (id: string) => void;
  /** ⋮ row-actions. If all omitted the menu trigger is hidden. */
  onEditRow?: (id: string) => void;
  onTestRow?: (id: string) => void;
  onDuplicateRow?: (id: string) => void;
  onDeleteRow?: (id: string) => void;
  /** Empty-state hint shown when `rows` is empty / undefined. */
  emptyHint?: string;
}

const SEARCH_PLACEHOLDER = "Search by name, host, env… ⌘K";

export function ConnectionsListPanel({
  status,
  searchValue,
  onSearchChange,
  onCreateNew,
  rows,
  selectedId = null,
  onSelectRow,
  onEditRow,
  onTestRow,
  onDuplicateRow,
  onDeleteRow,
  emptyHint = "Select a connection or create a new one",
}: ConnectionsListPanelProps) {
  return (
    <Stack
      data-testid="connections-list-panel"
      flex={1}
      h="full"
      gap={3}
      px={5}
      py={4}
      align="stretch"
      overflowY="auto"
    >
      <MasterDetailListHeader
        title="Connections"
        subtitleSlot={
          <HStack
            gap={1}
            data-testid="connections-list-status"
            fontFamily="mono"
            fontSize="11px"
          >
            <Text color="fg.muted">{status.total}</Text>
            <Text color="fg.subtle">·</Text>
            <Text color="green.fg">{status.ok} ok</Text>
            <Text color="fg.subtle">·</Text>
            <Text color="yellow.fg">{status.slow} slow</Text>
            <Text color="fg.subtle">·</Text>
            <Text color="red.fg">{status.down} down</Text>
          </HStack>
        }
        actionsSlot={
          <Btn
            variant="primary"
            data-testid="connections-create-new"
            onClick={onCreateNew}
          >
            <LuPlus size={12} /> New
          </Btn>
        }
      />

      <Input
        data-testid="connections-search"
        type="text"
        value={searchValue}
        placeholder={SEARCH_PLACEHOLDER}
        onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
          onSearchChange(e.target.value)
        }
      />

      {rows && rows.length > 0 ? (
        <Stack
          flex={1}
          gap={0}
          align="stretch"
          data-testid="connections-list-rows"
          overflowY="auto"
        >
          {rows.map((row) => (
            <ConnectionListRow
              key={row.id}
              item={row}
              selected={row.id === selectedId}
              onSelect={(id) => onSelectRow?.(id)}
              onEdit={onEditRow}
              onTest={onTestRow}
              onDuplicate={onDuplicateRow}
              onDelete={onDeleteRow}
            />
          ))}
        </Stack>
      ) : (
        <Flex
          flex={1}
          align="center"
          justify="center"
          data-testid="connections-list-empty"
        >
          <Text fontSize="13px" color="fg.subtle">
            {emptyHint}
          </Text>
        </Flex>
      )}
    </Stack>
  );
}
