// Canvas §5 — Detail panel credentials section (Epic 42 Story 02).
//
// Read-only summary by default (host / port / user / database / `••••••••`).
// "Edit" toggles inputs + reveals Save/Cancel.
// "Rotate" button opens a small inline form to write a new password
// (consumer pushes through the keychain).
//
// Pure presentational: takes the `Connection` plus async `onSave` /
// `onRotatePassword` callbacks. No store coupling so tests can mock
// behavior with vi.fn() promises.

import { useEffect, useState } from "react";
import {
  Box,
  Flex,
  HStack,
  Stack,
  Text,
  chakra,
} from "@chakra-ui/react";

import { LuKey } from "react-icons/lu";

import { Btn } from "@/components/atoms";
import type {
  Connection,
  UpdateConnectionInput,
} from "@/lib/tauri/connections";

const Field = chakra("input");

export interface ConnectionDetailCredentialsProps {
  connection: Connection;
  onSave: (input: UpdateConnectionInput) => Promise<void> | void;
  onRotatePassword: (newPassword: string) => Promise<void> | void;
  /** When provided, the Edit button delegates to this callback
   * (opens the full NewConnectionModal in edit mode) instead of
   * entering inline edit. Single source of truth for edit. */
  onRequestEdit?: () => void;
}

interface DraftState {
  host: string;
  port: string;
  database: string;
  username: string;
}

function draftFrom(c: Connection): DraftState {
  return {
    host: c.host ?? "",
    port: c.port === null ? "" : String(c.port),
    database: c.database_name ?? "",
    username: c.username ?? "",
  };
}

const PASSWORD_MASK = "••••••••";

export function ConnectionDetailCredentials({
  connection,
  onSave,
  onRotatePassword,
  onRequestEdit,
}: ConnectionDetailCredentialsProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<DraftState>(draftFrom(connection));
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [rotating, setRotating] = useState(false);
  const [rotatePassword, setRotatePassword] = useState("");
  const [rotateBusy, setRotateBusy] = useState(false);
  const [rotateError, setRotateError] = useState<string | null>(null);

  // Reset draft when the selected connection changes.
  useEffect(() => {
    setDraft(draftFrom(connection));
    setEditing(false);
    setSaveError(null);
    setRotating(false);
    setRotatePassword("");
    setRotateError(null);
  }, [connection]);

  function startEdit() {
    if (onRequestEdit) {
      onRequestEdit();
      return;
    }
    setDraft(draftFrom(connection));
    setSaveError(null);
    setEditing(true);
  }

  function cancelEdit() {
    setDraft(draftFrom(connection));
    setSaveError(null);
    setEditing(false);
  }

  async function handleSave() {
    setSaveBusy(true);
    setSaveError(null);
    try {
      const input: UpdateConnectionInput = {
        host: draft.host,
        port:
          draft.port.trim().length === 0
            ? undefined
            : Number(draft.port) || 0,
        database_name: draft.database,
        username: draft.username,
      };
      await onSave(input);
      setEditing(false);
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaveBusy(false);
    }
  }

  function startRotate() {
    setRotatePassword("");
    setRotateError(null);
    setRotating(true);
  }

  function cancelRotate() {
    setRotatePassword("");
    setRotateError(null);
    setRotating(false);
  }

  async function handleRotate() {
    if (rotatePassword.length === 0) {
      setRotateError("Password cannot be empty");
      return;
    }
    setRotateBusy(true);
    setRotateError(null);
    try {
      await onRotatePassword(rotatePassword);
      setRotating(false);
      setRotatePassword("");
    } catch (err) {
      setRotateError(err instanceof Error ? err.message : String(err));
    } finally {
      setRotateBusy(false);
    }
  }

  return (
    <Stack
      data-testid="connection-credentials"
      gap={3}
      align="stretch"
    >
      <Flex justify="space-between" align="center">
        <Text
          fontFamily="mono"
          fontSize="11px"
          fontWeight="bold"
          letterSpacing="0.08em"
          textTransform="uppercase"
          color="fg.muted"
        >
          Credentials
        </Text>
        {!editing ? (
          <Btn
            variant="ghost"
            data-testid="credentials-edit"
            onClick={startEdit}
          >
            Edit
          </Btn>
        ) : (
          <HStack gap={1}>
            <Btn
              variant="ghost"
              data-testid="credentials-cancel"
              onClick={cancelEdit}
              disabled={saveBusy}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="credentials-save"
              onClick={handleSave}
              disabled={saveBusy}
            >
              {saveBusy ? "Saving…" : "Save"}
            </Btn>
          </HStack>
        )}
      </Flex>

      {!editing ? (
        <Stack
          gap={2}
          data-testid="credentials-readonly"
          fontSize="12px"
        >
          <SummaryRow label="Host" value={connection.host ?? "—"} />
          <SummaryRow
            label="Port"
            value={connection.port === null ? "—" : String(connection.port)}
          />
          <SummaryRow label="User" value={connection.username ?? "—"} />
          <SummaryRow
            label="Database"
            value={connection.database_name ?? "—"}
          />
          <SummaryRow label="Password" value={PASSWORD_MASK} mono />
        </Stack>
      ) : (
        <Stack
          gap={2}
          data-testid="credentials-editing"
          fontSize="12px"
        >
          <EditField
            label="Host"
            testId="credentials-host"
            value={draft.host}
            onChange={(v) => setDraft({ ...draft, host: v })}
          />
          <EditField
            label="Port"
            testId="credentials-port"
            value={draft.port}
            onChange={(v) => setDraft({ ...draft, port: v })}
          />
          <EditField
            label="User"
            testId="credentials-user"
            value={draft.username}
            onChange={(v) => setDraft({ ...draft, username: v })}
          />
          <EditField
            label="Database"
            testId="credentials-database"
            value={draft.database}
            onChange={(v) => setDraft({ ...draft, database: v })}
          />
        </Stack>
      )}

      {saveError !== null && (
        <Text
          data-testid="credentials-save-error"
          fontSize="11px"
          color="red.fg"
        >
          {saveError}
        </Text>
      )}

      <Box
        data-testid="credentials-rotate-section"
        borderTopWidth="1px"
        borderTopColor="border"
        pt={3}
      >
        {!rotating ? (
          <Btn
            variant="ghost"
            data-testid="credentials-rotate"
            onClick={startRotate}
          >
            <LuKey size={12} />
            Rotate password
          </Btn>
        ) : (
          <Stack gap={2}>
            <Text fontSize="11px" color="fg.muted">
              Enter the new password — it will be written to the OS
              keychain. The vault file only stores a{" "}
              <Box as="code" fontFamily="mono">
                {"{{keychain:…}}"}
              </Box>{" "}
              reference.
            </Text>
            <Field
              data-testid="credentials-rotate-input"
              type="password"
              value={rotatePassword}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setRotatePassword(e.target.value)
              }
              h="28px"
              px={2}
              fontSize="12px"
              fontFamily="mono"
              bg="bg.muted"
              color="fg"
              borderWidth="1px"
              borderColor="border"
              borderRadius="6px"
              outline="none"
              _focus={{ borderColor: "brand.fg" }}
            />
            <HStack gap={1}>
              <Btn
                variant="ghost"
                data-testid="credentials-rotate-cancel"
                onClick={cancelRotate}
                disabled={rotateBusy}
              >
                Cancel
              </Btn>
              <Btn
                variant="primary"
                data-testid="credentials-rotate-save"
                onClick={handleRotate}
                disabled={rotateBusy}
              >
                {rotateBusy ? "Rotating…" : "Save new password"}
              </Btn>
            </HStack>
            {rotateError !== null && (
              <Text
                data-testid="credentials-rotate-error"
                fontSize="11px"
                color="red.fg"
              >
                {rotateError}
              </Text>
            )}
          </Stack>
        )}
      </Box>
    </Stack>
  );
}

function SummaryRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <Flex
      data-testid={`credentials-row-${label.toLowerCase()}`}
      align="baseline"
      justify="space-between"
      gap={3}
    >
      <Text fontSize="11px" color="fg.subtle" minW="64px">
        {label}
      </Text>
      <Text
        flex={1}
        fontFamily={mono ? "mono" : "body"}
        color="fg"
        textAlign="right"
        truncate
      >
        {value}
      </Text>
    </Flex>
  );
}

function EditField({
  label,
  testId,
  value,
  onChange,
}: {
  label: string;
  testId: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <Flex align="center" gap={3}>
      <Text fontSize="11px" color="fg.subtle" minW="64px">
        {label}
      </Text>
      <Field
        data-testid={testId}
        type="text"
        value={value}
        onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
          onChange(e.target.value)
        }
        flex={1}
        h="28px"
        px={2}
        fontSize="12px"
        fontFamily="mono"
        bg="bg.muted"
        color="fg"
        borderWidth="1px"
        borderColor="border"
        borderRadius="6px"
        outline="none"
        _focus={{ borderColor: "brand.fg" }}
      />
    </Flex>
  );
}
