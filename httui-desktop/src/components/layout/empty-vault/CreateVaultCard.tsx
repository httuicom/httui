// "Create vault" card — V1 vertical 1.
//
// Expandable card: collapsed state shows the icon/title/body and a
// CTA pill; expanded state shows a parent-folder picker + name input
// + Create submit. Consumer wires `onCreate(parent, name)` to the
// Tauri `create_vault` command.

import { useState, useCallback } from "react";
import { Box, HStack, Stack, Text, chakra } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";

const CardBox = chakra("div");

export interface CreateVaultCardProps {
  /** `(parentPath, name)` → consumer creates and switches into the
   * new vault. */
  onCreate: (parentPath: string, name: string) => Promise<void>;
  /** Open a directory picker for the parent folder. Returns absolute
   * path or `null` when the user cancels. */
  onPickParent: () => Promise<string | null>;
  /** Disable while another card is mid-flow. */
  busy?: boolean;
}

export function CreateVaultCard({
  onCreate,
  onPickParent,
  busy = false,
}: CreateVaultCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [parent, setParent] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const handleExpand = useCallback(() => {
    setExpanded(true);
    setError(null);
  }, []);

  const handlePick = useCallback(async () => {
    try {
      const picked = await onPickParent();
      if (picked) setParent(picked);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [onPickParent]);

  const handleSubmit = useCallback(async () => {
    if (!parent) {
      setError("Escolha a pasta pai");
      return;
    }
    const trimmed = name.trim();
    if (!trimmed) {
      setError("Informe um nome para o vault");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await onCreate(parent, trimmed);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [parent, name, onCreate]);

  const disabled = busy || submitting;

  return (
    <CardBox
      data-atom="create-vault-card"
      data-testid="create-vault-card"
      data-expanded={expanded ? "true" : "false"}
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="12px"
      p="22px"
      minH="260px"
      opacity={busy ? 0.6 : 1}
    >
      <Stack gap={3} h="full" align="stretch">
        <Box
          aria-hidden
          w="32px"
          h="32px"
          borderRadius="6px"
          bg="color-mix(in oklab, oklch(0.62 0.10 145) 14%, transparent)"
          display="inline-flex"
          alignItems="center"
          justifyContent="center"
          color="oklch(0.62 0.10 145)"
          fontSize="16px"
          data-testid="create-vault-icon"
        >
          ✎
        </Box>
        <Text
          fontFamily="var(--chakra-fonts-serif)"
          fontSize="18px"
          fontWeight={600}
          color="fg"
          data-testid="create-vault-title"
        >
          Create vault
        </Text>
        <Text
          fontSize="12px"
          color="fg.muted"
          lineHeight={1.4}
          data-testid="create-vault-body"
        >
          Comece do zero com uma pasta nova já versionada com git.
        </Text>

        {!expanded ? (
          <chakra.button
            type="button"
            data-testid="create-vault-expand"
            onClick={handleExpand}
            disabled={disabled}
            mt={1.5}
            fontSize="11px"
            color="brand.fg"
            fontWeight={600}
            textAlign="left"
            bg="transparent"
            cursor={disabled ? "not-allowed" : "pointer"}
            _disabled={{ opacity: 0.6 }}
          >
            Criar vault novo →
          </chakra.button>
        ) : (
          <Stack gap={2} mt={1.5} data-testid="create-vault-form">
            <HStack gap={2}>
              <chakra.button
                type="button"
                data-testid="create-vault-pick-parent"
                onClick={handlePick}
                disabled={disabled}
                h="24px"
                px="10px"
                borderRadius="4px"
                fontSize="11px"
                bg="bg.muted"
                borderWidth="1px"
                borderColor="border"
                color="fg.muted"
                cursor={disabled ? "not-allowed" : "pointer"}
                _hover={disabled ? undefined : { bg: "bg.emphasized" }}
              >
                Choose…
              </chakra.button>
              <Text
                flex={1}
                fontSize="11px"
                color="fg.subtle"
                truncate
                data-testid="create-vault-parent"
              >
                {parent ?? "(escolha a pasta pai)"}
              </Text>
            </HStack>
            <Input
              data-testid="create-vault-name"
              aria-label="Nome do vault"
              placeholder="meu-vault"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={disabled}
            />
            {error && (
              <Text
                data-testid="create-vault-error"
                fontSize="11px"
                color="red.500"
              >
                {error}
              </Text>
            )}
            <Btn
              variant="primary"
              data-testid="create-vault-submit"
              onClick={handleSubmit}
              disabled={disabled}
            >
              {submitting ? "Criando…" : "Create"}
            </Btn>
          </Stack>
        )}
      </Stack>
    </CardBox>
  );
}
