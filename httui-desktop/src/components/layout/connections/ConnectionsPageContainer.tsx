// Smart wrapper around <ConnectionsPage /> (V4). Owns data fetching,
// IPC dispatch, and cross-store wiring; the presentational page
// stays prop-driven and trivially testable.
//
// V3 lesson: keep this lean. If a feature doesn't have a clean home,
// don't invent a workspace tab to host it.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfigSyncedResource } from "@/hooks/useConfigSyncedResource";

import {
  findConnectionUses,
  testConnection,
  type UpdateConnectionInput,
} from "@/lib/tauri/connections";
import { useConnectionsStore } from "@/stores/connections";
import { useSchemaCacheStore } from "@/stores/schemaCache";
import { useWorkspaceStore } from "@/stores/workspace";
import type { RunbookUsage } from "./connection-usages";
import { ConnectionsPage } from "./ConnectionsPage";
import { NewConnectionModalContainer } from "./NewConnectionModalContainer";

interface ConnectionsPageContainerProps {
  onNavigateFile?: (filePath: string) => void;
}

export function ConnectionsPageContainer({
  onNavigateFile,
}: ConnectionsPageContainerProps) {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const connections = useConnectionsStore((s) => s.connections);
  const refreshConnections = useConnectionsStore((s) => s.refresh);
  const createConn = useConnectionsStore((s) => s.createConnection);
  const updateConn = useConnectionsStore((s) => s.updateConnection);
  const deleteConn = useConnectionsStore((s) => s.deleteConnection);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [usagesByConnection, setUsagesByConnection] = useState<
    Record<string, RunbookUsage[]>
  >({});
  const [usagesLoading, setUsagesLoading] = useState<Record<string, boolean>>(
    {},
  );
  const [newOpen, setNewOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  const ensureSchema = useSchemaCacheStore((s) => s.ensureLoaded);
  const refreshSchema = useSchemaCacheStore((s) => s.refresh);
  const schemaByConn = useSchemaCacheStore((s) => s.byConnection);

  // Refresh on mount + on external `connections.toml` edits (the
  // backend emits `config-changed` category "connections" when the
  // file or its `.local` sibling changes on disk).
  useConfigSyncedResource("connections", refreshConnections);

  // The only field the prefetch effect reads from `connections` is
  // the selected connection's name. Derive it here so the effect can
  // depend on a stable string instead of the whole array.
  const selectedConnName = useMemo(
    () =>
      selectedId
        ? (connections.find((c) => c.id === selectedId)?.name ?? null)
        : null,
    [selectedId, connections],
  );

  // Pre-fetch schema + usages on selection change so the detail
  // panel renders without an extra click. Keyed on the selected
  // connection's *name* (B4): the old `connections`-array dep re-ran
  // the FS grep + ensureSchema on every unrelated store refresh
  // (test-ping, CRUD, config-changed) even when the selection was
  // unchanged. A rename still re-fires (name changes); a real
  // selection change still re-fires (selectedId changes).
  useEffect(() => {
    if (!selectedId || !vaultPath || !selectedConnName) return;
    void ensureSchema(selectedId);
    setUsagesLoading((m) => ({ ...m, [selectedConnName]: true }));
    findConnectionUses(vaultPath, selectedConnName)
      .then((r) => {
        const usages: RunbookUsage[] = r.map((u) => ({
          filePath: u.file,
          line: u.line,
          preview: null,
        }));
        setUsagesByConnection((m) => ({
          ...m,
          [selectedConnName]: usages,
        }));
      })
      .finally(() => {
        setUsagesLoading((m) => ({ ...m, [selectedConnName]: false }));
      });
  }, [selectedId, selectedConnName, vaultPath, ensureSchema]);

  const handleSaveCredentials = useCallback(
    async (id: string, input: UpdateConnectionInput) => {
      await updateConn(id, input);
    },
    [updateConn],
  );

  const handleRotatePassword = useCallback(
    async (id: string, newPassword: string) => {
      await updateConn(id, { password: newPassword });
    },
    [updateConn],
  );

  const handleTestConnection = useCallback(
    async (id: string): Promise<number> => {
      const start = performance.now();
      await testConnection(id);
      return performance.now() - start;
    },
    [],
  );

  const handleDuplicate = useCallback(
    async (id: string) => {
      const src = connections.find((c) => c.id === id);
      if (!src) return;
      await createConn({
        name: `${src.name}-copy`,
        driver: src.driver,
        host: src.host ?? undefined,
        port: src.port ?? undefined,
        database_name: src.database_name ?? undefined,
        username: src.username ?? undefined,
        ssl_mode: src.ssl_mode ?? undefined,
        is_readonly: src.is_readonly,
      });
    },
    [connections, createConn],
  );

  const handleDelete = useCallback(
    async (id: string) => {
      await deleteConn(id);
      if (selectedId === id) setSelectedId(null);
    },
    [deleteConn, selectedId],
  );

  // Slice the schemaCache map to the shape ConnectionsPage expects.
  const schemaProp = useMemo(() => {
    const out: Record<
      string,
      {
        schema: (typeof schemaByConn)[string]["schema"];
        loading: boolean;
        error: string | null;
      }
    > = {};
    for (const [id, entry] of Object.entries(schemaByConn)) {
      out[id] = {
        schema: entry.schema,
        loading: entry.loading,
        error: entry.error,
      };
    }
    return out;
  }, [schemaByConn]);

  const editing = editingId
    ? (connections.find((c) => c.id === editingId) ?? null)
    : null;
  const modalOpen = newOpen || Boolean(editing);

  return (
    <>
      <ConnectionsPage
        connections={connections}
        selectedId={selectedId}
        onSelectId={setSelectedId}
        onSaveCredentials={handleSaveCredentials}
        onRotatePassword={handleRotatePassword}
        onTestConnection={handleTestConnection}
        onDuplicateConnection={handleDuplicate}
        onDeleteConnection={handleDelete}
        schemaByConnection={schemaProp}
        onRefreshSchema={(id) => {
          void refreshSchema(id);
        }}
        usagesByConnection={usagesByConnection}
        usagesLoadingByConnection={usagesLoading}
        onOpenUsage={(filePath) => onNavigateFile?.(filePath)}
        onCreateNew={() => setNewOpen(true)}
        onEditRow={setEditingId}
        onRequestEditCredentials={setEditingId}
        onTestRow={(id) => {
          void testConnection(id);
        }}
        onDuplicateRow={handleDuplicate}
        onDeleteRow={handleDelete}
      />
      <NewConnectionModalContainer
        open={modalOpen}
        editing={editing}
        onClose={() => {
          setNewOpen(false);
          setEditingId(null);
        }}
        onCreated={() => {
          void refreshConnections();
        }}
      />
    </>
  );
}
