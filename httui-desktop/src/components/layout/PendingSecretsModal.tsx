// First-run secrets modal.
//
// Renders a list of `MissingRef`s the user needs to provide a value
// for. Each row has Save (persist + remove from store) and Skip
// (hide from this session of the modal, but **keep** the ref in the
// store so the badge still counts it).
//
// Footer: "Skip all" / "Done" both call `dismiss()` — they hide the
// modal but never touch the store. Whatever wasn't saved stays
// pending and the StatusBar badge surfaces the count.
//
// Re-opening the modal (via the badge) clears the per-session
// "skipped" set so all still-pending refs render again.
//
// Per CLAUDE.md note: do NOT use Chakra `Dialog.Root` — use Portal +
// Box instead so closing returns focus to whatever the user had
// active before. Dialog's focus trap leaves the editor unable to
// receive keyboard input after dismissal.

import { useEffect, useMemo, useState, useCallback } from "react";
import {
  Box,
  Flex,
  HStack,
  Heading,
  Portal,
  Stack,
  Text,
  chakra,
} from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import type { MissingRef } from "@/lib/tauri/commands";
import { saveSecret } from "@/lib/tauri/commands";
import { usePendingSecretsStore } from "@/stores/pendingSecrets";

const Backdrop = chakra("div");

export function PendingSecretsModal() {
  const open = usePendingSecretsStore((s) => s.modalOpen);
  // Outer just gates on `open`. The inner component owns the
  // session-local "skipped" set and only mounts while the modal is
  // visible — so re-opening after a dismiss starts with a fresh
  // `skipped` Set without any useEffect-vs-useEffect race.
  if (!open) return null;
  return <PendingSecretsModalContent />;
}

function PendingSecretsModalContent() {
  const pending = usePendingSecretsStore((s) => s.pending);
  const removePending = usePendingSecretsStore((s) => s.removePending);
  const dismiss = usePendingSecretsStore((s) => s.dismiss);

  // Session-local "skipped" set. Mounted fresh every time the modal
  // opens (outer remount), so we don't need a reset effect.
  const [skipped, setSkipped] = useState<Set<string>>(() => new Set());

  const visible = useMemo(
    () => pending.filter((r) => !skipped.has(r.keychain_key)),
    [pending, skipped],
  );

  // When the visible list empties out — every row was either Saved
  // or Skipped this session — close the modal automatically. Refs
  // skipped this session stay in the store so the badge keeps
  // counting them; refs Saved are already gone from `pending`.
  useEffect(() => {
    if (visible.length === 0) {
      dismiss();
    }
  }, [visible.length, dismiss]);

  return (
    <Portal>
      <Backdrop
        data-testid="pending-secrets-backdrop"
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        display="flex"
        alignItems="center"
        justifyContent="center"
        zIndex={10000}
      >
        <Box
          data-testid="pending-secrets-modal"
          bg="bg"
          borderWidth="1px"
          borderColor="border"
          borderRadius="12px"
          p="24px"
          w="min(560px, 90vw)"
          maxH="80vh"
          overflowY="auto"
          boxShadow="0 24px 60px -20px oklch(0.15 0.04 230 / 0.45)"
        >
          <Heading as="h2" size="lg" mb={1}>
            Secrets pendentes
          </Heading>
          <Text fontSize="13px" color="fg.muted" mb={5}>
            Este vault referencia secrets que ainda não estão no seu keychain.
            Preencha cada um abaixo. Você pode pular agora e preencher depois —
            o app não conseguirá executar blocos que dependam dos secrets
            pulados.
          </Text>

          <Stack gap={3} data-testid="pending-secrets-list">
            {visible.map((ref) => (
              <PendingSecretRow
                key={ref.keychain_key}
                refEntry={ref}
                onSaved={() => removePending(ref.keychain_key)}
                onSkipped={() =>
                  setSkipped((prev) => {
                    const next = new Set(prev);
                    next.add(ref.keychain_key);
                    return next;
                  })
                }
              />
            ))}
          </Stack>

          <Flex mt={6} justify="flex-end" gap={2}>
            <Btn
              variant="ghost"
              data-testid="pending-secrets-skip-all"
              onClick={dismiss}
            >
              Skip all
            </Btn>
            <Btn
              variant="primary"
              data-testid="pending-secrets-done"
              onClick={dismiss}
            >
              Done
            </Btn>
          </Flex>
        </Box>
      </Backdrop>
    </Portal>
  );
}

interface RowProps {
  refEntry: MissingRef;
  /** Save succeeded — remove the ref from the global pending list. */
  onSaved: () => void;
  /** Skip clicked — hide this row from the current modal session, but
   * keep the ref pending in the store. */
  onSkipped: () => void;
}

function PendingSecretRow({ refEntry, onSaved, onSkipped }: RowProps) {
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const handleSave = useCallback(async () => {
    if (!value) {
      setError("Informe o valor do secret");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await saveSecret(refEntry.keychain_key, value);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSaving(false);
    }
  }, [value, refEntry.keychain_key, onSaved]);

  const kindLabel = refEntry.kind === "connection" ? "Conexão" : "Variável";

  return (
    <Box
      data-testid={`pending-secret-row-${refEntry.keychain_key}`}
      borderWidth="1px"
      borderColor="border"
      borderRadius="8px"
      p="12px"
    >
      <HStack gap={2} mb={2} align="baseline">
        <Text fontSize="11px" color="fg.subtle" textTransform="uppercase">
          {kindLabel}
        </Text>
        <Text
          fontFamily="mono"
          fontSize="13px"
          fontWeight={600}
          color="fg"
          data-testid="pending-secret-label"
        >
          {refEntry.label}
        </Text>
        <Text
          fontSize="11px"
          color="fg.subtle"
          ml="auto"
          data-testid="pending-secret-source"
        >
          {refEntry.source_file}
        </Text>
      </HStack>

      <HStack gap={2}>
        <Input
          type="password"
          data-testid="pending-secret-input"
          aria-label={`Valor para ${refEntry.label}`}
          placeholder="••••••"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          disabled={saving}
          flex={1}
        />
        <Btn
          variant="primary"
          data-testid="pending-secret-save"
          onClick={handleSave}
          disabled={saving}
        >
          {saving ? "…" : "Save"}
        </Btn>
        <Btn
          variant="ghost"
          data-testid="pending-secret-skip"
          onClick={onSkipped}
          disabled={saving}
        >
          Skip
        </Btn>
      </HStack>

      {error && (
        <Text
          mt={2}
          fontSize="11px"
          color="red.500"
          data-testid="pending-secret-error"
        >
          {error}
        </Text>
      )}
    </Box>
  );
}
