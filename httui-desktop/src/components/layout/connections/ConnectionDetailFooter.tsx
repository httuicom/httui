// Canvas §5 — Detail panel footer actions (Epic 42 Story 05).
//
// Three buttons: Test (with inline latency result), Duplicate
// (clones the connection with " (copy)" suffix), Delete (two-step
// confirm). The Rotate flow from Story 02 stays inside the
// credentials section — Story 05's "Rotate" task is handled there
// (audit-032 documents the consolidation).
//
// Pure presentational: takes async callbacks. Test result + delete
// confirm state lives locally so each test runs cleanly.

import { useEffect, useRef, useState } from "react";
import { Box, Flex, HStack, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

export type TestResult =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "ok"; latencyMs: number }
  | { kind: "err"; message: string };

export interface ConnectionDetailFooterProps {
  /** Run the connection test. Resolves to elapsed ms on success;
   * rejects with an Error on failure. */
  onTest: () => Promise<number>;
  /** Clone the current connection with " (copy)" suffix. Consumer
   * issues the create call (no password copied). */
  onDuplicate: () => Promise<void> | void;
  /** Delete after a two-step confirm. Consumer removes the
   * connection from `connections.toml` + the keychain entry. */
  onDelete: () => Promise<void> | void;
  /** Reset window for the delete confirmation in milliseconds.
   * After this many ms without a second click, Delete reverts to
   * "Delete" state. Default 4000ms. */
  deleteConfirmTimeoutMs?: number;
}

export function ConnectionDetailFooter({
  onTest,
  onDuplicate,
  onDelete,
  deleteConfirmTimeoutMs = 4000,
}: ConnectionDetailFooterProps) {
  const [testResult, setTestResult] = useState<TestResult>({ kind: "idle" });
  const [duplicateBusy, setDuplicateBusy] = useState(false);
  const [duplicateError, setDuplicateError] = useState<string | null>(null);

  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const confirmTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (confirmTimerRef.current !== null) {
        clearTimeout(confirmTimerRef.current);
        confirmTimerRef.current = null;
      }
    };
  }, []);

  async function handleTest() {
    setTestResult({ kind: "running" });
    try {
      const latencyMs = await onTest();
      setTestResult({ kind: "ok", latencyMs });
    } catch (err) {
      setTestResult({
        kind: "err",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }

  async function handleDuplicate() {
    setDuplicateBusy(true);
    setDuplicateError(null);
    try {
      await onDuplicate();
    } catch (err) {
      setDuplicateError(err instanceof Error ? err.message : String(err));
    } finally {
      setDuplicateBusy(false);
    }
  }

  function handleDeleteClick() {
    if (!confirmingDelete) {
      setConfirmingDelete(true);
      setDeleteError(null);
      if (confirmTimerRef.current !== null) {
        clearTimeout(confirmTimerRef.current);
      }
      confirmTimerRef.current = setTimeout(() => {
        setConfirmingDelete(false);
      }, deleteConfirmTimeoutMs);
      return;
    }
    void confirmDelete();
  }

  async function confirmDelete() {
    if (confirmTimerRef.current !== null) {
      clearTimeout(confirmTimerRef.current);
      confirmTimerRef.current = null;
    }
    setDeleteBusy(true);
    setDeleteError(null);
    try {
      await onDelete();
      setConfirmingDelete(false);
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleteBusy(false);
    }
  }

  return (
    <Box
      data-testid="connection-detail-footer"
      borderTopWidth="1px"
      borderTopColor="border"
      pt={3}
    >
      <Flex justify="space-between" align="center" gap={2} wrap="wrap">
        <HStack gap={1}>
          <Btn
            variant="ghost"
            data-testid="footer-test"
            onClick={handleTest}
            disabled={testResult.kind === "running"}
          >
            {testResult.kind === "running" ? "Testing…" : "▶ Test"}
          </Btn>
          <Btn
            variant="ghost"
            data-testid="footer-duplicate"
            onClick={handleDuplicate}
            disabled={duplicateBusy}
          >
            {duplicateBusy ? "Duplicating…" : "Duplicate"}
          </Btn>
        </HStack>
        <Btn
          variant="ghost"
          data-testid="footer-delete"
          onClick={handleDeleteClick}
          disabled={deleteBusy}
          color={confirmingDelete ? "red.fg" : undefined}
        >
          {deleteBusy
            ? "Deleting…"
            : confirmingDelete
              ? "Click again to confirm"
              : "Delete"}
        </Btn>
      </Flex>

      <TestResultBanner result={testResult} />

      {duplicateError !== null && (
        <Text
          data-testid="footer-duplicate-error"
          fontSize="11px"
          color="red.fg"
          mt={2}
        >
          {duplicateError}
        </Text>
      )}

      {deleteError !== null && (
        <Text
          data-testid="footer-delete-error"
          fontSize="11px"
          color="red.fg"
          mt={2}
        >
          {deleteError}
        </Text>
      )}
    </Box>
  );
}

function TestResultBanner({ result }: { result: TestResult }) {
  if (result.kind === "idle" || result.kind === "running") return null;
  if (result.kind === "ok") {
    return (
      <Box
        data-testid="footer-test-ok"
        mt={2}
        px={2.5}
        py={1.5}
        borderWidth="1px"
        borderColor="green.muted"
        bg="green.subtle"
        color="green.fg"
        borderRadius="6px"
        fontSize="11px"
      >
        <Flex align="center" gap={2}>
          <Box
            h="6px"
            w="6px"
            borderRadius="full"
            bg="green.solid"
            aria-hidden
          />
          <Text fontWeight={500}>Conexão OK</Text>
          <Text fontFamily="mono" color="green.fg">
            {result.latencyMs}ms
          </Text>
        </Flex>
      </Box>
    );
  }
  return (
    <Box
      data-testid="footer-test-err"
      mt={2}
      px={2.5}
      py={1.5}
      borderWidth="1px"
      borderColor="red.muted"
      bg="red.subtle"
      color="red.fg"
      borderRadius="6px"
      fontSize="11px"
    >
      <Flex align="center" gap={2}>
        <Box
          h="6px"
          w="6px"
          borderRadius="full"
          bg="red.solid"
          aria-hidden
        />
        <Text fontWeight={500}>Falhou</Text>
        <Text fontFamily="mono" color="red.fg" truncate>
          {result.message}
        </Text>
      </Flex>
    </Box>
  );
}
