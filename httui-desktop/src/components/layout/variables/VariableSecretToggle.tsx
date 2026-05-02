// Canvas §6 Variables — is_secret toggle (Epic 43 Story 02 slice 3).
//
// Controlled switch for the variable's `is_secret` flag. Demotion
// (secret → public) goes through `confirmDemote` first — the parent
// owns the actual confirmation UI (could be a modal, an inline
// confirmation banner, or a no-op for tests). When `confirmDemote` is
// undefined the toggle proceeds without asking; when it resolves false
// the toggle is a no-op.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Switch } from "@/components/ui/switch";

export interface VariableSecretToggleProps {
  isSecret: boolean;
  /** Called with the new `is_secret` value after the optional confirmation passes. */
  onToggle?: (next: boolean) => void;
  /** Demotion gate. When provided AND we're going secret → public, the
   * toggle awaits this callback and only flips when it resolves true. */
  confirmDemote?: () => Promise<boolean>;
  /** Disables the switch (e.g. while a save is in flight upstream). */
  disabled?: boolean;
}

export function VariableSecretToggle({
  isSecret,
  onToggle,
  confirmDemote,
  disabled,
}: VariableSecretToggleProps) {
  const [pending, setPending] = useState(false);

  async function handleChange(next: boolean) {
    if (next === isSecret) return;
    if (isSecret && !next && confirmDemote) {
      setPending(true);
      try {
        const ok = await confirmDemote();
        if (!ok) return;
      } finally {
        setPending(false);
      }
    }
    onToggle?.(next);
  }

  return (
    <Box
      data-testid="variable-secret-toggle"
      data-is-secret={isSecret || undefined}
      data-pending={pending || undefined}
      px={4}
      py={3}
      borderTopWidth="1px"
      borderTopColor="border"
    >
      <Flex align="center" justify="space-between" gap={3}>
        <Box>
          <Text
            as="span"
            fontFamily="mono"
            fontSize="11px"
            fontWeight="bold"
            color="fg"
            data-testid="variable-secret-toggle-label"
          >
            is_secret
          </Text>
          <Text
            fontSize="11px"
            color="fg.subtle"
            mt={0.5}
            data-testid="variable-secret-toggle-hint"
          >
            {isSecret
              ? "Valor vive no keychain — não vai pra envs/*.toml."
              : "Valor é gravado em envs/*.toml — qualquer um com o vault vê."}
          </Text>
        </Box>
        <Switch
          size="sm"
          checked={isSecret}
          disabled={disabled || pending}
          data-testid="variable-secret-toggle-switch"
          onCheckedChange={(e: { checked: boolean }) => handleChange(e.checked)}
          aria-label="is_secret"
        />
      </Flex>
    </Box>
  );
}
