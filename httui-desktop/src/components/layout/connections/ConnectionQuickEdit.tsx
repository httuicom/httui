// Connection quick-edit popover body (carry).
//
// Mounted by ConnectionsList inside a Portal+Popover anchored to the
// sidebar chip (NOT a Dialog — preserves CM6 focus). Surfaces, in
// order: status badge → Rotate password → Temporary host:port
// override → Test → Duplicate, plus Edit/Delete as footer actions.
//
// "Temporary host:port" is session-only: it writes to
// `useConnectionSessionOverrideStore`, never to the vault record. The
// override is applied per DB run in `executeDbStreamed`.

import { Box, Flex, HStack, Text } from "@chakra-ui/react";
import { useState } from "react";
import { LuCopy, LuPencil, LuPlugZap, LuTrash2 } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";
import { TemporaryChip } from "@/components/layout/variables/TemporaryChip";
import type { Connection } from "@/lib/tauri/connections";
import { updateConnection } from "@/lib/tauri/connections";
import { useConnectionSessionOverrideStore } from "@/stores/connectionSessionOverride";

export interface ConnectionQuickEditProps {
  conn: Connection;
  /** Current ping status for the dot/latency line. */
  pingStatus?: "idle" | "ok" | "err";
  pingLatencyMs?: number | null;
  /** Re-ping this connection (parent owns the ping map). */
  onTest: () => void;
  /** Open the full ConnectionForm for this connection. */
  onEdit: () => void;
  /** Delete this connection. */
  onDelete: () => void;
  /** Duplicate this connection (parent calls create_connection). */
  onDuplicate: () => void;
  /** Called after a mutation (rotate password) so the list refreshes. */
  onChanged: () => void;
}

const SectionLabel = ({ children }: { children: string }) => (
  <Text
    fontSize="10px"
    fontWeight={600}
    textTransform="uppercase"
    letterSpacing="0.06em"
    color="fg.subtle"
    mb={1}
  >
    {children}
  </Text>
);

export function ConnectionQuickEdit({
  conn,
  pingStatus = "idle",
  pingLatencyMs,
  onTest,
  onEdit,
  onDelete,
  onDuplicate,
  onChanged,
}: ConnectionQuickEditProps) {
  const override = useConnectionSessionOverrideStore((s) =>
    s.getOverride(conn.id),
  );
  const setOverride = useConnectionSessionOverrideStore((s) => s.setOverride);
  const clearOverride = useConnectionSessionOverrideStore(
    (s) => s.clearOverride,
  );

  const [password, setPassword] = useState("");
  const [rotateMsg, setRotateMsg] = useState<string | null>(null);
  const [host, setHost] = useState(override?.host ?? "");
  const [port, setPort] = useState(
    override?.port != null ? String(override.port) : "",
  );

  const dotColor =
    pingStatus === "ok"
      ? "green.500"
      : pingStatus === "err"
        ? "red.500"
        : "gray.500";

  async function handleRotate() {
    if (password.trim() === "") return;
    try {
      await updateConnection(conn.id, { password });
      setPassword("");
      setRotateMsg("Password updated");
      onChanged();
    } catch (e) {
      setRotateMsg(e instanceof Error ? e.message : "Failed to update");
    }
  }

  function handleApplyOverride() {
    const portNum = port.trim() === "" ? undefined : Number(port);
    setOverride(conn.id, {
      host: host.trim() === "" ? undefined : host,
      port: Number.isFinite(portNum) ? portNum : undefined,
    });
  }

  function handleClearOverride() {
    clearOverride(conn.id);
    setHost("");
    setPort("");
  }

  return (
    <Box
      data-testid={`conn-quickedit-${conn.id}`}
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      shadow="2xl"
      minW="320px"
      p={3}
    >
      {/* Status badge */}
      <Flex align="center" gap={2} mb={3}>
        <Box w={2} h={2} rounded="full" bg={dotColor} flexShrink={0} />
        <Text flex={1} fontFamily="mono" fontSize="12px" truncate>
          {conn.name}
        </Text>
        {pingLatencyMs != null && (
          <Text fontFamily="mono" fontSize="10px" color="fg.subtle">
            {pingLatencyMs}ms
          </Text>
        )}
        {override && <TemporaryChip onClear={handleClearOverride} />}
      </Flex>

      {/* Rotate password */}
      <Box mb={3}>
        <SectionLabel>Rotate password</SectionLabel>
        <HStack gap={2}>
          <Input
            data-testid="conn-quickedit-password"
            type="password"
            placeholder="New password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          <Btn
            variant="ghost"
            data-testid="conn-quickedit-rotate"
            onClick={handleRotate}
            disabled={password.trim() === ""}
          >
            Rotate
          </Btn>
        </HStack>
        {rotateMsg && (
          <Text
            fontSize="10px"
            color="fg.muted"
            mt={1}
            data-testid="conn-quickedit-rotate-msg"
          >
            {rotateMsg}
          </Text>
        )}
      </Box>

      {/* Temporary host:port override */}
      <Box mb={3}>
        <SectionLabel>Temporary host:port</SectionLabel>
        <HStack gap={2}>
          <Input
            data-testid="conn-quickedit-host"
            placeholder={conn.host ?? "host"}
            value={host}
            onChange={(e) => setHost(e.target.value)}
          />
          <Input
            data-testid="conn-quickedit-port"
            placeholder={conn.port != null ? String(conn.port) : "port"}
            value={port}
            onChange={(e) => setPort(e.target.value)}
            w="84px"
          />
          <Btn
            variant="ghost"
            data-testid="conn-quickedit-apply"
            onClick={handleApplyOverride}
          >
            Apply
          </Btn>
        </HStack>
        <Text fontSize="10px" color="fg.subtle" mt={1}>
          Session-only — the saved connection is untouched.
        </Text>
      </Box>

      {/* Test + Duplicate */}
      <HStack gap={2}>
        <Btn variant="ghost" data-testid="conn-quickedit-test" onClick={onTest}>
          <LuPlugZap size={13} />
          Test
        </Btn>
        <Btn
          variant="ghost"
          data-testid="conn-quickedit-duplicate"
          onClick={onDuplicate}
        >
          <LuCopy size={13} />
          Duplicate
        </Btn>
      </HStack>

      <Flex
        mt={3}
        pt={2}
        borderTopWidth="1px"
        borderTopColor="border"
        justify="space-between"
      >
        <Btn variant="ghost" data-testid="conn-quickedit-edit" onClick={onEdit}>
          <LuPencil size={13} />
          Edit…
        </Btn>
        <Btn
          variant="ghost"
          data-testid="conn-quickedit-delete"
          onClick={onDelete}
          color="fg.error"
        >
          <LuTrash2 size={13} />
          Delete
        </Btn>
      </Flex>
    </Box>
  );
}
