// Body of the inline `{{ref}}` quick popover (V11 cenário 3).
//
// Shows the variable's per-env value, a session-override input
// (reuses the V5 useSessionOverrideStore + TemporaryChip), and a
// "Used in N blocks" expander. Block-ref shaped keys (with dots)
// get a read-only note instead of the override controls.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useEffect, useState } from "react";

import { Btn, Input } from "@/components/atoms";
import { TemporaryChip } from "@/components/layout/variables/TemporaryChip";
import { grepVarUses, type VarUseEntry } from "@/lib/tauri/var-uses";
import { useEnvironmentStore } from "@/stores/environment";
import { useSessionOverrideStore } from "@/stores/sessionOverride";
import type { RefPopoverState } from "@/lib/blocks/cm-ref-popover";

export interface RefPopoverProps {
  state: RefPopoverState;
  vaultPath: string | null;
  onClose: () => void;
}

export function RefPopover({ state, vaultPath, onClose }: RefPopoverProps) {
  const rawKey = state.rawKey;
  const isEnvVar = !rawKey.includes(".");
  const envName = useEnvironmentStore((s) => s.activeEnvironment?.name ?? null);
  const clearOverride = useSessionOverrideStore((s) => s.clearOverride);
  const override = useSessionOverrideStore((s) =>
    envName ? s.overrides[envName]?.[rawKey] : undefined,
  );

  return (
    <Box
      data-testid="ref-popover"
      data-ref={rawKey}
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      shadow="2xl"
      minW="300px"
      maxW="420px"
      p={3}
    >
      <Flex align="center" gap={2} mb={2}>
        <Text
          flex={1}
          fontFamily="mono"
          fontSize="12px"
          fontWeight={600}
          color="brand.fg"
          truncate
        >
          {`{{${rawKey}}}`}
        </Text>
        {override !== undefined && envName && (
          <TemporaryChip onClear={() => clearOverride(envName, rawKey)} />
        )}
      </Flex>

      {!isEnvVar ? (
        <Text fontSize="11px" color="fg.muted" data-testid="ref-popover-blockref">
          Block reference — resolves from the block above at run time.
        </Text>
      ) : (
        <RefEnvVarBody
          rawKey={rawKey}
          envName={envName}
          override={override}
          vaultPath={vaultPath}
        />
      )}

      <Flex justify="flex-end" mt={3}>
        <Btn variant="ghost" data-testid="ref-popover-close" onClick={onClose}>
          Close
        </Btn>
      </Flex>
    </Box>
  );
}

interface RefEnvVarBodyProps {
  rawKey: string;
  envName: string | null;
  override: string | undefined;
  vaultPath: string | null;
}

function RefEnvVarBody({
  rawKey,
  envName,
  override,
  vaultPath,
}: RefEnvVarBodyProps) {
  const setOverride = useSessionOverrideStore((s) => s.setOverride);
  const [resolved, setResolved] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [uses, setUses] = useState<VarUseEntry[] | null>(null);
  const [showUses, setShowUses] = useState(false);

  useEffect(() => {
    let alive = true;
    void Promise.resolve(
      useEnvironmentStore.getState().getActiveVariables(),
    ).then((vars) => {
      if (alive) setResolved(vars[rawKey] ?? null);
    });
    return () => {
      alive = false;
    };
  }, [rawKey]);

  useEffect(() => {
    if (!vaultPath) return;
    let alive = true;
    grepVarUses(vaultPath, rawKey)
      .then((u) => alive && setUses(u))
      .catch(() => alive && setUses([]));
    return () => {
      alive = false;
    };
  }, [vaultPath, rawKey]);

  function applyOverride() {
    if (!envName || draft.trim() === "") return;
    setOverride(envName, rawKey, draft);
    setDraft("");
  }

  const count = uses?.length ?? 0;

  return (
    <>
      <Box mb={3}>
        <Text fontSize="10px" color="fg.subtle" mb={0.5}>
          {envName ? `value in ${envName}` : "no active environment"}
        </Text>
        <Text
          fontFamily="mono"
          fontSize="12px"
          color={resolved == null ? "fg.subtle" : "fg"}
          data-testid="ref-popover-value"
          wordBreak="break-all"
        >
          {override ?? resolved ?? "(not set in active env)"}
        </Text>
      </Box>

      <Box mb={3}>
        <Text fontSize="10px" color="fg.subtle" mb={1}>
          Session override
        </Text>
        <Flex gap={2}>
          <Input
            data-testid="ref-popover-override-input"
            placeholder="temporary value…"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                applyOverride();
              }
            }}
            disabled={!envName}
          />
          <Btn
            variant="ghost"
            data-testid="ref-popover-override-set"
            onClick={applyOverride}
            disabled={!envName || draft.trim() === ""}
          >
            Set
          </Btn>
        </Flex>
      </Box>

      <Btn
        variant="ghost"
        data-testid="ref-popover-uses"
        onClick={() => setShowUses((v) => !v)}
        disabled={count === 0}
      >
        Used in {count} block{count === 1 ? "" : "s"}
      </Btn>
      {showUses && count > 0 && (
        <Box mt={2} maxH="140px" overflowY="auto">
          {uses?.map((u, i) => (
            <Text
              key={`${u.file_path}:${u.line}:${i}`}
              fontFamily="mono"
              fontSize="10px"
              color="fg.muted"
              truncate
              data-testid="ref-popover-use-row"
            >
              {u.file_path}:{u.line}
            </Text>
          ))}
        </Box>
      )}
    </>
  );
}
