// "Clone vault" card — V1 vertical 1.
//
// Expandable card: collapsed state shows the icon/title/body and a
// CTA pill; expanded state shows a URL input + optional parent
// folder picker + Clone submit. The consumer wires
// `onClone(url, parent)` to the Tauri `clone_vault_cmd` command;
// the backend always derives the leaf folder name from the URL,
// so `parent` is just the *container* (defaults to `~/Documents`
// when null).

import { useState, useCallback } from "react";
import { Box, HStack, Stack, Text, chakra } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";

const CardBox = chakra("div");

export interface CloneVaultCardProps {
  /** `(url, parent)` → consumer runs the clone + switchVault.
   * `parent` is the container folder; the leaf is always derived
   * from the URL on the backend. */
  onClone: (url: string, parent: string | null) => Promise<void>;
  /** Open a directory picker for the parent folder. Returns absolute
   * path or `null` when the user cancels. */
  onPickParent: () => Promise<string | null>;
  /** Disable while another card is mid-flow. */
  busy?: boolean;
}

export function CloneVaultCard({
  onClone,
  onPickParent,
  busy = false,
}: CloneVaultCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [url, setUrl] = useState("");
  const [parent, setParent] = useState<string | null>(null);
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
    const trimmed = url.trim();
    if (!trimmed) {
      setError("Informe a URL do repositório");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await onClone(trimmed, parent);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [url, parent, onClone]);

  const disabled = busy || submitting;

  return (
    <CardBox
      data-atom="clone-vault-card"
      data-testid="clone-vault-card"
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
          bg="color-mix(in oklab, oklch(0.62 0.14 50) 14%, transparent)"
          display="inline-flex"
          alignItems="center"
          justifyContent="center"
          color="oklch(0.62 0.14 50)"
          fontSize="16px"
          data-testid="clone-vault-icon"
        >
          ↧
        </Box>
        <Text
          fontFamily="var(--chakra-fonts-serif)"
          fontSize="18px"
          fontWeight={600}
          color="fg"
          data-testid="clone-vault-title"
        >
          Clone vault
        </Text>
        <Text
          fontSize="12px"
          color="fg.muted"
          lineHeight={1.4}
          data-testid="clone-vault-body"
        >
          Clone um repositório git da sua equipe (público ou privado, via
          credenciais do sistema).
        </Text>

        {!expanded ? (
          <chakra.button
            type="button"
            data-testid="clone-vault-expand"
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
            Clonar repositório →
          </chakra.button>
        ) : (
          <Stack gap={2} mt={1.5} data-testid="clone-vault-form">
            <Input
              data-testid="clone-vault-url"
              aria-label="URL do repositório"
              placeholder="https://github.com/owner/repo.git"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              disabled={disabled}
            />
            <HStack gap={2}>
              <chakra.button
                type="button"
                data-testid="clone-vault-pick-parent"
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
                Pasta pai…
              </chakra.button>
              <Text
                flex={1}
                fontSize="11px"
                color="fg.subtle"
                truncate
                data-testid="clone-vault-parent"
              >
                {parent ?? "(padrão: ~/Documents)"}
              </Text>
            </HStack>
            {error && (
              <Text
                data-testid="clone-vault-error"
                fontSize="11px"
                color="red.500"
              >
                {error}
              </Text>
            )}
            <Btn
              variant="primary"
              data-testid="clone-vault-submit"
              onClick={handleSubmit}
              disabled={disabled}
            >
              {submitting ? "Clonando…" : "Clone"}
            </Btn>
          </Stack>
        )}
      </Stack>
    </CardBox>
  );
}
