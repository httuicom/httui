// Canvas §5 — Connections refined page (Epic 42 Story 01).
//
// 3-column grid: 220px kind sidebar / 1fr list / 420px detail.
// Slice 2 (this file): consumes a `connections: Connection[]` +
// optional `enrichment` array, derives sidebar counts / env summary
// / list rows / status counts via the pure helpers in
// `connections-derive.ts`, and threads selection through the
// detail panel.

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
  /** Raw connection list from `listConnections()`. Optional —
   * defaults to empty so the page renders the empty state. */
  connections?: Connection[];
  /** Per-row enrichment (env / latency / uses) keyed by
   * connection id. Defaults to empty; rows then render with
   * "untested" status / 0 uses / null env. */
  enrichment?: ConnectionEnrichment[];
  /** Override sidebar counts (e.g. for tests). When omitted,
   * counts derive from `connections`. */
  countsByKind?: Partial<Record<ConnectionKind, number>>;
  /** Override env summary list. When omitted, derives from
   * `enrichment`. */
  envs?: EnvSummary[];
  onCreateNew?: () => void;
  /** ⋮ row-menu handlers. */
  onEditRow?: (id: string) => void;
  onTestRow?: (id: string) => void;
  onDuplicateRow?: (id: string) => void;
  onDeleteRow?: (id: string) => void;
  /** Save handler for the credentials Edit/Save flow (Story 02). */
  onSaveCredentials?: (
    id: string,
    input: UpdateConnectionInput,
  ) => Promise<void> | void;
  /** Rotate-password handler (Story 02). */
  onRotatePassword?: (id: string, newPassword: string) => Promise<void> | void;
  /** When provided, the detail panel "Edit" delegates to this
   * (opens the modal in edit mode) instead of inline edit. */
  onRequestEditCredentials?: (id: string) => void;
  /** Story 03 — schema state for the selected connection. */
  schemaByConnection?: Record<
    string,
    {
      schema: ConnectionSchema | null;
      loading: boolean;
      error: string | null;
    }
  >;
  /** Hot-tables map (canvas: top 5 from `block_run_history`). */
  hotTablesByConnection?: Record<string, HotTableEntry[]>;
  /** Click "Refresh" in the schema preview — consumer should call
   * `useSchemaCacheStore.refresh(id)`. */
  onRefreshSchema?: (id: string) => void;
  /** Story 04 — runbook usages keyed by connection id. Consumer
   * fills via a vault-grep Tauri command driven by
   * `connection-usages.ts`. */
  usagesByConnection?: Record<string, RunbookUsage[]>;
  /** True while the consumer is loading usages for a connection. */
  usagesLoadingByConnection?: Record<string, boolean>;
  /** Click on a usage row → consumer opens the file at the line
   * (typically `useEditorSession.handleFileSelect` + cursor scroll). */
  onOpenUsage?: (filePath: string, line: number) => void;
  /** Story 05 — footer actions, dispatched with the selected
   * connection's id. */
  onTestConnection?: (id: string) => Promise<number>;
  onDuplicateConnection?: (id: string) => Promise<void> | void;
  onDeleteConnection?: (id: string) => Promise<void> | void;
  /** Controlled selection (V4). When provided, the page becomes a
   * controlled component and emits changes via `onSelectId`.
   * Omit both to keep the legacy uncontrolled behaviour (used by
   * the Story 02-05 test suite). */
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
  const [selectedKind, setSelectedKind] = useState<ConnectionKind | null>(
    null,
  );
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
    <Flex
      data-testid="connections-page"
      h="full"
      w="full"
      overflow="hidden"
    >
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
            ? schemaByConnection?.[selectedConnection.id]?.schema ?? null
            : null
        }
        schemaLoading={
          selectedConnection
            ? schemaByConnection?.[selectedConnection.id]?.loading ?? false
            : false
        }
        schemaError={
          selectedConnection
            ? schemaByConnection?.[selectedConnection.id]?.error ?? null
            : null
        }
        hotTables={
          selectedConnection
            ? hotTablesByConnection?.[selectedConnection.id] ?? []
            : []
        }
        onRefreshSchema={
          selectedConnection && onRefreshSchema
            ? () => onRefreshSchema(selectedConnection.id)
            : undefined
        }
        usages={
          selectedConnection
            ? usagesByConnection?.[selectedConnection.id] ?? []
            : []
        }
        usagesLoading={
          selectedConnection
            ? usagesLoadingByConnection?.[selectedConnection.id] ?? false
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
