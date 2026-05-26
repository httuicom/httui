// Canvas §5 — right column of the Connections refined page (420px).
//
// Slice 1: empty/placeholder states.
// Slice 2 (wiring): selection cascade renders connection name.
// Slice 3 credentials section when a real `Connection` is
// passed in; placeholder when only the name is known (e.g. test
// fixtures or pre-load stubs).

import { Box, Stack, Text } from "@chakra-ui/react";

import { ConnectionDetailCredentials } from "./ConnectionDetailCredentials";
import {
  ConnectionDetailSchemaPreview,
  type HotTableEntry,
} from "./ConnectionDetailSchemaPreview";
import { ConnectionDetailUsedIn } from "./ConnectionDetailUsedIn";
import { ConnectionDetailFooter } from "./ConnectionDetailFooter";
import type { RunbookUsage } from "./connection-usages";
import type {
  Connection,
  UpdateConnectionInput,
} from "@/lib/tauri/connections";
import type { ConnectionSchema } from "@/stores/schemaCache";

export interface ConnectionsDetailPanelProps {
  /** Currently-selected connection name, or `null` for no
   * selection. */
  selectedConnectionName: string | null;
  /** Optional full Connection record — when present, the
   * Credentials section renders. When omitted (legacy
   * placeholder path), the panel falls back to the name-only
   * placeholder. */
  selectedConnection?: Connection | null;
  /** Save handler for the credentials Edit/Save flow. */
  onSaveCredentials?: (input: UpdateConnectionInput) => Promise<void> | void;
  /** Rotate-password handler. The consumer should write to the
   * keychain and update the `{{keychain:…}}` ref in
   * `connections.toml`. */
  onRotatePassword?: (newPassword: string) => Promise<void> | void;
  /** When provided, the Edit button delegates to the modal instead
   * of entering inline edit. */
  onRequestEdit?: () => void;
  /** pre-fetched schema (consumer drives via
   * `useSchemaCacheStore.ensureLoaded`). */
  schema?: ConnectionSchema | null;
  schemaLoading?: boolean;
  schemaError?: string | null;
  /** Top-N hot tables for the schema preview header section.
   * Consumer derives from a `block_run_history` join. */
  hotTables?: HotTableEntry[];
  /** Click → consumer triggers `useSchemaCacheStore.refresh`. */
  onRefreshSchema?: () => void;
  /** runbook usages for the selected connection. */
  usages?: RunbookUsage[];
  usagesLoading?: boolean;
  /** Click on a usage row → consumer opens the file at the line. */
  onOpenUsage?: (filePath: string, line: number) => void;
  /** footer actions. Test resolves to elapsed ms,
   * Duplicate clones with " (copy)" suffix, Delete removes the
   * connection + keychain entry after a two-step confirm. */
  onTestConnection?: () => Promise<number>;
  onDuplicateConnection?: () => Promise<void> | void;
  onDeleteConnection?: () => Promise<void> | void;
}

export function ConnectionsDetailPanel({
  selectedConnectionName,
  selectedConnection = null,
  onSaveCredentials,
  onRotatePassword,
  onRequestEdit,
  schema = null,
  schemaLoading = false,
  schemaError = null,
  hotTables = [],
  onRefreshSchema,
  usages = [],
  usagesLoading = false,
  onOpenUsage,
  onTestConnection,
  onDuplicateConnection,
  onDeleteConnection,
}: ConnectionsDetailPanelProps) {
  return (
    <Box
      data-testid="connections-detail-panel"
      w="420px"
      h="full"
      borderLeftWidth="1px"
      borderLeftColor="border"
      bg="bg.subtle"
      overflowY="auto"
      p={5}
    >
      {selectedConnectionName === null ? (
        <Stack
          h="full"
          align="center"
          justify="center"
          gap={2}
          data-testid="connections-detail-empty"
        >
          <Text fontSize="13px" color="fg.subtle">
            Nothing selected
          </Text>
          <Text fontSize="11px" color="fg.subtle" textAlign="center">
            Pick a connection on the left to see credentials, schema preview,
            and where it's used.
          </Text>
        </Stack>
      ) : selectedConnection ? (
        <Stack gap={4} data-testid="connections-detail-loaded">
          <Text fontSize="14px" fontWeight={600} truncate>
            {selectedConnection.name}
          </Text>
          <ConnectionDetailCredentials
            connection={selectedConnection}
            onSave={onSaveCredentials ?? (() => {})}
            onRotatePassword={onRotatePassword ?? (() => {})}
            onRequestEdit={onRequestEdit}
          />
          <ConnectionDetailSchemaPreview
            schema={schema}
            loading={schemaLoading}
            error={schemaError}
            hotTables={hotTables}
            onRefresh={onRefreshSchema}
          />
          <ConnectionDetailUsedIn
            usages={usages}
            loading={usagesLoading}
            onOpen={onOpenUsage}
          />
          {onTestConnection && onDuplicateConnection && onDeleteConnection && (
            <ConnectionDetailFooter
              onTest={onTestConnection}
              onDuplicate={onDuplicateConnection}
              onDelete={onDeleteConnection}
            />
          )}
        </Stack>
      ) : (
        <Stack gap={3} data-testid="connections-detail-placeholder">
          <Text fontSize="13px" fontWeight={600}>
            {selectedConnectionName}
          </Text>
          <Text fontSize="11px" color="fg.subtle">
            Detail sections (credentials / schema / used in runbooks) land in
            later slices.
          </Text>
        </Stack>
      )}
    </Box>
  );
}
