// Connections page: 3-column grid (220px sidebar / 1fr list / 420px detail).
// Derives sidebar counts, env summary, list rows, and status counts from
// `connections-derive.ts` and threads selection into the detail panel.

import { useMemo, useState } from "react";
import { Flex } from "@chakra-ui/react";

import type {
  Connection,
  UpdateConnectionInput,
} from "@/lib/tauri/connections";

import type { ConnectionSchema } from "@/stores/schemaCache";
import {
  ConnectionsKindSidebar,
  type EnvSummary,
} from "./ConnectionsKindSidebar";
import { ConnectionsListPanel } from "./ConnectionsListPanel";
import { ConnectionsDetailPanel } from "./ConnectionsDetailPanel";
import type { HotTableEntry } from "./ConnectionDetailSchemaPreview";
import type { RunbookUsage } from "./connection-usages";
import {
  buildListRows,
  countsByKind as deriveCountsByKind,
  envSummaries as deriveEnvSummaries,
  listStatusCounts,
  type ConnectionEnrichment,
} from "./connections-derive";
import type { ConnectionKind } from "./connection-kinds";

export interface ConnectionsPageProps {
  connections?: Connection[];
  enrichment?: ConnectionEnrichment[];
  /** Override derived sidebar counts (e.g. in tests). */
  countsByKind?: Partial<Record<ConnectionKind, number>>;
  /** Override derived env summary list. */
  envs?: EnvSummary[];
  onCreateNew?: () => void;
  /** ⋮ row-menu handlers. */
  onEditRow?: (id: string) => void;
  onTestRow?: (id: string) => void;
  onDuplicateRow?: (id: string) => void;
  onDeleteRow?: (id: string) => void;
  onSaveCredentials?: (
    id: string,
    input: UpdateConnectionInput,
  ) => Promise<void> | void;
  onRotatePassword?: (id: string, newPassword: string) => Promise<void> | void;
  /** Open the modal in edit mode instead of inline editing. */
  onRequestEditCredentials?: (id: string) => void;
  schemaByConnection?: Record<
    string,
    {
      schema: ConnectionSchema | null;
      loading: boolean;
      error: string | null;
    }
  >;
  hotTablesByConnection?: Record<string, HotTableEntry[]>;
  onRefreshSchema?: (id: string) => void;
  usagesByConnection?: Record<string, RunbookUsage[]>;
  usagesLoadingByConnection?: Record<string, boolean>;
  onOpenUsage?: (filePath: string, line: number) => void;
  onTestConnection?: (id: string) => Promise<number>;
  onDuplicateConnection?: (id: string) => Promise<void> | void;
  onDeleteConnection?: (id: string) => Promise<void> | void;
  /** Controlled selection — omit both props for uncontrolled behaviour. */
  selectedId?: string | null;
  onSelectId?: (id: string | null) => void;
}

export function ConnectionsPage({
  connections = [],
  enrichment = [],
  countsByKind: countsByKindOverride,
  envs: envsOverride,
  onCreateNew,
  onEditRow,
  onTestRow,
  onDuplicateRow,
  onDeleteRow,
  onSaveCredentials,
  onRotatePassword,
  onRequestEditCredentials,
  schemaByConnection,
  hotTablesByConnection,
  onRefreshSchema,
  usagesByConnection,
  usagesLoadingByConnection,
  onOpenUsage,
  onTestConnection,
  onDuplicateConnection,
  onDeleteConnection,
  selectedId: selectedIdProp,
  onSelectId,
}: ConnectionsPageProps) {
  const [selectedKind, setSelectedKind] = useState<ConnectionKind | null>(null);
  const [searchValue, setSearchValue] = useState("");
  const [internalSelectedId, setInternalSelectedId] = useState<string | null>(
    null,
  );
  const isControlled = selectedIdProp !== undefined;
  const selectedId = isControlled ? selectedIdProp : internalSelectedId;
  const setSelectedId = (id: string | null) => {
    if (!isControlled) setInternalSelectedId(id);
    onSelectId?.(id);
  };

  const countsByKind = useMemo(
    () => countsByKindOverride ?? deriveCountsByKind(connections),
    [countsByKindOverride, connections],
  );

  const envs = useMemo(
    () => envsOverride ?? deriveEnvSummaries(enrichment),
    [envsOverride, enrichment],
  );

  const rows = useMemo(
    () =>
      buildListRows({
        connections,
        enrichment,
        kindFilter: selectedKind,
        search: searchValue,
      }),
    [connections, enrichment, selectedKind, searchValue],
  );

  const status = useMemo(() => listStatusCounts(rows), [rows]);

  const selectedConnection = useMemo(() => {
    if (selectedId === null) return null;
    return connections.find((c) => c.id === selectedId) ?? null;
  }, [selectedId, connections]);

  const selectedConnectionName = selectedConnection?.name ?? null;

  const handleCreateNew = useMemo(
    () => onCreateNew ?? (() => {}),
    [onCreateNew],
  );

  return (
    <Flex data-testid="connections-page" h="full" w="full" overflow="hidden">
      <ConnectionsKindSidebar
        countsByKind={countsByKind}
        selectedKind={selectedKind}
        onSelectKind={setSelectedKind}
        envs={envs}
      />
      <ConnectionsListPanel
        status={status}
        searchValue={searchValue}
        onSearchChange={setSearchValue}
        onCreateNew={handleCreateNew}
        rows={rows}
        selectedId={selectedId}
        onSelectRow={setSelectedId}
        onEditRow={onEditRow}
        onTestRow={onTestRow}
        onDuplicateRow={onDuplicateRow}
        onDeleteRow={onDeleteRow}
      />
      <ConnectionsDetailPanel
        selectedConnectionName={selectedConnectionName}
        selectedConnection={selectedConnection}
        onSaveCredentials={
          selectedConnection && onSaveCredentials
            ? (input) => onSaveCredentials(selectedConnection.id, input)
            : undefined
        }
        onRotatePassword={
          selectedConnection && onRotatePassword
            ? (pw) => onRotatePassword(selectedConnection.id, pw)
            : undefined
        }
        onRequestEdit={
          selectedConnection && onRequestEditCredentials
            ? () => onRequestEditCredentials(selectedConnection.id)
            : undefined
        }
        schema={
          selectedConnection
            ? (schemaByConnection?.[selectedConnection.id]?.schema ?? null)
            : null
        }
        schemaLoading={
          selectedConnection
            ? (schemaByConnection?.[selectedConnection.id]?.loading ?? false)
            : false
        }
        schemaError={
          selectedConnection
            ? (schemaByConnection?.[selectedConnection.id]?.error ?? null)
            : null
        }
        hotTables={
          selectedConnection
            ? (hotTablesByConnection?.[selectedConnection.id] ?? [])
            : []
        }
        onRefreshSchema={
          selectedConnection && onRefreshSchema
            ? () => onRefreshSchema(selectedConnection.id)
            : undefined
        }
        usages={
          selectedConnection
            ? (usagesByConnection?.[selectedConnection.id] ?? [])
            : []
        }
        usagesLoading={
          selectedConnection
            ? (usagesLoadingByConnection?.[selectedConnection.id] ?? false)
            : false
        }
        onOpenUsage={onOpenUsage}
        onTestConnection={
          selectedConnection && onTestConnection
            ? () => onTestConnection(selectedConnection.id)
            : undefined
        }
        onDuplicateConnection={
          selectedConnection && onDuplicateConnection
            ? () => onDuplicateConnection(selectedConnection.id)
            : undefined
        }
        onDeleteConnection={
          selectedConnection && onDeleteConnection
            ? () => onDeleteConnection(selectedConnection.id)
            : undefined
        }
      />
    </Flex>
  );
}
