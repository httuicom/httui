// Right detail column (420px) for the Connections page.
// Shows credentials, schema preview, and runbook usages for the selected connection.

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
  selectedConnectionName: string | null;
  /** Full Connection record; when absent the panel shows a name-only placeholder. */
  selectedConnection?: Connection | null;
  onSaveCredentials?: (input: UpdateConnectionInput) => Promise<void> | void;
  /** Write new password to the OS keychain. */
  onRotatePassword?: (newPassword: string) => Promise<void> | void;
  /** Delegate Edit to the modal instead of inline editing. */
  onRequestEdit?: () => void;
  schema?: ConnectionSchema | null;
  schemaLoading?: boolean;
  schemaError?: string | null;
  hotTables?: HotTableEntry[];
  onRefreshSchema?: () => void;
  usages?: RunbookUsage[];
  usagesLoading?: boolean;
  onOpenUsage?: (filePath: string, line: number) => void;
  /** Test resolves to elapsed ms; Delete is two-step confirmed. */
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
